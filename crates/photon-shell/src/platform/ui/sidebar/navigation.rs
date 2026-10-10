//! The side dock's titlebar row: the toggle stays visible while navigation
//! controls fade with the dock.

use gpui::{AnyElement, div, prelude::*, px};

use super::super::layout::h_stack;
use super::super::metrics;
use super::super::titlebar::window_drag_area;

pub(in super::super) fn navigation_bar(
    width: f32,
    navigation_shown: f32,
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
    let controls = h_stack()
        .gap(px(metrics::SIDEBAR_CONTROL_GAP))
        .children(controls);
    let mut bar = window_drag_area(contents)
        .child(toggle.unwrap_or_else(|| div().into_any_element()))
        .child(div().flex_1());
    if navigation_shown > 0.0 {
        bar = bar.child(controls.opacity(navigation_shown));
    }
    bar
}
