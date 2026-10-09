//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. The base
//! light and dark palettes come from GPUI-CE; adjust token derivation here when
//! the shell needs a different role mapping.

use gpui::{ColorExt, Rgba, WindowAppearance, colors::Colors};
use photon_performance::PerformancePalette;
use std::{cell::Cell, rc::Rc, sync::OnceLock};

mod opacity {
    pub(super) const WINDOW_TINT: f32 = 0.88;
    pub(super) const TEXT_SECONDARY: f32 = 0.72;
    pub(super) const TEXT_DISABLED: f32 = 0.42;
    pub(super) const FOCUSED_FIELD: f32 = 0.08;
    pub(super) const TAB_HOVER: f32 = 0.08;
    pub(super) const CONTROL_HOVER: f32 = 0.12;
    pub(super) const PERFORMANCE_SURFACE: f32 = 0.95;
    pub(super) const MENU_BORDER: f32 = 0.55;
    pub(super) const MENU_HOVER: f32 = 0.12;
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

/// The app's explicit theme preference. `None` follows the system appearance.
#[derive(Clone, Default)]
pub(super) struct ThemePreference(Rc<Cell<Option<WindowAppearance>>>);

impl ThemePreference {
    pub(super) fn appearance(&self, system_appearance: WindowAppearance) -> WindowAppearance {
        self.0.get().unwrap_or(system_appearance)
    }

    pub(super) fn set(&self, appearance: WindowAppearance) {
        self.0.set(Some(appearance));
    }
}

/// Semantic colors used by the shell's controls and surfaces.
#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    pub window_tint: u32,
    pub page_background: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_disabled: u32,
    pub accent: u32,
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
    pub menu_hover: u32,
}

impl ThemeColors {
    /// Returns cached role colors for the active appearance.
    pub(super) fn for_appearance(appearance: WindowAppearance) -> Self {
        static LIGHT: OnceLock<ThemeColors> = OnceLock::new();
        static DARK: OnceLock<ThemeColors> = OnceLock::new();

        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => {
                *DARK.get_or_init(|| Self::from_gpui(Colors::dark(), error::dark()))
            }
            WindowAppearance::Light | WindowAppearance::VibrantLight => {
                *LIGHT.get_or_init(|| Self::from_gpui(Colors::light(), error::light()))
            }
        }
    }

    fn from_gpui(colors: Colors, error: Rgba) -> Self {
        let text_secondary = mix_colors(colors.text, colors.background, opacity::TEXT_SECONDARY);
        let text_disabled = mix_colors(colors.text, colors.background, opacity::TEXT_DISABLED);

        Self {
            window_tint: to_rgba_token(colors.background.opacity(opacity::WINDOW_TINT)),
            page_background: to_rgb_token(colors.background),
            text_primary: to_rgb_token(colors.text),
            text_secondary: to_rgb_token(text_secondary),
            text_disabled: to_rgb_token(text_disabled),
            accent: to_rgb_token(colors.selected),
            selection: to_rgba_token(colors.selected),
            field: to_rgba_token(colors.container),
            field_focused: to_rgba_token(mix_colors(
                colors.selected,
                colors.container,
                opacity::FOCUSED_FIELD,
            )),
            field_error_border: to_rgba_token(error),
            tab_active_surface: to_rgba_token(colors.container),
            tab_hover_surface: to_rgba_token(colors.selected.opacity(opacity::TAB_HOVER)),
            control_hover_surface: to_rgba_token(colors.text.opacity(opacity::CONTROL_HOVER)),
            performance_palette: PerformancePalette {
                surface: to_rgba_token(colors.container.opacity(opacity::PERFORMANCE_SURFACE)),
                text: to_rgb_token(colors.text),
                secondary_text: to_rgb_token(text_secondary),
            },
            menu_surface: to_rgba_token(colors.container),
            menu_border: to_rgba_token(colors.border.opacity(opacity::MENU_BORDER)),
            menu_hover: to_rgba_token(colors.selected.opacity(opacity::MENU_HOVER)),
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
