//! Monochrome shell icons, tinted with the caller's color.

use gpui::{Image, ImageFormat, Transformation, prelude::*, px, radians, rgb, svg};
use std::f32::consts::TAU;
use std::sync::{Arc, LazyLock};

/// The browser's logo, in its own colors.
pub(super) fn photon_logo() -> Arc<Image> {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            photon_brand::LOGO_SVG.to_vec(),
        ))
    });
    LOGO.clone()
}

/// A gear, for settings.
pub(super) fn settings_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 2.5l1.6 2.6 3-.6.6 3 2.6 1.6-1.4 2.9 1.4 2.9-2.6 1.6-.6 3-3-.6L12 21.5l-1.6-2.6-3 .6-.6-3-2.6-1.6 1.4-2.9-1.4-2.9 2.6-1.6.6-3 3 .6Z"/></g></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A circle half filled, for appearance.
pub(super) fn appearance_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17Z" fill="currentColor"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A page with a grid, for the new tab page.
pub(super) fn grid_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><rect x="4" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="4" width="6.5" height="6.5" rx="1.5"/><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5"/><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5"/></g></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A shield, for privacy.
pub(super) fn shield_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 3.5 5 6v5.5c0 4.3 2.9 7.6 7 9 4.1-1.4 7-4.7 7-9V6Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A window with a panel down its left side, for showing or hiding the
/// sidebar.
pub(super) fn sidebar_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><rect x="3.5" y="5" width="17" height="14" rx="3"/><path d="M9.5 5v14"/></g></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A pencil, for customising.
pub(super) fn edit_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 20h4L19 9l-4-4L4 16v4Zm9-13 4 4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

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

pub(super) fn audio_icon(color: u32, size: f32, muted: bool) -> impl IntoElement {
    let data: &[u8] = if muted {
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5 6 9H3v6h3l5 4z"/><path d="m16 9 5 6m0-6-5 6"/></g></svg>"##
    } else {
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7m3-10a9 9 0 0 1 0 13"/></g></svg>"##
    };
    svg()
        .data(data)
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

pub(super) fn minus_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"##,
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

/// A clock with a turning-back arrow, for past searches.
pub(super) fn history_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12a8 8 0 1 0 2.4-5.7L4 8.7"/><path d="M4 4v4.7h4.7M12 8v4.5l3 2"/></g></svg>"##,
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

pub(super) fn warning_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.3 4.1 2.6 17.5A2 2 0 0 0 4.3 20.5h15.4a2 2 0 0 0 1.7-3L13.7 4.1a2 2 0 0 0-3.4 0Z"/><path d="M12 9.5v4.5M12 17.2v.1"/></g></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn chevron_up_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="m6 15 6-6 6 6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn chevron_down_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="m6 9 6 6 6-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
        )
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}
