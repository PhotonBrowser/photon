//! Reusable shell layout primitives.

use gpui::{Div, div, prelude::*};

/// Start a horizontal flex layout. Add alignment, spacing, and sizing as needed.
pub(super) fn h_stack() -> Div {
    div().flex().flex_row()
}

/// Start a vertical flex layout. Add alignment, spacing, and sizing as needed.
pub(super) fn v_stack() -> Div {
    div().flex().flex_col()
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
