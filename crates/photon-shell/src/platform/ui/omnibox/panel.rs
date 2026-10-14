//! The field's content, and the panel it opens into while typing.

use gpui::{AnyElement, Context, Focusable, deferred, prelude::*, px, rgb, rgba};
use gpui_elements::editable_text::text_input;
use std::rc::Rc;

use super::super::button::toolbar_button;
use super::super::controls::OwnsClicks;
use super::super::controls::themed_text_input;
use super::super::icons::{search_icon_sized, shield_icon, sliders_horizontal_icon};
use super::super::layout::{Elevated, Elevation, Raised};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance};
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
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let input_focus = self.input.focus_handle(cx).tab_index(3).tab_stop(true);
        let style = FieldStyle::current(cx);
        let icon = (!style.trailing_site_info && (style.idle_icon || !self.suggestions.is_empty()))
            .then(|| self.field_icon(palette, cx));
        let site_info_open = self.site_info_open;
        let centered_sidebar = style.center_text && self.webview.is_some();
        let input = themed_text_input(text_input("titlebar-omnibox-input"), palette)
            .state(self.input.downgrade())
            .track_focus(&input_focus)
            .aria_label("Search or enter address")
            .placeholder("Search or enter address")
            .placeholder_color(rgb(palette.text_secondary))
            .flex_1()
            .min_w_0()
            .whitespace_nowrap()
            .overflow_x_scroll();
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
        let row = h_stack()
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .w_full()
            .flex_shrink_0()
            .h(px(style.height - 2.0 * metrics::OMNIBOX_BORDER_WIDTH));

        if centered_sidebar {
            let content = h_stack()
                .absolute()
                .left(px(if editing {
                    metrics::OMNIBOX_HORIZONTAL_PADDING
                } else {
                    metrics::SIDEBAR_OMNIBOX_TEXT_INSET
                }))
                .right(px(metrics::SIDEBAR_OMNIBOX_TEXT_INSET))
                .top_0()
                .bottom_0()
                .items_center();
            let content = if editing {
                content.child(input).into_any_element()
            } else {
                content
                    .child(
                        gpui::div()
                            .w_full()
                            .text_center()
                            .truncate()
                            .child(self.current_url.clone()),
                    )
                    .into_any_element()
            };
            let row = row
                .relative()
                .child(content)
                .when(self.webview.is_some(), |row| {
                    row.child(
                        site_info_button
                            .absolute()
                            .right(px(metrics::SIDEBAR_PADDING))
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
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let site = photon_omnibox::display_domain(&self.page_url(cx));
        let secure = self
            .webview
            .as_ref()
            .and_then(|webview| webview.read(cx).state.connection_secure);
        let style = FieldStyle::current(cx);
        let workspace_layout = WorkspaceLayout::current(cx);
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
        deferred(
            v_stack()
                .id("omnibox-panel")
                .absolute()
                .top(border)
                .left(border)
                .right(border)
                // A narrow field opens into a panel that reaches over the page.
                .when(!FieldStyle::current(cx).trailing_site_info, |panel| {
                    panel.min_w(px(metrics::OMNIBOX_PANEL_MIN_WIDTH))
                })
                .rounded(px(FieldStyle::current(cx).radius))
                .border_1()
                .border_color(rgba(palette.menu_border))
                .raised(palette)
                .elevated(Elevation::High)
                .occlude()
                .owns_clicks()
                .child(self.field_content(palette, true, cx))
                .child(rows.animate_in(
                    "omnibox-suggestions-appear",
                    Entrance::fade(),
                    &super::super::settings::Settings::get(cx).animations,
                )),
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
            hover: Rc::new(move |index, _, cx| {
                hover.update(cx, |this, cx| this.hover(index, cx)).ok();
            }),
        }
    }
}
