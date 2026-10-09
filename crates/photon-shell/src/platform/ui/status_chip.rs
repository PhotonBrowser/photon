//! A small status chip in the window's bottom-right corner.

use gpui::{ElementId, MouseButton, Role, SharedString, prelude::*, px, rgb, rgba};

use super::button::{ButtonSize, button, icon_button};
use super::icons::{check_icon, close_icon, loading_spinner, warning_icon};
use super::layout::h_stack;
use super::motion::{AnimateIn, Entrance};
use super::{ClickHandler, metrics, theme::ThemeColors};

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
        chip = chip.child(icon_button(
            "status-chip-dismiss",
            "Dismiss",
            true,
            metrics::CHIP_CLOSE_SIZE,
            close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
            palette,
            on_dismiss,
        ));
    }

    chip.animate_in(id, Entrance::fade())
}
