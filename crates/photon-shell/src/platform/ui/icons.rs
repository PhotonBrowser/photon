//! Monochrome shell icons, tinted with the caller's color.
//!
//! Icons come from [Lucide](https://lucide.dev) (ISC license), vendored
//! unchanged from `lucide-static` in `assets/icons/lucide/`. To add one,
//! download it from the same version into that folder and load it with
//! `lucide!`.

use gpui::{Image, ImageFormat, Transformation, prelude::*, px, radians, rgb, svg};
use std::f32::consts::TAU;
use std::sync::{Arc, LazyLock};

/// A vendored Lucide icon's SVG, by its Lucide name.
macro_rules! lucide {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icons/lucide/",
            $name,
            ".svg"
        ))
    };
}

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

/// An icon from `data`, `size` square, tinted `color`.
fn icon(data: &'static [u8], color: u32, size: f32) -> gpui::Svg {
    svg()
        .data(data)
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// A gear, for settings.
pub(super) fn settings_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("settings"), color, size)
}

/// A circle half filled, for appearance.
pub(super) fn appearance_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("contrast"), color, size)
}

/// A grid, for the new tab page.
pub(super) fn grid_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("layout-grid"), color, size)
}

/// A shield, for privacy.
pub(super) fn shield_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("shield"), color, size)
}

/// A window with a panel down its left side, for showing or hiding the
/// sidebar.
pub(super) fn sidebar_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("panel-left"), color, size)
}

/// A pencil, for customising.
pub(super) fn edit_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("pencil"), color, size)
}

pub(super) fn search_icon_sized(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("search"), color, size)
}

pub(super) fn back_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("arrow-left"), color, size)
}

pub(super) fn forward_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("arrow-right"), color, size)
}

pub(super) fn reload_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("rotate-cw"), color, size)
}

pub(super) fn close_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("x"), color, size)
}

pub(super) fn add_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("plus"), color, size)
}

pub(super) fn minus_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("minus"), color, size)
}

pub(super) fn more_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("ellipsis-vertical"), color, size)
}

pub(super) fn check_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("check"), color, size)
}

/// A clock with a turning-back arrow, for past searches.
pub(super) fn history_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("history"), color, size)
}

pub(super) fn warning_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("triangle-alert"), color, size)
}

pub(super) fn chevron_up_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("chevron-up"), color, size)
}

pub(super) fn chevron_down_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("chevron-down"), color, size)
}

/// A speaker, crossed out when `muted`.
pub(super) fn audio_icon(color: u32, size: f32, muted: bool) -> impl IntoElement {
    let data: &'static [u8] = if muted {
        lucide!("volume-x")
    } else {
        lucide!("volume-2")
    };
    icon(data, color, size)
}

pub(super) fn globe_icon(color: u32, size: f32) -> gpui::Svg {
    icon(lucide!("globe"), color, size)
}

/// Steps in one turn of the loading spinner, one per spoke.
pub(super) const LOADING_SPINNER_STEPS: usize = 12;

/// Twelve spokes fading behind the leading one, turned one spoke per `step`.
pub(super) fn loading_spinner(color: u32, size: f32, step: usize) -> impl IntoElement {
    let turn = (step % LOADING_SPINNER_STEPS) as f32 / LOADING_SPINNER_STEPS as f32;
    icon(
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 2.5v4" opacity="1"/><path d="M12 2.5v4" opacity=".92" transform="rotate(-30 12 12)"/><path d="M12 2.5v4" opacity=".84" transform="rotate(-60 12 12)"/><path d="M12 2.5v4" opacity=".76" transform="rotate(-90 12 12)"/><path d="M12 2.5v4" opacity=".68" transform="rotate(-120 12 12)"/><path d="M12 2.5v4" opacity=".6" transform="rotate(-150 12 12)"/><path d="M12 2.5v4" opacity=".52" transform="rotate(-180 12 12)"/><path d="M12 2.5v4" opacity=".44" transform="rotate(-210 12 12)"/><path d="M12 2.5v4" opacity=".36" transform="rotate(-240 12 12)"/><path d="M12 2.5v4" opacity=".28" transform="rotate(-270 12 12)"/><path d="M12 2.5v4" opacity=".2" transform="rotate(-300 12 12)"/><path d="M12 2.5v4" opacity=".12" transform="rotate(-330 12 12)"/></g></svg>"##,
        color,
        size,
    )
    .with_transformation(Transformation::rotate(radians(turn * TAU)))
}
