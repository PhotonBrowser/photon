//! Read-only details the Engine reports for the active site.

use gpui::{Role, prelude::*, px, rgb};

use super::super::icons::shield_icon;
use super::super::layout::{h_stack, v_stack};
use super::super::menu::popover_surface;
use super::super::motion::Transition;
use super::super::{metrics, theme::ThemeColors};

pub(super) fn popover(
    site: String,
    connection_secure: Option<bool>,
    width: f32,
    palette: ThemeColors,
    cx: &gpui::App,
) -> impl IntoElement {
    let status = match connection_secure {
        Some(true) => "Connection is secure",
        Some(false) => "Connection is not secure",
        None => "Connection security unavailable",
    };
    let status_color = if connection_secure == Some(false) {
        palette.error
    } else {
        palette.text_primary
    };
    let content = v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(
            gpui::div()
                .w_full()
                .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                .py(px(metrics::MENU_ITEM_GAP))
                .text_size(px(metrics::BUTTON_FONT_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(palette.text_primary))
                .truncate()
                .child(site),
        )
        .child(
            h_stack()
                .items_center()
                .gap(px(metrics::MENU_ITEM_GAP))
                .w_full()
                .h(px(metrics::MENU_ITEM_HEIGHT))
                .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                .child(shield_icon(status_color, metrics::TOOLBAR_ICON_SIZE))
                .child(
                    gpui::div()
                        .flex_1()
                        .text_size(px(metrics::BUTTON_FONT_SIZE))
                        .text_color(rgb(status_color))
                        .child(status),
                ),
        );

    popover_surface(
        "Site information",
        Role::Dialog,
        width,
        content,
        Transition::Enter,
        palette,
        cx,
    )
}
