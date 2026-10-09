//! Non-blocking notice for a recovered WebContent process crash.

use gpui::{Role, div, prelude::*, px, rgb, rgba};

use super::ClickHandler;
use super::button::button;
use super::layout::{h_stack, v_stack};
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
                .gap(px(metrics::BUTTON_GAP))
                .child(button(
                    "crash-alert-reload",
                    "Reload page",
                    true,
                    palette,
                    on_reload,
                ))
                .child(button(
                    "crash-alert-dismiss",
                    "Dismiss",
                    false,
                    palette,
                    on_dismiss,
                )),
        )
}
