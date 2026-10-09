//! Text buttons for alerts and dialogs.

use gpui::{MouseButton, Role, prelude::*, px, rgb, rgba};

use super::ClickHandler;
use super::layout::h_stack;
use super::{metrics, theme::ThemeColors};

/// A labelled button. The `primary` button is the default action and is
/// filled; others are plain until hovered.
pub(super) fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(px(metrics::BUTTON_HEIGHT))
        .min_w(px(metrics::BUTTON_MIN_WIDTH))
        .px(px(metrics::BUTTON_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::BUTTON_FONT_SIZE))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(label);

    if primary {
        button
            .bg(rgba(palette.selection))
            .text_color(rgb(palette.text_primary))
    } else {
        button
            .text_color(rgb(palette.text_primary))
            .bg(rgba(palette.control_hover_surface))
            .hover(|style| style.bg(rgba(palette.menu_hover)))
    }
}
