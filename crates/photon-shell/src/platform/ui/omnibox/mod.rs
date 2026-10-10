//! The address and search field in the browser toolbar. While typing, it
//! opens into a panel of suggestions over the page.
//!
//! - `editing`: typing, inline completion, choosing and removing suggestions.
//! - `panel`: the field's content and the open panel.
//! - `rows`: the suggestion rows.

mod editing;
mod panel;
mod rows;

use gpui::{
    Context, Entity, EventEmitter, Focusable, MouseButton, Render, Subscription, Window,
    prelude::*, px, rgb, rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape},
};
use photon_core::Suggestion;
use photon_omnibox::{OmniboxTarget, resolve_with};

use super::history::BrowsingHistory;
use super::layout::h_stack;
use super::{PhotonWebView, WebViewEvent};
use super::{metrics, settings::Settings, theme::palette};

const INVALID_ADDRESS_DESCRIPTION: &str = "This address can't be opened";

pub(super) struct Omnibox {
    input: Entity<EditableTextState>,
    /// The active tab's web view; `None` while it shows one of Photon's pages.
    webview: Option<Entity<PhotonWebView>>,
    /// The address of the active tab's Photon page, if it shows one.
    page_address: Option<String>,
    /// The page address the field shows while nobody is editing it.
    current_url: String,
    /// The submitted text cannot be opened. Cleared by the next edit.
    invalid: bool,
    /// What the typed text could open. While there are any, the field opens
    /// into a panel listing them.
    suggestions: Vec<Suggestion>,
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
    _input_subscriptions: [Subscription; 2],
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
        ];
        let mut omnibox = Self {
            input,
            webview: None,
            page_address: None,
            current_url: String::new(),
            invalid: false,
            suggestions: Vec::new(),
            selected: 0,
            hovered: None,
            typed: String::new(),
            completion: None,
            filled: None,
            deleting: false,
            _input_subscriptions: input_subscriptions,
            _page_subscriptions: Vec::new(),
        };
        omnibox.set_content(None, None, window, cx);
        omnibox
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    /// Shows the active tab's address: its web view's, followed as it
    /// changes, or `page_address` for one of Photon's pages, which is `None`
    /// when the page leaves the field empty.
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
        self.invalid = false;
        let input_focus = self.input.focus_handle(cx);
        self._page_subscriptions = vec![
            // An abandoned edit reverts to the page address.
            cx.on_blur(&input_focus, window, |this, _, cx| this.show_page_url(cx)),
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

    fn show_page_url(&mut self, cx: &mut Context<Self>) {
        let url = match self.webview.as_ref() {
            Some(webview) => webview.read(cx).omnibox_url(),
            None => self.page_address.clone().unwrap_or_default(),
        };
        self.current_url = url.clone();
        self.input.update(cx, |input, cx| {
            if input.as_str() != url {
                input.emplace(&url, cx);
            }
        });
    }

    fn submit(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if !self.suggestions.is_empty() {
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
        let palette = palette(window, cx);
        let editing = self.is_editing(window, cx);
        let field = if editing {
            palette.field_focused
        } else {
            palette.field
        };
        let field_box = h_stack()
            .id("titlebar-omnibox")
            .relative()
            .items_center()
            .flex_1()
            .min_w_0()
            .h(px(metrics::OMNIBOX_HEIGHT))
            .rounded(px(metrics::OMNIBOX_RADIUS))
            .bg(rgba(field))
            .border_1()
            .border_color(if self.invalid {
                rgb_to_hsla(rgba(palette.field_error_border))
            } else {
                gpui::transparent_black()
            })
            .text_size(px(metrics::OMNIBOX_FONT_SIZE))
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
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::cancel));
        let mut field_box = self.handle_editing_keys(field_box, cx);
        // The open panel draws the field's content itself, in the same place.
        field_box = if editing && !self.suggestions.is_empty() {
            field_box.child(self.open_panel(palette, cx))
        } else {
            field_box.child(self.field_content(palette, cx))
        };
        if self.invalid {
            field_box = field_box.aria_description(INVALID_ADDRESS_DESCRIPTION);
        }
        field_box
    }
}
