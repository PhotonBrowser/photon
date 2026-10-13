//! Monochrome shell icons, tinted with the caller's color.
//!
//! Icons come from [Lucide](https://lucide.dev) (ISC license), vendored
//! unchanged from `lucide-static` in `assets/icons/lucide/`. To add one,
//! download it from the same version into that folder and load it with
//! `lucide!`.

use gpui::{
    Image, ImageFormat, Transformation, hsla, hsla_to_rgba, prelude::*, px, radians, rgb,
    rgb_to_hsla, rgba, svg,
};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::{Arc, LazyLock, Mutex};

type LogoTint = (u32, u32);
type LogoCache = Mutex<HashMap<LogoTint, Arc<Image>>>;

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

/// The browser's logo. The original grey tones are tinted with the active
/// window colour, including both ends of a window gradient.
pub(super) fn photon_logo(theme_tint: Option<(u32, u32)>) -> Arc<Image> {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            photon_brand::LOGO_SVG.to_vec(),
        ))
    });
    let Some((start, end)) = theme_tint else {
        return LOGO.clone();
    };

    static THEMED_LOGOS: LazyLock<LogoCache> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut logos = THEMED_LOGOS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    logos
        .entry((start, end))
        .or_insert_with(|| {
            let base = tinted_gradient("photon-logo-base", 0xacacac, start, end);
            let highlight = tinted_gradient("photon-logo-highlight", 0xd5d5d5, start, end);
            let shade = tinted_gradient("photon-logo-shade", 0x696969, start, end);
            let svg = String::from_utf8_lossy(photon_brand::LOGO_SVG)
                .replace("</defs>", &format!("{base}{highlight}{shade}</defs>"))
                .replace("#ACACAC", "url(#photon-logo-base)")
                .replace("#D5D5D5", "url(#photon-logo-highlight)")
                .replace("#696969", "url(#photon-logo-shade)");
            Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
        })
        .clone()
}

fn tinted_gradient(name: &str, grey: u32, start: u32, end: u32) -> String {
    let grey_lightness = rgb_to_hsla(rgb(grey)).color.lightness;
    let tint = |color| {
        let hue_and_saturation = rgb_to_hsla(rgba(color)).color;
        let hue = hue_and_saturation.hue.into_positive_degrees() / 360.0;
        let color = hsla_to_rgba(hsla(
            hue,
            hue_and_saturation.saturation,
            grey_lightness,
            1.0,
        ));
        let channel = |value: f32| (value * 255.0).round() as u32;
        (channel(color.color.red) << 16)
            | (channel(color.color.green) << 8)
            | channel(color.color.blue)
    };
    format!(
        "<linearGradient id=\"{name}\" x1=\"0%\" y1=\"0%\" x2=\"100%\" y2=\"100%\"><stop offset=\"0%\" stop-color=\"#{:06X}\"/><stop offset=\"100%\" stop-color=\"#{:06X}\"/></linearGradient>",
        tint(start),
        tint(end),
    )
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

/// A sidebar with an arrow out of it, for expanding the collapsed sidebar.
pub(super) fn sidebar_expand_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("panel-left-open"), color, size)
}

/// A frame of corners around nothing, for Zen mode, which leaves only the
/// page.
pub(super) fn zen_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("scan"), color, size)
}

/// A keyboard, for the list of keyboard shortcuts.
pub(super) fn keyboard_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("keyboard"), color, size)
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

pub(super) fn minimize_window_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("minus"), color, size)
}

/// A filled star, for the empty favourites hint.
pub(super) fn favourite_star_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="m12 2.5 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5-5.8-3.1-5.8 3.1 1.1-6.5-4.7-4.6 6.5-.9z"/></svg>"##)
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub(super) fn add_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("plus"), color, size)
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

/// Two overlapping pages, for copying.
pub(super) fn copy_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("copy"), color, size)
}

/// Two arrows pointing apart, for opening a preview in a tab.
pub(super) fn open_in_tab_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("maximize-2"), color, size)
}

/// A window, for opening a new one.
pub(super) fn window_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("app-window"), color, size)
}

/// A single window outline, for maximizing.
pub(super) fn maximize_window_icon(color: u32, size: f32) -> impl IntoElement {
    svg()
        .data(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="1.5" fill="none" stroke="currentColor" stroke-width="2"/></svg>"##)
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

/// Overlapping windows, for restoring a maximized window.
pub(super) fn restore_window_icon(color: u32, size: f32) -> impl IntoElement {
    icon(lucide!("copy"), color, size)
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
