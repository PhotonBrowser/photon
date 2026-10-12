//! The side dock's navigation controls: in a row beside the window controls,
//! or, collapsed to icons, the toggle beneath the stacked window controls.

use gpui::{AnyElement, div, prelude::*, px};

use super::super::layout::{h_stack, v_stack};
use super::super::metrics;

pub(in super::super) fn navigation_bar(
    width: f32,
    fullscreen: bool,
    leading: bool,
    toggle: Option<AnyElement>,
    controls: Vec<AnyElement>,
) -> gpui::Div {
    let controls_inset = if fullscreen {
        0.0
    } else {
        metrics::WINDOW_CONTROLS_INSET
    };
    let mut contents = h_stack()
        .w(px(width))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(metrics::SIDEBAR_CONTROL_GAP))
        .pr(px(metrics::SIDEBAR_PADDING))
        .tab_group();
    contents = if leading {
        contents.pl(px(controls_inset))
    } else {
        contents.pl(px(metrics::SIDEBAR_PADDING))
    };
    contents
        .child(toggle.unwrap_or_else(|| div().into_any_element()))
        .child(div().flex_1())
        .child(
            h_stack()
                .gap(px(metrics::SIDEBAR_CONTROL_GAP))
                .children(controls),
        )
}

/// The collapsed sidebar's top: room for the window controls stacked at the
/// leading edge, then the toggle that expands it.
pub(in super::super) fn navigation_column(
    fullscreen: bool,
    leading: bool,
    toggle: Option<AnyElement>,
) -> gpui::Div {
    let controls_inset = if leading && !fullscreen {
        metrics::WINDOW_CONTROLS_COLUMN_INSET
    } else {
        metrics::SIDEBAR_PADDING
    };
    v_stack()
        .w(px(metrics::SIDEBAR_COLLAPSED_WIDTH))
        .flex_shrink_0()
        .items_center()
        .pt(px(controls_inset))
        .pb(px(metrics::SIDEBAR_COLLAPSED_GAP))
        .tab_group()
        .children(toggle)
}
