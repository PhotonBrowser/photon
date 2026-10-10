//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. The base
//! light and dark palettes come from GPUI-CE; adjust token derivation here when
//! the shell needs a different role mapping.

use gpui::{
    App, Background, ColorExt, Rgba, Window, WindowAppearance, colors::Colors, hsla, hsla_to_rgba,
    linear_color_stop, linear_gradient, rgb, rgb_to_hsla, rgba,
};
use photon_core::{Transparency, WindowColor};
use photon_performance::PerformancePalette;
use std::cell::RefCell;
use std::collections::HashMap;

use super::settings::Settings;

/// How opaque the window and raised surfaces are, and how much raised
/// surfaces blur what is behind them, for each transparency setting. Surfaces
/// on the window are tints over it, so they are exactly as see-through as the
/// window; menus and dialogs over a busy page are more opaque and frost the
/// page behind them, so their text stays readable.
#[derive(Clone, Copy)]
struct SurfaceOpacity {
    window: f32,
    raised: f32,
    /// The blur radius behind raised surfaces, in pixels.
    raised_blur: f32,
}

impl SurfaceOpacity {
    const fn for_transparency(transparency: Transparency) -> Self {
        match transparency {
            Transparency::Off => Self {
                window: 1.0,
                raised: 1.0,
                raised_blur: 0.0,
            },
            Transparency::Subtle => Self {
                window: 0.88,
                raised: 0.92,
                raised_blur: 20.0,
            },
            Transparency::Clear => Self {
                window: 0.7,
                raised: 0.84,
                raised_blur: 32.0,
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

/// The direction a window gradient runs, in degrees: from the top left.
const WINDOW_GRADIENT_ANGLE: f32 = 135.0;

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

/// The window colours, as hues, and how they tint each appearance.
mod window_color {
    use photon_core::WindowColor;

    /// A colour's hue, in turns, or `None` for the system grey.
    pub(super) fn hue(color: WindowColor) -> Option<f32> {
        match color {
            WindowColor::System => None,
            WindowColor::Blue => Some(0.6),
            WindowColor::Purple => Some(0.74),
            WindowColor::Pink => Some(0.9),
            WindowColor::Red => Some(0.99),
            WindowColor::Orange => Some(0.07),
            WindowColor::Yellow => Some(0.13),
            WindowColor::Green => Some(0.38),
            WindowColor::Teal => Some(0.49),
        }
    }

    /// How far a gradient turns toward the neighbouring hue, in turns.
    pub(super) const GRADIENT_TURN: f32 = 0.09;

    /// Saturation and lightness of the tint on each appearance, and of the
    /// swatches that offer it.
    pub(super) const LIGHT: (f32, f32) = (0.55, 0.87);
    pub(super) const DARK: (f32, f32) = (0.3, 0.17);
    pub(super) const SWATCH: (f32, f32) = (0.6, 0.55);
}

/// The colors for `window`, in the appearance, transparency and window
/// colour the settings choose.
pub(super) fn palette(window: &Window, cx: &App) -> ThemeColors {
    let settings = Settings::get(cx);
    ThemeColors::for_appearance(
        Settings::appearance(window.appearance(), cx),
        WindowStyle {
            transparency: settings.transparency,
            color: settings.window_color,
            gradient: settings.window_gradient,
        },
    )
}

/// What decides a palette besides the appearance.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct WindowStyle {
    transparency: Transparency,
    color: WindowColor,
    gradient: bool,
}

/// How `color` looks in the swatch that offers it; the system grey is the
/// page background.
pub(super) fn window_color_swatch(color: WindowColor, palette: ThemeColors) -> u32 {
    match window_color::hue(color) {
        Some(hue) => {
            let (saturation, lightness) = window_color::SWATCH;
            to_rgb_token(hsla_to_rgba(hsla(hue, saturation, lightness, 1.0)))
        }
        None => palette.page_background,
    }
}

/// Semantic colors used by the shell's controls and surfaces.
#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    /// The window's background, fading to `window_tint_end` when it is a
    /// gradient; see [`ThemeColors::window_background`].
    pub window_tint: u32,
    pub window_tint_end: u32,
    pub page_background: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub selection: u32,
    pub field_error_border: u32,
    pub performance_palette: PerformancePalette,
    /// How much raised surfaces blur what is behind them; see
    /// [`Raised`](super::layout::Raised).
    pub raised_blur: f32,
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
    /// The window's background: its tint, or a gradient between tints.
    pub(super) fn window_background(&self) -> Background {
        if self.window_tint == self.window_tint_end {
            return rgba(self.window_tint).into();
        }
        linear_gradient(
            WINDOW_GRADIENT_ANGLE,
            linear_color_stop(rgb_to_hsla(rgba(self.window_tint)), 0.0),
            linear_color_stop(rgb_to_hsla(rgba(self.window_tint_end)), 1.0),
        )
    }

    /// Returns cached role colors for an appearance and window style.
    fn for_appearance(appearance: WindowAppearance, style: WindowStyle) -> Self {
        thread_local! {
            static CACHE: RefCell<HashMap<(bool, WindowStyle), ThemeColors>> =
                RefCell::new(HashMap::new());
        }
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        CACHE.with_borrow_mut(|cache| {
            *cache.entry((dark, style)).or_insert_with(|| {
                let opacity = SurfaceOpacity::for_transparency(style.transparency);
                let (colors, error, link) = if dark {
                    (Colors::dark(), error::dark(), link::dark())
                } else {
                    (Colors::light(), error::light(), link::light())
                };
                Self::from_gpui(colors, error, link, opacity, style, dark)
            })
        })
    }

    fn from_gpui(
        colors: Colors,
        error: Rgba,
        link: Rgba,
        surfaces: SurfaceOpacity,
        style: WindowStyle,
        dark: bool,
    ) -> Self {
        let text_secondary = mix_colors(colors.text, colors.background, opacity::TEXT_SECONDARY);
        let (saturation, lightness) = if dark {
            window_color::DARK
        } else {
            window_color::LIGHT
        };
        let tint = |hue: f32| -> Rgba {
            hsla_to_rgba(hsla(
                hue.rem_euclid(1.0),
                saturation,
                lightness,
                surfaces.window,
            ))
        };
        let (window_tint, window_tint_end) = match window_color::hue(style.color) {
            None => {
                let tint = colors.background.opacity(surfaces.window);
                (tint, tint)
            }
            Some(hue) if style.gradient => (tint(hue), tint(hue + window_color::GRADIENT_TURN)),
            Some(hue) => (tint(hue), tint(hue)),
        };

        Self {
            window_tint: to_rgba_token(window_tint),
            window_tint_end: to_rgba_token(window_tint_end),
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
            raised_blur: surfaces.raised_blur,
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
