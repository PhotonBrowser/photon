//! The field's content, and the panel it opens into while typing.

use gpui::{AnyElement, Context, Focusable, deferred, prelude::*, px, rgb, rgba};
use gpui_elements::editable_text::text_input;
use photon_core::DockRegion;
use std::rc::Rc;

use super::super::button::toolbar_button;
use super::super::controls::OwnsClicks;
use super::super::controls::themed_text_input;
use super::super::icons::{search_icon_sized, shield_icon, sliders_horizontal_icon};
use super::super::layout::{Elevated, Elevation, Raised};
use super::super::layout::{h_stack, v_stack};
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
        let icon_color = if site_info_open && !style.trailing_site_info {
            palette.text_primary
        } else {
            palette.text_secondary
        };
        let site_info_button = toolbar_button(
            "site-info-button",
            "Site information",
            0,
            true,
            if style.trailing_site_info {
                sliders_horizontal_icon(palette.text_secondary, metrics::OMNIBOX_ICON_SIZE)
                    .into_any_element()
            } else {
                shield_icon(icon_color, metrics::OMNIBOX_ICON_SIZE).into_any_element()
            },
            palette,
            Box::new(cx.listener(move |this, _, _, cx| {
                this.site_info_open = !this.site_info_open;
                cx.notify();
            })),
        )
        .aria_expanded(site_info_open);
        let site_info_button = if style.trailing_site_info {
            site_info_button.rounded_r(px(metrics::sidebar_omnibox_button_radius()))
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
            let content = h_stack()
                .absolute()
                .left(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .right(px(metrics::SIDEBAR_OMNIBOX_TEXT_INSET))
                .top_0()
                .bottom_0()
                .items_center()
                .child(input);
            let row = row
                .relative()
                .child(content)
                .when(self.webview.is_some(), |row| {
                    row.child(
                        site_info_button
                            .absolute()
                            .right(px(metrics::SIDEBAR_OMNIBOX_BUTTON_INSET))
                            .top(px((style.height
                                - 2.0 * metrics::OMNIBOX_BORDER_WIDTH
                                - metrics::TOOLBAR_BUTTON_SIZE)
                                / 2.0)),
                    )
                });
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
            row.px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .children(icon)
                .child(input)
                .when(self.webview.is_some(), |row| row.child(site_info_button))
        } else {
            row.px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                .when(self.webview.is_some(), |row| row.child(site_info_button))
                .children(icon)
                .child(input)
        }
    }

    pub(super) fn site_info_popover(
        &self,
        palette: ThemeColors,
        style: FieldStyle,
        workspace_layout: &WorkspaceLayout,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let site = photon_core::display_domain(&self.page_url(cx));
        let secure = self
            .webview
            .as_ref()
            .and_then(|webview| webview.read(cx).state.connection_secure);
        let opens_toward_page =
            workspace_layout.sidebar_region() != Some(photon_core::DockRegion::Trailing);
        let popover_width = if style.trailing_site_info {
            let sidebar_width = workspace_layout
                .sidebar_extent()
                .unwrap_or_else(|| super::super::settings::Settings::get(cx).sidebar.width())
                as f32;
            metrics::SITE_INFO_WIDTH
                .min(sidebar_width - metrics::SIDEBAR_PADDING)
                .max(metrics::MENU_WIDTH)
        } else {
            metrics::SITE_INFO_WIDTH
        };
        deferred(
            gpui::div()
                .absolute()
                .when(style.trailing_site_info && opens_toward_page, |popover| {
                    popover.left_0()
                })
                .when(style.trailing_site_info && !opens_toward_page, |popover| {
                    popover.right_0()
                })
                .when(!style.trailing_site_info, |popover| popover.left_0())
                .top(px(style.height + metrics::MENU_ITEM_GAP))
                .when(self.site_info_open, |popover| {
                    popover
                        .occlude()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.site_info_open = false;
                            cx.notify();
                        }))
                        .child(super::site_info::popover(
                            site,
                            secure,
                            popover_width,
                            palette,
                            cx,
                        ))
                }),
        )
        .priority(1)
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
