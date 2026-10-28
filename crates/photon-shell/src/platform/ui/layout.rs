//! Reusable shell layout primitives.

use gpui::{Div, FontWeight, SharedString, Stateful, div, prelude::*, px, rgb, rgba};

use super::{metrics, theme::ThemeColors};

/// Start a horizontal flex layout. Add alignment, spacing, and sizing as needed.
pub(super) fn h_stack() -> Div {
    div().flex().flex_row()
}

/// Start a vertical flex layout. Add alignment, spacing, and sizing as needed.
pub(super) fn v_stack() -> Div {
    div().flex().flex_col()
}

/// A label above a control, such as “Shortcuts” above its choices.
pub(super) fn label(text: &'static str, palette: ThemeColors) -> impl IntoElement {
    div()
        .text_size(px(metrics::TAB_FONT_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(palette.text_secondary))
        .child(text)
}

/// A label and its control, with the shell's standard form spacing.
pub(super) fn form_field(
    label_text: &'static str,
    control: impl IntoElement,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .gap(px(metrics::FORM_FIELD_GAP))
        .child(label(label_text, palette))
        .child(control)
}

/// Secondary text that supports a control without competing with its label.
/// A short error, such as why a form cannot be saved, in the error colour
/// and read out as it appears.
pub(super) fn error_text(text: impl Into<SharedString>, palette: ThemeColors) -> Stateful<Div> {
    div()
        .id("form-error")
        .aria_live(gpui::accesskit::Live::Polite)
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.error))
        .child(text.into())
}

pub(super) fn secondary_text(text: impl Into<SharedString>, palette: ThemeColors) -> Div {
    div()
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .child(text.into())
}

/// How far a surface floats above what is under it, shown by its shadow.
#[derive(Clone, Copy)]
pub(super) enum Elevation {
    /// Small parts that sit just above their surface: switch knobs, chips.
    Low,
    /// Bars and the active tab, above the page.
    Medium,
    /// Menus, popovers and modals, above everything else.
    High,
}

/// Gives any styled element one of the shell's [`Elevation`] levels.
pub(super) trait Elevated: Styled + Sized {
    fn elevated(self, elevation: Elevation) -> Self {
        match elevation {
            Elevation::Low => self.shadow_sm(),
            Elevation::Medium => self.shadow_md(),
            Elevation::High => self.shadow_lg(),
        }
    }
}

impl<E: Styled> Elevated for E {}

/// Gives a floating element the theme's raised surface: translucent, and
/// frosting what is behind it as much as the transparency setting asks.
pub(super) trait Raised: Styled + Sized {
    fn raised(self, palette: ThemeColors) -> Self {
        self.bg(rgba(palette.menu_surface))
    }
}

impl<E: Styled> Raised for E {}

/// Gives inputs and secondary controls the same themed surface and outline.
pub(super) trait ControlSurface: Styled + Sized {
    fn control_surface(self, palette: ThemeColors) -> Self {
        let surface = self
            .rounded(px(metrics::control_radius()))
            .border_1()
            .border_color(rgba(palette.control_border))
            .bg(rgba(palette.surface));
        if palette.surface_blur > 0.0 {
            surface.backdrop_blur(px(palette.surface_blur))
        } else {
            surface
        }
    }
}

impl<E: Styled> ControlSurface for E {}
