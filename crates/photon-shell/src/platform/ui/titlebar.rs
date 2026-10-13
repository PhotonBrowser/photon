//! Titlebar space: moves or zooms the window from empty space, like a native
//! titlebar, around the shell UI it holds.

use gpui::{Div, InteractiveElement, MouseButton, Window, prelude::*, px};

use super::layout::h_stack;
use crate::platform::window_settings;

use super::{metrics, theme::ThemeColors, window_controls};

/// A centered titlebar strip, with room for window controls while windowed.
pub(super) fn titlebar(content: impl IntoElement, window: &Window, palette: ThemeColors) -> Div {
    let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();
    let leading_inset = if fullscreen {
        0.0
    } else {
        window_settings::native_controls_inset()
    };
    let trailing_inset = if fullscreen {
        0.0
    } else {
        window_settings::native_controls_trailing_inset()
    };
    let show_custom_controls = !fullscreen && window_settings::uses_custom_window_controls();
    let area = h_stack()
        .relative()
        .w_full()
        .h(px(metrics::TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .pl(px(leading_inset))
        .pr(px(trailing_inset))
        .child(content)
        .when(show_custom_controls, |area| {
            area.child(window_controls::controls(window, palette))
        });
    window_drag_area(area)
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
