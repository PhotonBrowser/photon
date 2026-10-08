//! Monochrome shell icons, tinted with the caller's color.

use gpui::{prelude::*, px, rgb, svg};

pub(super) fn search_icon_sized(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="10.5" cy="10.5" r="6.5" fill="none" stroke="currentColor" stroke-width="2.5"/><path d="M15.5 15.5 21 21" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn back_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M19 12H5m0 0 7-7m-7 7 7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn forward_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 12h14m0 0-7-7m7 7-7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn reload_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M20 11a8 8 0 0 0-14.9-3M5 4v4h4m-5 5a8 8 0 0 0 14.9 3M19 20v-4h-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn close_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn add_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn more_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="5" r="1.8" fill="currentColor"/><circle cx="12" cy="12" r="1.8" fill="currentColor"/><circle cx="12" cy="19" r="1.8" fill="currentColor"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn check_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="m5 12 4.5 4.5L19 7" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}
