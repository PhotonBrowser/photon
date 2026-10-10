//! Titlebar space: moves or zooms the window from empty space, like a native
//! titlebar, around the shell UI it holds.

use gpui::{Div, InteractiveElement, MouseButton, Window, prelude::*, px};

use super::layout::h_stack;
use super::metrics;

/// A centered titlebar strip, with room for native controls while windowed.
pub(super) fn titlebar(content: impl IntoElement, window: &Window) -> Div {
    let controls_inset = if window.is_fullscreen() || window.is_simple_fullscreen() {
        0.0
    } else {
        metrics::WINDOW_CONTROLS_INSET
    };
    window_drag_area(
        h_stack()
            .w_full()
            .h(px(metrics::TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .px(px(controls_inset)),
    )
    .child(content)
}

/// Makes empty space in `area` move the window, and a double-click zoom it.
/// Interactive content inside must stop left mouse-down propagation, so only
/// clicks on empty space reach the area.
pub(super) fn window_drag_area<E: InteractiveElement>(area: E) -> E {
    area.on_mouse_down(MouseButton::Left, |event, window, _| {
        if event.click_count == 2 {
            window.titlebar_double_click();
        } else {
            window.start_window_move();
        }
    })
}
