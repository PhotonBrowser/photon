//! Shared horizontal dock row for address and navigation components.

use gpui::{AnyElement, prelude::*, px};

use super::layout::h_stack;
use super::metrics;

/// Places independent browser controls in one compact toolbar row.
pub(super) fn toolbar_row(controls: Vec<AnyElement>) -> impl IntoElement {
    h_stack()
        .w_full()
        .h(px(metrics::TOOLBAR_HEIGHT))
        .items_center()
        .gap(px(metrics::TOOLBAR_CONTROL_GAP))
        .px(px(metrics::TOOLBAR_HORIZONTAL_INSET))
        .tab_group()
        .children(controls)
}
