//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. The base
//! light and dark palettes come from GPUI-CE; adjust token derivation here when
//! the shell needs a different role mapping.

use gpui::{App, ColorExt, Rgba, Window, WindowAppearance, colors::Colors, rgb};
use photon_performance::PerformancePalette;
use std::sync::OnceLock;

use super::settings::Settings;

mod opacity {
    pub(super) const WINDOW_TINT: f32 = 0.88;
    pub(super) const TEXT_SECONDARY: f32 = 0.72;
    pub(super) const TEXT_DISABLED: f32 = 0.42;
    pub(super) const FOCUSED_FIELD: f32 = 0.08;
    pub(super) const TAB_HOVER: f32 = 0.12;
    pub(super) const CONTROL_HOVER: f32 = 0.18;
    pub(super) const PERFORMANCE_SURFACE: f32 = 0.95;
    pub(super) const MODAL_BACKDROP: f32 = 0.28;
    pub(super) const MENU_BORDER: f32 = 0.12;
    pub(super) const HOVER: f32 = 0.07;
    pub(super) const INTERNAL_PAGE: f32 = 0.04;
    pub(super) const RAISED_SURFACE: f32 = 0.92;
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

/// The colors for `window`, in the appearance the settings choose.
pub(super) fn palette(window: &Window, cx: &App) -> ThemeColors {
    ThemeColors::for_appearance(Settings::appearance(window.appearance(), cx))
}

/// Semantic colors used by the shell's controls and surfaces.
#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    pub window_tint: u32,
    pub page_background: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_disabled: u32,
    pub selection: u32,
    pub field: u32,
    pub field_focused: u32,
    pub field_error_border: u32,
    pub tab_active_surface: u32,
    pub tab_hover_surface: u32,
    pub control_hover_surface: u32,
    pub performance_palette: PerformancePalette,
    pub menu_surface: u32,
    pub menu_border: u32,
    /// The background of the browser's own pages: a subtle translucent tint
    /// set apart from the window.
    pub internal_page_surface: u32,
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
    /// Returns cached role colors for the active appearance.
    pub(super) fn for_appearance(appearance: WindowAppearance) -> Self {
        static LIGHT: OnceLock<ThemeColors> = OnceLock::new();
        static DARK: OnceLock<ThemeColors> = OnceLock::new();

        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                *DARK.get_or_init(|| Self::from_gpui(Colors::dark(), error::dark(), link::dark()))
            }
            WindowAppearance::Light | WindowAppearance::VibrantLight => *LIGHT
                .get_or_init(|| Self::from_gpui(Colors::light(), error::light(), link::light())),
        }
    }

    fn from_gpui(colors: Colors, error: Rgba, link: Rgba) -> Self {
        let text_secondary = mix_colors(colors.text, colors.background, opacity::TEXT_SECONDARY);
        let text_disabled = mix_colors(colors.text, colors.background, opacity::TEXT_DISABLED);

        Self {
            window_tint: to_rgba_token(colors.background.opacity(opacity::WINDOW_TINT)),
            page_background: to_rgb_token(colors.background),
            text_primary: to_rgb_token(colors.text),
            text_secondary: to_rgb_token(text_secondary),
            text_disabled: to_rgb_token(text_disabled),
            selection: to_rgba_token(colors.selected),
            field: to_rgba_token(colors.container),
            field_focused: to_rgba_token(mix_colors(
                colors.selected,
                colors.container,
                opacity::FOCUSED_FIELD,
            )),
            field_error_border: to_rgba_token(error),
            tab_active_surface: to_rgba_token(colors.container),
            tab_hover_surface: to_rgba_token(colors.text.opacity(opacity::TAB_HOVER)),
            control_hover_surface: to_rgba_token(colors.text.opacity(opacity::CONTROL_HOVER)),
            performance_palette: PerformancePalette {
                surface: to_rgba_token(colors.container.opacity(opacity::PERFORMANCE_SURFACE)),
                text: to_rgb_token(colors.text),
                secondary_text: to_rgb_token(text_secondary),
            },
            // Raised surfaces let the window's frosted background show through,
            // but stay opaque enough to read over a busy page.
            menu_surface: to_rgba_token(colors.container.opacity(opacity::RAISED_SURFACE)),
            // A light hairline that separates raised surfaces from what is under
            // them without a dark outline.
            menu_border: to_rgba_token(colors.text.opacity(opacity::MENU_BORDER)),
            // A light tint over the window's frosted background, like the
            // rest of the window.
            internal_page_surface: to_rgba_token(colors.text.opacity(opacity::INTERNAL_PAGE)),
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
