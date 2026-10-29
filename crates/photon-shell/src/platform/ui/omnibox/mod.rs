//! The address and search field in the browser toolbar. While typing, it
//! opens into a panel of suggestions over the page.
//!
//! - `editing`: typing, inline completion, choosing and removing suggestions.
//! - `panel`: the field's content and the open panel.
//! - `rows`: the suggestion rows.

mod editing;
mod panel;
mod rows;
mod site_info;

use super::glass::{glass, glass_available};
use gpui::{
    Context, Entity, EventEmitter, Focusable, MouseButton, Render, Subscription, Task, Window,
    prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape},
};
use photon_core::{History, Suggestion};
use photon_core::{OmniboxTarget, display_address, display_domain, resolve_with};
use std::sync::Arc;

use super::history::BrowsingHistory;
use super::motion::{Speed, Tween};
use super::workspace_layout::{OmniboxPlacement, WorkspaceLayout};
use super::{PhotonWebView, WebViewEvent};
use super::{metrics, settings::Settings, theme::palette};

const INVALID_ADDRESS_DESCRIPTION: &str = "This address can't be opened";

/// The field's size: compact in the toolbar, larger in the sidebar, where it
/// also leaves out the search icon until it shows suggestions.
#[derive(Clone, Copy)]
struct FieldStyle {
    height: f32,
    radius: f32,
    font_size: f32,
    idle_icon: bool,
    center_text: bool,
    trailing_site_info: bool,
}

impl FieldStyle {
    fn for_layout(layout: OmniboxPlacement) -> Self {
        match layout {
            OmniboxPlacement::Toolbar => Self {
                height: metrics::OMNIBOX_HEIGHT,
                radius: metrics::omnibox_radius(),
                font_size: metrics::OMNIBOX_FONT_SIZE,
                idle_icon: true,
                center_text: false,
                trailing_site_info: false,
            },
            OmniboxPlacement::Sidebar => Self {
                height: metrics::SIDEBAR_OMNIBOX_HEIGHT,
                radius: metrics::sidebar_omnibox_radius(),
                font_size: metrics::SIDEBAR_OMNIBOX_FONT_SIZE,
                idle_icon: false,
                center_text: true,
                trailing_site_info: true,
            },
        }
    }
}

pub(super) struct Omnibox {
    input: Entity<EditableTextState>,
    /// The active tab's web view; `None` while it shows one of Photon's pages.
    webview: Option<Entity<PhotonWebView>>,
    /// The address of the active tab's Photon page, if it shows one.
    page_address: Option<String>,
    /// The address the field shows for the page: short while idle, full
    /// while editing. Text that differs from it is an edit.
    current_url: String,
    /// The submitted text cannot be opened. Cleared by the next edit.
    invalid: bool,
    site_info_open: bool,
    /// The GPUI popover shows the connection's page rather than the site's.
    site_info_security: bool,
    /// Keeps the site information popover drawn while it animates away.
    site_info_presence: super::motion::Presence<()>,
    field_hovered: bool,
    /// The surface beneath the field while it floats over the page, as in
    /// the sidebar shown over it, so its glass does not show the page.
    floating_surface: Option<u32>,
    /// The native site information popover; GPUI draws it where this is absent.
    #[cfg(target_os = "macos")]
    site_info_native: Option<crate::platform::AppKitSiteInfoPopover>,
    address_text_offset: Tween,
    address_text_opacity: Tween,
    address_text_position_initialized: bool,
    address_text_editing: bool,
    /// What the typed text could open. While there are any, the field opens
    /// into a panel listing them.
    suggestions: Vec<Suggestion>,
    /// The current background history query, canceled when input changes.
    suggestion_task: Option<Task<()>>,
    suggestion_generation: u64,
    /// The history the suggestions were made from. Icon loads update the
    /// history global without changing it, and must not suggest again.
    suggested_from: Option<Arc<History>>,
    /// The suggestion Enter opens.
    selected: usize,
    /// The suggestion under the pointer.
    hovered: Option<usize>,
    /// The text as typed, without an inline completion.
    typed: String,
    /// The first suggestion's address, which continues the typed text.
    completion: Option<String>,
    /// Text the omnibox put in the field itself, which is not an edit.
    filled: Option<String>,
    /// The next edit removes text, so it is not completed again.
    deleting: bool,
    _input_subscriptions: [Subscription; 3],
    _history_subscription: Subscription,
    _keystroke_subscription: Subscription,
    _page_subscriptions: Vec<Subscription>,
}

