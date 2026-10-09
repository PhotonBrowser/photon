//! Monochrome shell icons, tinted with the caller's color.

use gpui::{Transformation, prelude::*, px, radians, rgb, svg};
use std::f32::consts::TAU;

/// Steps in one turn of the loading spinner, one per spoke.
pub(super) const LOADING_SPINNER_STEPS: usize = 12;

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

pub(super) fn globe_icon(color: u32, size: f32) -> gpui::Svg {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3Z" stroke-linejoin="round"/></g></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// Twelve spokes fading behind the leading one, turned one spoke per `step`.
pub(super) fn loading_spinner(color: u32, size: f32, step: usize) -> impl IntoElement {
    let turn = (step % LOADING_SPINNER_STEPS) as f32 / LOADING_SPINNER_STEPS as f32;
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 2.5v4" opacity="1"/><path d="M12 2.5v4" opacity=".92" transform="rotate(-30 12 12)"/><path d="M12 2.5v4" opacity=".84" transform="rotate(-60 12 12)"/><path d="M12 2.5v4" opacity=".76" transform="rotate(-90 12 12)"/><path d="M12 2.5v4" opacity=".68" transform="rotate(-120 12 12)"/><path d="M12 2.5v4" opacity=".6" transform="rotate(-150 12 12)"/><path d="M12 2.5v4" opacity=".52" transform="rotate(-180 12 12)"/><path d="M12 2.5v4" opacity=".44" transform="rotate(-210 12 12)"/><path d="M12 2.5v4" opacity=".36" transform="rotate(-240 12 12)"/><path d="M12 2.5v4" opacity=".28" transform="rotate(-270 12 12)"/><path d="M12 2.5v4" opacity=".2" transform="rotate(-300 12 12)"/><path d="M12 2.5v4" opacity=".12" transform="rotate(-330 12 12)"/></g></svg>"##,
        )
        .with_transformation(Transformation::rotate(radians(turn * TAU)))
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}
