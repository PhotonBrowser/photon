//! The titlebar strip: hosts shell UI between the native window controls.

use gpui::{Div, div, prelude::*, px};

use super::theme::metrics;

pub(super) fn titlebar(content: impl IntoElement) -> Div {
    div()
        .w_full()
        .h(px(metrics::TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .px(px(metrics::WINDOW_CONTROLS_INSET))
        .child(content)
}
