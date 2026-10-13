//! Client-side window controls, shown only by targets that draw their own.

use gpui::{Div, IntoElement, ParentElement, Styled, Window, px};

use super::button::toolbar_button;
use super::icons::{close_icon, maximize_window_icon, minimize_window_icon, restore_window_icon};
use super::layout::h_stack;
use super::{ClickHandler, metrics, theme::ThemeColors};

/// The Linux titlebar's minimize, maximize/restore, and close buttons.
pub(super) fn controls(window: &Window, palette: ThemeColors) -> Div {
    let color = palette.text_secondary;
    let icon_size = metrics::TOOLBAR_ICON_SIZE;
    let minimized = window.window_controls().minimize;
    let maximized = window.window_controls().maximize;
    let (maximize_label, maximize_icon) = if window.is_maximized() {
        (
            "Restore window",
            restore_window_icon(color, icon_size).into_any_element(),
        )
    } else {
        (
            "Maximize window",
            maximize_window_icon(color, icon_size).into_any_element(),
        )
    };
    let minimize: ClickHandler = Box::new(|_, window, _| window.minimize_window());
    let zoom: ClickHandler = Box::new(|_, window, _| window.zoom_window());
    let close: ClickHandler = Box::new(|_, window, _| window.remove_window());

    h_stack()
        .absolute()
        .top_0()
        .right(px(metrics::WINDOW_CONTROL_EDGE_INSET))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .items_center()
        .gap(px(metrics::WINDOW_CONTROL_GAP))
        .child(toolbar_button(
            "window-minimize",
            "Minimize window",
            0,
            minimized,
            minimize_window_icon(color, icon_size),
            palette,
            minimize,
        ))
        .child(toolbar_button(
            "window-maximize",
            maximize_label,
            0,
            maximized,
            maximize_icon,
            palette,
            zoom,
        ))
        .child(toolbar_button(
            "window-close",
            "Close window",
            0,
            true,
            close_icon(color, icon_size),
            palette,
            close,
        ))
}
