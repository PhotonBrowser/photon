//! The titlebar strip: hosts shell UI between the native window controls and
//! moves the window from its empty space.

use gpui::{Div, MouseButton, prelude::*, px};

use super::layout::h_stack;
use super::theme::metrics;

/// Interactive titlebar content must stop left mouse-down propagation, so only
/// clicks on empty space reach the titlebar and move or zoom the window.
pub(super) fn titlebar(content: impl IntoElement) -> Div {
    h_stack()
        .w_full()
        .h(px(metrics::TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .px(px(metrics::WINDOW_CONTROLS_INSET))
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            if event.click_count == 2 {
                window.titlebar_double_click();
            } else {
                window.start_window_move();
            }
        })
        .child(content)
}
