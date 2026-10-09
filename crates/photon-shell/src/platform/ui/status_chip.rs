//! A small status chip in the window's bottom-right corner.

use gpui::{
    Animation, AnimationExt, ElementId, MouseButton, Role, SharedString, ease_out_quint,
    prelude::*, px, rgb, rgba,
};
use std::time::Duration;

use super::button::{ButtonSize, button};
use super::icons::{check_icon, close_icon, loading_spinner, warning_icon};
use super::layout::h_stack;
use super::{ClickHandler, metrics, theme::ThemeColors};

/// How long a chip takes to fade in.
const APPEAR_DURATION: Duration = Duration::from_millis(200);

/// What a chip's leading icon shows.
pub(super) enum ChipIcon {
    /// Work in progress; the value is the spinner's animation step.
    Working(usize),
    Done,
    Problem,
}

/// A compact chip with an icon and message, an optional action button and
/// an optional close button. A new `id` fades the chip in again.
pub(super) fn status_chip(
    id: ElementId,
    palette: ThemeColors,
    icon: ChipIcon,
    message: SharedString,
    action: Option<(&'static str, ClickHandler)>,
    on_dismiss: Option<ClickHandler>,
) -> impl IntoElement {
    let icon_size = metrics::CHIP_ICON_SIZE;
    let icon = match icon {
        ChipIcon::Working(step) => {
            loading_spinner(palette.text_secondary, icon_size, step).into_any_element()
        }
        ChipIcon::Done => check_icon(palette.text_secondary, icon_size).into_any_element(),
        ChipIcon::Problem => warning_icon(palette.text_primary, icon_size).into_any_element(),
    };

    let mut chip = h_stack()
        .id(id.clone())
        .role(Role::Status)
        .aria_label(message.clone())
        .aria_live(gpui::accesskit::Live::Polite)
        .items_center()
        .gap(px(metrics::CHIP_GAP))
        .min_h(px(metrics::CHIP_HEIGHT))
        .pl(px(metrics::CHIP_PADDING))
        .pr(px(if action.is_some() || on_dismiss.is_some() {
            metrics::CHIP_TRAILING_PADDING
        } else {
            metrics::CHIP_PADDING
        }))
        .rounded(px(metrics::CHIP_HEIGHT / 2.0))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .text_size(px(metrics::CHIP_FONT_SIZE))
        .text_color(rgb(palette.text_primary))
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(icon)
        .child(message);
    if let Some((label, on_click)) = action {
        chip = chip.child(button(
            "status-chip-action",
            label,
            true,
            ButtonSize::Small,
            palette,
            on_click,
        ));
    }
    if let Some(on_dismiss) = on_dismiss {
        chip = chip.child(
            h_stack()
                .id("status-chip-dismiss")
                .role(Role::Button)
                .aria_label("Dismiss")
                .tab_index(0)
                .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
                .items_center()
                .justify_center()
                .size(px(metrics::CHIP_CLOSE_SIZE))
                .rounded(px(metrics::CHIP_CLOSE_SIZE / 2.0))
                .hover(|style| style.bg(rgba(palette.control_hover_surface)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(on_dismiss)
                .child(close_icon(
                    palette.text_secondary,
                    metrics::CHIP_CLOSE_ICON_SIZE,
                )),
        );
    }

    chip.with_animation(
        id,
        Animation::new(APPEAR_DURATION).with_easing(ease_out_quint()),
        |chip, delta| chip.opacity(delta),
    )
}
