//! Titlebar space: moves or zooms the window from empty space, like a native
//! titlebar, around the shell UI it holds.

use gpui::{Div, InteractiveElement, MouseButton, prelude::*, px};

use super::layout::h_stack;
use super::metrics;

/// A centered titlebar strip between the native window controls, for windows
/// without a sidebar.
pub(super) fn titlebar(content: impl IntoElement) -> Div {
    window_drag_area(
        h_stack()
            .w_full()
            .h(px(metrics::TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .px(px(metrics::WINDOW_CONTROLS_INSET)),
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
