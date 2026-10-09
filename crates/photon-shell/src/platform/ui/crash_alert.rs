//! Non-blocking notice for a recovered WebContent process crash.

use gpui::{MouseButton, Role, div, prelude::*, px, rgb, rgba};

use super::layout::{h_stack, v_stack};
use super::toolbar::ClickHandler;
use super::{metrics, theme::ThemeColors};

pub(super) fn crash_alert(
    palette: ThemeColors,
    on_reload: ClickHandler,
    on_dismiss: ClickHandler,
) -> impl IntoElement {
    v_stack()
        .id("engine-crash-alert")
        .role(Role::Alert)
        .aria_label("Page crash notification")
        .aria_live(gpui::accesskit::Live::Assertive)
        .aria_live_atomic(true)
        .absolute()
        .right(px(metrics::CRASH_ALERT_INSET))
        .bottom(px(metrics::CRASH_ALERT_INSET))
        .max_w(px(metrics::CRASH_ALERT_MAX_WIDTH))
        .gap(px(metrics::CRASH_ALERT_GAP))
        .p(px(metrics::CRASH_ALERT_PADDING))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .text_color(rgb(palette.text_primary))
        .child(
            div()
                .text_size(px(metrics::CRASH_ALERT_TITLE_SIZE))
                .child("Page crashed"),
        )
        .child(
            div()
                .text_size(px(metrics::MENU_FONT_SIZE))
                .text_color(rgb(palette.text_secondary))
                .child("Photon restarted the page process and tried to reload this page."),
        )
        .child(
            h_stack()
                .items_center()
                .justify_end()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(alert_button(
                    "crash-alert-reload",
                    "Reload page",
                    true,
                    palette,
                    on_reload,
                ))
                .child(alert_button(
                    "crash-alert-dismiss",
                    "Dismiss",
                    false,
                    palette,
                    on_dismiss,
                )),
        )
}

fn alert_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let mut button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .h(px(metrics::MENU_ITEM_HEIGHT))
        .items_center()
        .justify_center()
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .rounded(px(metrics::MENU_ITEM_RADIUS))
        .text_size(px(metrics::MENU_FONT_SIZE))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(label);

    if primary {
        button = button
            .bg(rgba(palette.selection))
            .text_color(rgb(palette.text_primary));
    } else {
        button = button
            .text_color(rgb(palette.text_secondary))
            .hover(|style| style.bg(rgba(palette.menu_hover)));
    }

    button
}
