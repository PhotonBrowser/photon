//! The field's content, and the panel it opens into while typing.

use crate::platform::site_info::SiteControl;
use gpui::{AnyElement, Context, Focusable, deferred, div, prelude::*, px, rgb, rgba};
use gpui_elements::editable_text::text_input;
use photon_core::DockRegion;
use std::rc::Rc;

use super::super::button::toolbar_button;
use super::super::controls::OwnsClicks;
use super::super::controls::themed_text_input;
use super::super::icons::{search_icon_sized, site_controls_icon};
use super::super::layout::{Elevated, Elevation, Raised};
use super::super::layout::{h_stack, v_stack};
use super::super::menu::MENU_MOTION;
use super::super::motion::{AnimateIn, fade, popover, translated};
use super::super::workspace_layout::WorkspaceLayout;
use super::super::{metrics, theme::ThemeColors};
use super::rows::{SuggestionActions, icon_slot, suggestion_icon, suggestion_rows};
use super::{FieldStyle, Omnibox};

impl Omnibox {
    /// The icon and text field, laid out identically whether the field is
    /// closed or open, so the text does not move as it opens.
    pub(super) fn field_content(
        &self,
        palette: ThemeColors,
        editing: bool,
        style: FieldStyle,
        address_text_offset: f32,
        address_text_opacity: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let input_focus = self.input.focus_handle(cx).tab_index(3).tab_stop(true);
        let centered_sidebar = style.center_text && self.webview.is_some();
        let icon = (!style.trailing_site_info && (style.idle_icon || !self.suggestions.is_empty()))
            .then(|| self.field_icon(palette, cx));
        let site_info_open = self.site_info_open;
        let input = themed_text_input(text_input("titlebar-omnibox-input"), palette)
            .state(self.input.downgrade())
            .track_focus(&input_focus)
            .aria_label("Search or enter address")
            .placeholder("Search or enter address")
            .placeholder_color(rgb(palette.text_secondary))
            .flex_1()
            .min_w_0()
            .whitespace_nowrap()
            .when(centered_sidebar, |input| input.text_start());
        let input = if centered_sidebar && !editing {
            input.text_ellipsis()
        } else {
            input.overflow_x_scroll()
        };
        let icon_color = if site_info_open {
            palette.text_primary
        } else {
            palette.text_secondary
        };
        // The icon is a rounded square of its own, so a hover surface around it
        // would draw a box in a box: the icon brightens instead.
        let site_info_button = toolbar_button(
            "site-info-button",
            "Site information",
            0,
            true,
            div()
                .group_hover_style("site-info-button", |style| {
                    style.text_color(rgb(palette.text_primary))
                })
                .child(site_controls_icon(
                    icon_color,
                    metrics::SITE_INFO_BUTTON_ICON_SIZE,
                )),
            palette,
            Box::new(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    this.toggle_site_info(event.position(), window, cx);
                }),
            ),
        )
        .group("site-info-button")
        .hover_style(|style| style)
        .active_style(|style| style)
        .aria_expanded(site_info_open);
        let site_info_button = if style.trailing_site_info {
            site_info_button
                .size(px(metrics::SIDEBAR_OMNIBOX_BUTTON_SIZE))
                .rounded(px(metrics::sidebar_omnibox_button_radius()))
        } else {
            site_info_button
        };
        // The page's address rests quieter than text being edited.
        let input = input
            .text_color(rgb(if editing {
                palette.text_primary
            } else {
                palette.text_secondary
            }))
            .into_any_element();
        let input = if centered_sidebar {
            h_stack()
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .items_center()
                .overflow_x_hidden()
                .opacity(address_text_opacity)
                .child(translated(
                    input,
                    gpui::point(px(address_text_offset), px(0.0)),
                ))
                .into_any_element()
        } else {
            input
        };
        let row = h_stack()
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .w_full()
            .flex_shrink_0()
            .h(px(style.height - 2.0 * metrics::OMNIBOX_BORDER_WIDTH));

        if centered_sidebar {
            // The button sits as far from the end as from the top and bottom.
            let row = row
                .relative()
                .pl(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .pr(px(metrics::SIDEBAR_OMNIBOX_BUTTON_INSET))
                .child(
                    h_stack()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .items_center()
                        .child(input),
                )
                .child(site_info_button);
            if let Some(icon) = icon {
                row.child(
                    gpui::div()
                        .absolute()
                        .left(px(metrics::SIDEBAR_PADDING))
                        .top(px((style.height
                            - 2.0 * metrics::OMNIBOX_BORDER_WIDTH
                            - metrics::OMNIBOX_ICON_SIZE)
                            / 2.0))
                        .child(icon),
                )
            } else {
                row
            }
        } else if style.trailing_site_info {
            // The button sits as far from the end as from the top and bottom.
            row.pl(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .pr(px(metrics::SIDEBAR_OMNIBOX_BUTTON_INSET))
                .children(icon)
                .child(input)
                .child(site_info_button)
        } else {
            row.px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .child(site_info_button)
                .children(icon)
                .child(input)
        }
    }

    /// Opens or closes the popover: AppKit's own on macOS, GPUI's elsewhere.
    fn toggle_site_info(
        &mut self,
        anchor: gpui::Point<gpui::Pixels>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(target_os = "macos")]
        if let Some(popover) = &self.site_info_native {
            let guard = std::time::Duration::from_millis(metrics::POPOVER_REOPEN_GUARD_MS);
            if popover.is_shown() {
                popover.close();
            } else if !popover.closed_within(guard) {
                popover.update(&self.site_info(cx));
                popover.show(window, anchor);
            }
            return;
        }
        // Only the native popover needs to know where it was opened from.
        #[cfg(not(target_os = "macos"))]
        let _ = (anchor, window);
        self.site_info_open = !self.site_info_open;
        self.site_info_security = false;
        cx.notify();
    }

    pub(super) fn site_info_popover(
        &mut self,
        palette: ThemeColors,
        style: FieldStyle,
        workspace_layout: &WorkspaceLayout,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // The native popover follows the site as the field redraws.
        #[cfg(target_os = "macos")]
        if let Some(popover) = &self.site_info_native {
            if popover.is_shown() {
                popover.update(&self.site_info(cx));
            }
            return deferred(gpui::div()).priority(1);
        }
        {
            let motion = super::super::settings::Settings::motion(cx);
            let transition = self
                .site_info_presence
                .sync(self.site_info_open.then_some(()), MENU_MOTION, &motion, cx)
                .map(|((), transition)| transition);
            // The site's details are read only while the popover is drawn, not on
            // every caret blink of the field.
            let content = transition.map(|transition| {
                super::site_info::popover(
                    self.site_info(cx),
                    self.site_info_security,
                    self.site_info_actions(cx),
                    metrics::SITE_INFO_WIDTH,
                    transition,
                    palette,
                    cx,
                )
                .into_any_element()
            });
            let opens_toward_page =
                workspace_layout.sidebar_region() != Some(photon_core::DockRegion::Trailing);
            // Like a context menu, the popover opens from the button, over the
            // page rather than within the narrow sidebar.
            let button_inset =
                metrics::SIDEBAR_OMNIBOX_BUTTON_INSET + metrics::OMNIBOX_BORDER_WIDTH;
            let button_start = if style.trailing_site_info {
                let sidebar_width = workspace_layout
                    .sidebar_extent()
                    .unwrap_or_else(|| super::super::settings::Settings::get(cx).sidebar.width())
                    as f32;
                sidebar_width
                    - 2.0 * metrics::SIDEBAR_PADDING
                    - button_inset
                    - metrics::SIDEBAR_OMNIBOX_BUTTON_SIZE
            } else {
                metrics::OMNIBOX_HORIZONTAL_PADDING
            };
            deferred(
                gpui::div()
                    .absolute()
                    .when(opens_toward_page || !style.trailing_site_info, |popover| {
                        popover.left(px(button_start))
                    })
                    .when(style.trailing_site_info && !opens_toward_page, |popover| {
                        popover.right(px(button_inset))
                    })
                    .top(px(style.height + metrics::MENU_ITEM_GAP))
                    // Only the open popover takes clicks; a leaving one lets them through.
                    .when(self.site_info_open, |popover| {
                        popover
                            .occlude()
                            .on_mouse_down_out(
                                cx.listener(|this, _, _, cx| this.close_site_info(cx)),
                            )
                            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                                if event.keystroke.key == "escape" {
                                    cx.stop_propagation();
                                    this.close_site_info(cx);
                                }
                            }))
                    })
                    .children(content),
            )
            .priority(1)
        }
    }

    pub(super) fn close_site_info(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if let Some(popover) = &self.site_info_native {
            popover.close();
        }
        if self.site_info_open {
            self.site_info_open = false;
            cx.notify();
        }
    }

    fn site_info(&self, cx: &Context<Self>) -> crate::platform::site_info::SiteInfo {
        let webview = self.webview.as_ref().map(|webview| webview.read(cx));
        crate::platform::site_info::SiteInfo {
            site: Some(photon_core::display_domain(&self.page_url(cx)))
                .filter(|site| !site.is_empty())
                .unwrap_or_else(|| "Photon".to_owned()),
            connection_secure: webview.and_then(|webview| webview.state.connection_secure),
            sound_on: webview.map(|webview| !webview.audio_muted),
            microphone_on: webview.and_then(|webview| webview.microphone_allowed_here()),
        }
    }

    fn site_info_actions(&self, cx: &mut Context<Self>) -> super::site_info::SiteInfoActions {
        let this = cx.entity().downgrade();
        let page = |security| -> super::super::ClickHandler {
            let this = this.clone();
            Box::new(move |_, _, cx| {
                this.update(cx, |this, cx| {
                    this.site_info_security = security;
                    cx.notify();
                })
                .ok();
            })
        };
        let toggle = |control| -> super::super::ClickHandler {
            let this = this.clone();
            Box::new(move |_, _, cx| {
                this.update(cx, |this, cx| this.toggle_site_control(control, cx))
                    .ok();
            })
        };
        super::site_info::SiteInfoActions {
            toggle_microphone: toggle(SiteControl::Microphone),
            toggle_sound: toggle(SiteControl::Sound),
            show_security: page(true),
            back: page(false),
            close: Box::new(move |_, _, cx| {
                this.update(cx, |this, cx| this.close_site_info(cx)).ok();
            }),
        }
    }

    /// Turns one of the site's controls on or off, from either popover.
    pub(super) fn toggle_site_control(&mut self, control: SiteControl, cx: &mut Context<Self>) {
        if let Some(webview) = self.webview.clone() {
            webview.update(cx, |webview, cx| match control {
                SiteControl::Microphone => {
                    let allowed = webview.microphone_allowed_here() == Some(true);
                    webview.set_microphone_allowed_here(!allowed, cx);
                }
                SiteControl::Sound => webview.toggle_audio_mute(cx),
                SiteControl::Camera | SiteControl::PopOut => {}
            });
        }
        cx.notify();
    }

    /// The highlighted suggestion's icon while suggesting, otherwise a search icon.
    fn field_icon(&self, palette: ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        match self.suggestions.get(self.selected) {
            Some(suggestion) => suggestion_icon(suggestion, palette, cx),
            None => icon_slot(search_icon_sized(
                palette.text_primary,
                metrics::OMNIBOX_ICON_SIZE,
            )),
        }
    }

    /// The field opened into a panel over the page: its content in the same
    /// place, with the suggestions under it.
    pub(super) fn open_panel(
        &self,
        palette: ThemeColors,
        style: FieldStyle,
        sidebar_region: Option<DockRegion>,
        address_text_offset: f32,
        address_text_opacity: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = suggestion_rows(
            &self.suggestions,
            self.selected,
            self.hovered,
            palette,
            &self.suggestion_actions(cx),
            cx,
        );
        // Line up with the closed field, whose border the panel replaces.
        let border = px(-metrics::OMNIBOX_BORDER_WIDTH);
        let panel = v_stack().id("omnibox-panel").absolute().top(border);
        // The sidebar is narrow, so grow suggestions toward the page.
        let panel = match (style.trailing_site_info, sidebar_region) {
            (true, Some(DockRegion::Trailing)) => panel.right(border).w_full(),
            (true, _) => panel.left(border).w_full(),
            (false, _) => panel.left(border).right(border),
        }
        .min_w(px(metrics::OMNIBOX_PANEL_MIN_WIDTH));
        deferred(
            panel
                .rounded_t(px(style.radius))
                .rounded_b(px(metrics::omnibox_panel_bottom_radius()))
                .border_1()
                .border_color(rgba(palette.menu_border))
                .raised(palette)
                .elevated(Elevation::High)
                .occlude()
                .owns_clicks()
                .child(self.field_content(
                    palette,
                    true,
                    style,
                    address_text_offset,
                    address_text_opacity,
                    cx,
                ))
                .child(rows.animate_in(
                    "omnibox-suggestions-appear",
                    fade(),
                    &super::super::settings::Settings::motion(cx),
                ))
                .animate_in(
                    "omnibox-panel-appear",
                    popover(),
                    &super::super::settings::Settings::motion(cx),
                ),
        )
        .priority(1)
    }

    fn suggestion_actions(&self, cx: &mut Context<Self>) -> SuggestionActions {
        let this = cx.entity().downgrade();
        let (choose, remove, hover) = (this.clone(), this.clone(), this);
        SuggestionActions {
            choose: Rc::new(move |index, window, cx| {
                choose
                    .update(cx, |this, cx| this.choose(index, window, cx))
                    .ok();
            }),
            remove: Rc::new(move |index, _, cx| {
                remove.update(cx, |this, cx| this.remove(index, cx)).ok();
            }),
            hover: Rc::new(move |index, hovered, _, cx| {
                hover
                    .update(cx, |this, cx| this.hover(index, hovered, cx))
                    .ok();
            }),
        }
    }
}
