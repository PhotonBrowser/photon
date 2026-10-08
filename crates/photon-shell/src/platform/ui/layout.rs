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
