//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. The base
//! light and dark palettes come from GPUI-CE; adjust token derivation here when
//! the shell needs a different role mapping.

use gpui::{App, ColorExt, Rgba, Window, WindowAppearance, colors::Colors, rgb};
use photon_core::Transparency;
use photon_performance::PerformancePalette;
use std::sync::OnceLock;

use super::settings::Settings;

/// How opaque the window and raised surfaces are, for each transparency
/// setting. Surfaces on the window are tints over it, so they are exactly as
/// see-through as the window; menus and dialogs over a busy page are more
/// opaque, so their text stays readable.
#[derive(Clone, Copy)]
struct SurfaceOpacity {
    window: f32,
    raised: f32,
}

impl SurfaceOpacity {
    const fn for_transparency(transparency: Transparency) -> Self {
        match transparency {
            Transparency::Off => Self {
                window: 1.0,
                raised: 1.0,
            },
            Transparency::Subtle => Self {
                window: 0.88,
                raised: 0.92,
            },
            Transparency::Clear => Self {
                window: 0.7,
                raised: 0.84,
            },
        }
    }
}

mod opacity {
    pub(super) const TEXT_SECONDARY: f32 = 0.72;
    pub(super) const PERFORMANCE_SURFACE: f32 = 0.95;
    pub(super) const MODAL_BACKDROP: f32 = 0.28;
    pub(super) const MENU_BORDER: f32 = 0.12;
    pub(super) const HOVER: f32 = 0.07;
    /// How much of the text color tints the shared surface.
    pub(super) const SURFACE_TINT: f32 = 0.06;
    pub(super) const SELECTED: f32 = 0.12;
}

/// GPUI-CE's palettes have no error role, so the shell uses the macOS system
/// red for each appearance.
mod error {
    use gpui::{Rgba, rgb};

    pub(super) fn light() -> Rgba {
        rgb(0xff3b30)
    }

    pub(super) fn dark() -> Rgba {
        rgb(0xff453a)
    }
}

/// Link-style blue for addresses, readable on each appearance.
mod link {
    use gpui::{Rgba, rgb};

    pub(super) fn light() -> Rgba {
        rgb(0x1a73e8)
    }

    pub(super) fn dark() -> Rgba {
        rgb(0x8ab4f8)
    }
}

/// The colors for `window`, in the appearance and transparency the
/// settings choose.
pub(super) fn palette(window: &Window, cx: &App) -> ThemeColors {
    ThemeColors::for_appearance(
        Settings::appearance(window.appearance(), cx),
        Settings::get(cx).transparency,
    )
}

/// Semantic colors used by the shell's controls and surfaces.
#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    pub window_tint: u32,
    pub page_background: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub selection: u32,
    pub field_error_border: u32,
    pub performance_palette: PerformancePalette,
    pub menu_surface: u32,
    pub menu_border: u32,
    /// The one surface the window's parts sit on: the browser's own pages,
    /// the omnibox field, the active tab, buttons and text fields. A light
    /// tint over the window, exactly as see-through as the window is.
    pub surface: u32,
    /// A row or control under the pointer.
    pub hover_surface: u32,
    /// The selected row, such as the open settings section.
    pub selected_surface: u32,
    /// What is chosen or on: a chosen option, a switch that is on, a checked box.
    pub chosen: u32,
    /// Text and marks drawn on `chosen`.
    pub on_chosen: u32,
    /// Addresses in omnibox suggestions, set apart from page titles.
    pub suggestion_address: u32,
    /// Dims what a modal covers.
    pub modal_backdrop: u32,
}

impl ThemeColors {
    /// Returns cached role colors for an appearance and transparency.
    fn for_appearance(appearance: WindowAppearance, transparency: Transparency) -> Self {
        static CACHE: [[OnceLock<ThemeColors>; 3]; 2] = [
            [OnceLock::new(), OnceLock::new(), OnceLock::new()],
            [OnceLock::new(), OnceLock::new(), OnceLock::new()],
        ];
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let opacity = SurfaceOpacity::for_transparency(transparency);
        *CACHE[usize::from(dark)][transparency as usize].get_or_init(|| {
            if dark {
                Self::from_gpui(Colors::dark(), error::dark(), link::dark(), opacity)
            } else {
                Self::from_gpui(Colors::light(), error::light(), link::light(), opacity)
            }
        })
    }

    fn from_gpui(colors: Colors, error: Rgba, link: Rgba, surfaces: SurfaceOpacity) -> Self {
        let text_secondary = mix_colors(colors.text, colors.background, opacity::TEXT_SECONDARY);

        Self {
            window_tint: to_rgba_token(colors.background.opacity(surfaces.window)),
            page_background: to_rgb_token(colors.background),
            text_primary: to_rgb_token(colors.text),
            text_secondary: to_rgb_token(text_secondary),
            selection: to_rgba_token(colors.selected),
            field_error_border: to_rgba_token(error),
            performance_palette: PerformancePalette {
                surface: to_rgba_token(
                    colors
                        .container
                        .opacity(surfaces.raised.min(opacity::PERFORMANCE_SURFACE)),
                ),
                text: to_rgb_token(colors.text),
                secondary_text: to_rgb_token(text_secondary),
            },
            // Raised surfaces let the window's frosted background show through,
            // but stay opaque enough to read over a busy page.
            menu_surface: to_rgba_token(colors.container.opacity(surfaces.raised)),
            // A light hairline that separates raised surfaces from what is under
            // them without a dark outline.
            menu_border: to_rgba_token(colors.text.opacity(opacity::MENU_BORDER)),
            // A tint over the window rather than a layer of its own, so stacking
            // it never makes the window less see-through.
            surface: to_rgba_token(colors.text.opacity(opacity::SURFACE_TINT)),
            hover_surface: to_rgba_token(colors.text.opacity(opacity::HOVER)),
            selected_surface: to_rgba_token(colors.text.opacity(opacity::SELECTED)),
            chosen: to_rgb_token(link),
            on_chosen: to_rgb_token(rgb(0xffffff)),
            suggestion_address: to_rgb_token(link),
            modal_backdrop: to_rgba_token(Rgba::new(0.0, 0.0, 0.0, opacity::MODAL_BACKDROP)),
        }
    }
}

fn mix_colors(foreground: Rgba, background: Rgba, foreground_weight: f32) -> Rgba {
    let foreground_weight = foreground_weight.clamp(0.0, 1.0);
    let mix_channel = |foreground: f32, background: f32| {
        foreground * foreground_weight + background * (1.0 - foreground_weight)
    };

    Rgba::new(
        mix_channel(foreground.color.red, background.color.red),
        mix_channel(foreground.color.green, background.color.green),
        mix_channel(foreground.color.blue, background.color.blue),
        1.0,
    )
}

fn to_rgb_token(color: Rgba) -> u32 {
    (to_channel(color.color.red) << 16)
        | (to_channel(color.color.green) << 8)
        | to_channel(color.color.blue)
}

fn to_rgba_token(color: Rgba) -> u32 {
    (to_rgb_token(color) << 8) | to_channel(color.alpha)
}

fn to_channel(channel: f32) -> u32 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u32
}
