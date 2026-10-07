//! Monochrome shell icons, tinted with the caller's color.

use gpui::{Animation, AnimationExt, Transformation, prelude::*, px, radians, rgb, svg};
use std::time::Duration;

const SIZE: f32 = 13.0;

pub(super) fn search_icon(color: u32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="#000" stroke-width="2.5"/><path d="M15.5 15.5 21 21" stroke="#000" stroke-width="2.5" stroke-linecap="round"/></svg>"##,
        )
        .size(px(SIZE))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn loading_spinner(color: u32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 3a9 9 0 1 0 9 9" fill="none" stroke="#000" stroke-width="3" stroke-linecap="round"/></svg>"##,
        )
        .size(px(SIZE))
        .flex_shrink_0()
        .text_color(rgb(color))
        .with_animation(
            "page-loading-spinner",
            Animation::new(Duration::from_millis(900))
                .repeat_synced()
                .with_max_fps(30.0),
            |spinner, phase| {
                spinner.with_transformation(Transformation::rotate(radians(
                    phase * std::f32::consts::TAU,
                )))
            },
        )
}