/// What the omnibox asks its window to do.
pub(super) enum OmniboxEvent {
    /// Load this address in the active tab.
    Navigate(String),
}

impl EventEmitter<OmniboxEvent> for Omnibox {}

impl Omnibox {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let input_subscriptions = [
            cx.subscribe(&input, |this, _, _: &TextChanged, cx| this.text_changed(cx)),
            // The field lives in the cached chrome, which redraws only when a
            // view inside it is notified, so redraw for every edit, caret move
            // and blink.
            cx.observe(&input, |_, _, cx| cx.notify()),
            // The idle address follows the full address setting.
            cx.observe_global_in::<Settings>(window, |this, window, cx| {
                if !this.is_editing(window, cx) {
                    this.show_page_url(cx);
                }
            }),
        ];
        let history_subscription =
            cx.observe_global::<BrowsingHistory>(|this, cx| this.history_changed(cx));
        let this = cx.entity().downgrade();
        let keystroke_subscription = cx.intercept_keystrokes(move |event, window, cx| {
            this.update(cx, |this, cx| {
                if !this.is_editing(window, cx) {
                    return;
                }

                let backspace =
                    event.keystroke.key == "backspace" && !event.keystroke.modifiers.modified();
                if backspace && this.dismiss_completion(cx) {
                    cx.stop_propagation();
                    return;
                }

                if backspace && !this.suggestions.is_empty() {
                    // Mark this before EditableText dispatches DeleteLeft. This
                    // keeps the suggestion rows but prevents inline completion
                    // from being filled again after deletion.
                    this.deleting = true;
                }

                if event.keystroke.modifiers.modified() {
                    return;
                }

                let handled = match event.keystroke.key.as_str() {
                    "up" if !this.suggestions.is_empty() => {
                        this.select(this.selected.saturating_sub(1), cx);
                        true
                    }
                    "down" if !this.suggestions.is_empty() => {
                        this.select((this.selected + 1).min(this.suggestions.len() - 1), cx);
                        true
                    }
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            })
            .ok();
        });
        let mut omnibox = Self {
            input,
            webview: None,
            page_address: None,
            current_url: String::new(),
            invalid: false,
            site_info_open: false,
            site_info_security: false,
            site_info_presence: Default::default(),
            field_hovered: false,
            floating_surface: None,
            #[cfg(target_os = "macos")]
            site_info_native: {
                let toggle = cx.entity().downgrade();
                let close = toggle.clone();
                crate::platform::AppKitSiteInfoPopover::new(
                    cx.to_async(),
                    move |control, cx| {
                        toggle
                            .update(cx, |this, cx| this.toggle_site_control(control, cx))
                            .ok();
                    },
                    move |cx| {
                        close.update(cx, |this, cx| this.close_site_info(cx)).ok();
                    },
                )
            },
            address_text_offset: Tween::at(0.0),
            address_text_opacity: Tween::at(1.0),
            address_text_position_initialized: false,
            address_text_editing: false,
            suggestions: Vec::new(),
            suggestion_task: None,
            suggestion_generation: 0,
            suggested_from: None,
            selected: 0,
            hovered: None,
            typed: String::new(),
            completion: None,
            filled: None,
            deleting: false,
            _input_subscriptions: input_subscriptions,
            _history_subscription: history_subscription,
            _keystroke_subscription: keystroke_subscription,
            _page_subscriptions: Vec::new(),
        };
        omnibox.set_content(None, None, window, cx);
        omnibox
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    fn address_text_motion(
        &mut self,
        centered_sidebar: bool,
        editing: bool,
        resting_offset: f32,
        window: &mut Window,
        cx: &gpui::App,
    ) -> (f32, f32) {
        if !centered_sidebar {
            self.address_text_offset.jump_to(0.0);
            self.address_text_opacity.jump_to(1.0);
            self.address_text_position_initialized = false;
            self.address_text_editing = false;
            return (0.0, 1.0);
        }

        let motion = Settings::motion(cx);
        let target = if editing { 0.0 } else { resting_offset };
        if !self.address_text_position_initialized {
            self.address_text_offset.jump_to(target);
            self.address_text_position_initialized = true;
        } else if self.address_text_editing != editing {
            self.address_text_offset
                .animate_to(target, Speed::Gentle, &motion);
            self.address_text_opacity
                .jump_to(metrics::OMNIBOX_TEXT_TRANSITION_OPACITY);
            self.address_text_opacity
                .animate_to(1.0, Speed::Gentle, &motion);
        } else if self.address_text_offset.target() != target {
            self.address_text_offset.jump_to(target);
        }
        self.address_text_editing = editing;

        let now = std::time::Instant::now();
        let offset = self.address_text_offset.value_with_config(now, &motion);
        let opacity = self.address_text_opacity.value_with_config(now, &motion);
        if self
            .address_text_offset
            .is_running_with_config(now, &motion)
            || self
                .address_text_opacity
                .is_running_with_config(now, &motion)
        {
            window.request_animation_frame();
        }

        (offset, opacity)
    }

    /// Shows the active tab's address: its web view's, followed as it
    /// changes, or `page_address` for one of Photon's pages, which is `None`
    /// when the page leaves the field empty.
    /// Sets the surface the field rests on while it floats over the page.
    pub(super) fn set_floating_surface(&mut self, surface: Option<u32>) {
        self.floating_surface = surface;
    }

    pub(super) fn set_content(
        &mut self,
        webview: Option<Entity<PhotonWebView>>,
        page_address: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.webview = webview;
        self.page_address = page_address;
        self.close_suggestions(cx);
        self.site_info_open = false;
        self.invalid = false;
        let input_focus = self.input.focus_handle(cx);
        self._page_subscriptions = vec![
            // An abandoned edit reverts to the page address.
            cx.on_blur(&input_focus, window, |this, _, cx| this.show_page_url(cx)),
            // Editing starts from the full address.
            cx.on_focus(&input_focus, window, |this, _, cx| this.show_full_url(cx)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
        ];
        if let Some(webview) = self.webview.as_ref() {
            self._page_subscriptions.push(cx.subscribe_in(
                webview,
                window,
                |this, _, _: &WebViewEvent, window, cx| {
                    if !this.is_editing(window, cx) {
                        this.show_page_url(cx);
                    }
                },
            ));
        }
        self.show_page_url(cx);
        cx.notify();
    }

    fn is_editing(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }

    /// The active tab's full address.
    fn page_url(&self, cx: &Context<Self>) -> String {
        match self.webview.as_ref() {
            Some(webview) => webview.read(cx).omnibox_url(),
            None => self.page_address.clone().unwrap_or_default(),
        }
    }

    /// Shows the page's address the short way while nobody is editing it:
    /// just its site, as in Arc, or as people write it when the full address
    /// is chosen in settings.
    fn show_page_url(&mut self, cx: &mut Context<Self>) {
        let url = self.page_url(cx);
        let shown = if self.webview.is_none() {
            url
        } else if Settings::get(cx).show_full_address {
            display_address(&url)
        } else {
            display_domain(&url)
        };
        if self.current_url != shown || self.input.read(cx).as_str() != shown {
            self.show(shown, cx);
            cx.notify();
        }
    }

    /// Shows the full address, selected, as editing starts.
    fn show_full_url(&mut self, cx: &mut Context<Self>) {
        let url = self.page_url(cx);
        self.show(url, cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    fn show(&mut self, text: String, cx: &mut Context<Self>) {
        self.current_url = text.clone();
        self.input.update(cx, |input, cx| {
            if input.as_str() != text {
                input.emplace(&text, cx);
            }
        });
    }

    fn submit(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        // Rows still showing for earlier text would open the wrong thing.
        if !self.suggestions.is_empty() && self.suggestion_task.is_none() {
            self.choose(self.selected, window, cx);
            return;
        }
        let text = self.input.read(cx).as_str().to_owned();
        if text.trim().is_empty() {
            return;
        }
        self.open(&text, window, cx);
    }

    /// Resolves typed text and asks the browser window to open its URL.
    fn open(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        match resolve_with(text, &Settings::search_engines(cx)) {
            Ok(target) => {
                if let OmniboxTarget::Search { query, .. } = &target {
                    BrowsingHistory::record_search(query, cx);
                }
                self.close_suggestions(cx);
                cx.emit(OmniboxEvent::Navigate(target.url().to_owned()));
                self.return_to_page(window, cx);
            }
            Err(_) => {
                self.invalid = true;
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.close_suggestions(cx);
        self.return_to_page(window, cx);
    }

    fn return_to_page(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(webview) = self.webview.as_ref() {
            let focus_handle = webview.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
        }
    }
}

impl Render for Omnibox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_palette = palette(window, cx);
        let palette = super::theme::chrome_palette(window, cx);
        let editing = self.is_editing(window, cx);
        let workspace_layout = WorkspaceLayout::for_window(window, cx);
        let style = FieldStyle::for_layout(workspace_layout.omnibox_placement());
        let centered_sidebar = style.center_text && self.webview.is_some();
        let resting_offset = if centered_sidebar {
            let sidebar_width = workspace_layout
                .sidebar_extent()
                .unwrap_or_else(|| Settings::get(cx).sidebar.width())
                as f32;
            let field_width = sidebar_width - 2.0 * metrics::SIDEBAR_PADDING;
            let text_width = window
                .text_system()
                .layout_line(
                    self.current_url.as_str(),
                    px(style.font_size),
                    &[gpui::TextRun {
                        len: self.current_url.len(),
                        ..Default::default()
                    }],
                )
                .width;
            (field_width / 2.0 - f32::from(text_width) / 2.0 - metrics::OMNIBOX_HORIZONTAL_PADDING)
                .max(0.0)
        } else {
            0.0
        };
        let (address_text_offset, address_text_opacity) =
            self.address_text_motion(centered_sidebar, editing, resting_offset, window, cx);
        let motion = Settings::motion(cx);
        // At rest the glass alone is the field's surface; hover and editing
        // tint it. Without glass the field paints its own surface.
        let surface = if editing || self.field_hovered {
            palette.surface_hover
        } else if glass_available(cx) {
            0x0000_0000
        } else {
            palette.surface
        };
        let field_box = glass("titlebar-omnibox")
            .interactive(true)
            .window_tint(menu_palette)
            .backing(self.floating_surface)
            .flex()
            .flex_row()
            .relative()
            .items_center()
            .flex_1()
            .min_w_0()
            .h(px(style.height))
            .rounded(px(style.radius))
            .bg(rgba(surface))
            .text_size(px(style.font_size))
            .text_color(rgb(palette.text_primary))
            // Clicks on the field's padding or icon edit the address rather than
            // falling through to the titlebar and moving the window.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    window.focus(&this.input.focus_handle(cx), cx);
                }),
            )
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.field_hovered != *hovered {
                    this.field_hovered = *hovered;
                    cx.notify();
                }
            }))
            // Nor does a right-click reach the sidebar's browser menu.
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::cancel));
        let mut field_box = self.handle_editing_keys(field_box, cx);
        // The open panel draws the field's content itself, in the same place.
        field_box = if editing && !self.suggestions.is_empty() {
            field_box.child(self.open_panel(
                menu_palette,
                style,
                workspace_layout.sidebar_region(),
                address_text_offset,
                address_text_opacity,
                cx,
            ))
        } else {
            field_box.child(self.field_content(
                palette,
                editing,
                style,
                address_text_offset,
                address_text_opacity,
                cx,
            ))
        };
        field_box =
            field_box.child(self.site_info_popover(menu_palette, style, &workspace_layout, cx));
        if self.invalid {
            field_box = field_box.aria_description(INVALID_ADDRESS_DESCRIPTION);
        }
        gpui_motion::MotionElement::new(
            "omnibox-field-surface-motion",
            field_box,
            gpui_motion::Motion::new(gpui_motion::Target::default().opacity(1.0)),
        )
        .style_motion(
            gpui_motion::StyleMotion::new(gpui_motion::StyleTarget::default().color(
                gpui_motion::ColorProperty::Background,
                super::motion::motion_rgba_color(surface),
            ))
            .transition(super::motion::Curve::EaseOut.transition(motion.duration(Speed::Quick))),
        )
        .into_any_element()
    }
}
