//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. Values come
//! from the bundled `theme.toml`; this module resolves them into GPUI colors.

use gpui::{
    App, Background, ColorExt, Rgba, Window, WindowAppearance, hsla, hsla_to_rgba,
    linear_color_stop, linear_gradient, rgb, rgb_to_hsla, rgba,
};
use photon_core::{ThemeMode, Transparency, WindowColor};
use photon_performance::PerformancePalette;
use std::cell::RefCell;
use std::collections::HashMap;

use super::settings::Settings;

/// The window colours, as hues, and how they tint each appearance.
mod window_color {
    use crate::platform::theme_config;
    use photon_core::WindowColor;

    /// A colour's hue, in turns, or `None` for the system grey.
    pub(super) fn hue(color: WindowColor) -> Option<f32> {
        let name = match color {
            WindowColor::System => return None,
            WindowColor::Blue => "blue",
            WindowColor::Purple => "purple",
            WindowColor::Pink => "pink",
            WindowColor::Red => "red",
            WindowColor::Orange => "orange",
            WindowColor::Yellow => "yellow",
            WindowColor::Green => "green",
            WindowColor::Teal => "teal",
        };
        Some(
            *theme_config::theme()
                .window
                .hues
                .get(name)
                .expect("every named window color must have a hue token"),
        )
    }
}

/// One colour used for both the active space's window tint and the shell's
/// selected controls and links.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AccentColor {
    Graphite,
    Blue,
    Green,
    Red,
    Orange,
    Teal,
    Purple,
    Pink,
}

impl AccentColor {
    pub(super) const ALL: [Self; 8] = [
        Self::Graphite,
        Self::Blue,
        Self::Green,
        Self::Red,
        Self::Orange,
        Self::Teal,
        Self::Purple,
        Self::Pink,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Teal => "Teal",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Teal => "teal",
            Self::Purple => "purple",
            Self::Pink => "pink",
        }
    }

    pub(super) fn window_color(self) -> WindowColor {
        match self {
            Self::Graphite => WindowColor::System,
            Self::Blue => WindowColor::Blue,
            Self::Green => WindowColor::Green,
            Self::Red => WindowColor::Red,
            Self::Orange => WindowColor::Orange,
            Self::Teal => WindowColor::Teal,
            Self::Purple => WindowColor::Purple,
            Self::Pink => WindowColor::Pink,
        }
    }

    pub(super) fn from_window_color(color: WindowColor) -> Self {
        match color {
            WindowColor::System => Self::Graphite,
            WindowColor::Blue => Self::Blue,
            WindowColor::Purple => Self::Purple,
            WindowColor::Pink => Self::Pink,
            WindowColor::Red => Self::Red,
            WindowColor::Orange | WindowColor::Yellow => Self::Orange,
            WindowColor::Green => Self::Green,
            WindowColor::Teal => Self::Teal,
        }
    }
}

/// The colors for `window`, in the appearance, transparency and window
/// colour the settings choose.
pub(super) fn palette(window: &Window, cx: &App) -> ThemeColors {
    let settings = Settings::get(cx);
    let appearance = Settings::appearance(window.appearance(), cx);
    let mut palette = ThemeColors::for_appearance(
        appearance,
        WindowStyle {
            transparency: settings.transparency,
            color: settings.spaces.active().color,
            gradient: settings.spaces.active().gradient,
        },
    );
    let has_window_tint = [palette.window_tint, palette.window_tint_end]
        .into_iter()
        .any(|color| rgb_to_hsla(rgba(color)).color.saturation > f32::EPSILON);
    if has_window_tint {
        palette.logo_tint = Some((palette.window_tint, palette.window_tint_end));
    }
    palette
}

/// What decides a palette besides the appearance.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct WindowStyle {
    transparency: Transparency,
    color: WindowColor,
    gradient: bool,
}

#[derive(Clone, Copy)]
struct BaseColors {
    background: Rgba,
    text: Rgba,
    container: Rgba,
    selected: Rgba,
}

/// A palette for a miniature theme preview in Appearance settings.
pub(super) fn preview_palette(dark: bool, accent: AccentColor) -> ThemeColors {
    ThemeColors::for_appearance(
        if dark {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        },
        WindowStyle {
            transparency: Transparency::Off,
            color: accent.window_color(),
            gradient: false,
        },
    )
}

/// The Light/System/Dark preview palettes. System shows a light sidebar and a
/// dark page to make the mixed appearance explicit.
pub(super) fn theme_preview_palettes(
    mode: ThemeMode,
    accent: AccentColor,
) -> (ThemeColors, ThemeColors) {
    match mode {
        ThemeMode::Light => {
            let light = preview_palette(false, accent);
            (light, light)
        }
        ThemeMode::Dark => {
            let dark = preview_palette(true, accent);
            (dark, dark)
        }
        ThemeMode::System => (
            preview_palette(false, accent),
            preview_palette(true, accent),
        ),
    }
}

/// How `color` looks in the swatch that offers it; the system grey is the
/// page background.
pub(super) fn window_color_swatch(color: WindowColor, palette: ThemeColors) -> u32 {
    match window_color::hue(color) {
        Some(hue) => {
            let window = &crate::platform::theme_config::theme().window;
            to_rgb_token(hsla_to_rgba(hsla(
                hue,
                window.swatch_saturation,
                window.swatch_lightness,
                1.0,
            )))
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
    /// Error messages, and the label of a button that deletes or resets.
    pub error: u32,
    /// A button that deletes or resets, under the pointer.
    pub error_hover_surface: u32,
    pub performance_palette: PerformancePalette,
    /// How much raised surfaces blur what is behind them; see
    /// [`Raised`](super::layout::Raised).
    pub raised_blur: f32,
    pub menu_surface: u32,
    /// The shortcut icon well, lightly tinted so its page backdrop shows through.
    pub shortcut_icon_surface: u32,
    pub menu_border: u32,
    /// The quiet outline around inputs and secondary controls.
    pub control_border: u32,
    /// The one surface the window's parts sit on: the browser's own pages,
    /// the omnibox field, buttons and text fields. A light
    /// tint over the window, exactly as see-through as the window is.
    pub surface: u32,
    /// A row or control under the pointer.
    pub hover_surface: u32,
    /// Something resting on `surface`, such as a favourite tile, under the
    /// pointer: stronger than its rest, weaker than when selected.
    pub surface_hover: u32,
    /// The selected row, such as the active tab or the open settings
    /// section, stronger than a row under the pointer.
    pub selected_surface: u32,
    /// What is chosen or on: a chosen option, a switch that is on, a checked box.
    pub chosen: u32,
    /// The window tint endpoints used to tint Photon marks. The default grey
    /// theme keeps the original monochrome logo.
    pub logo_tint: Option<(u32, u32)>,
    /// The accent used while a chosen action is under the pointer.
    pub chosen_hover: u32,
    /// A quiet accent outline for contextual drop targets.
    pub soft_accent_border: u32,
    /// Text and marks drawn on `chosen`.
    pub on_chosen: u32,
    /// Addresses in omnibox suggestions, set apart from page titles.
    pub suggestion_address: u32,
    /// Dims what a modal covers.
    pub modal_backdrop: u32,
    /// Dims the page under a link preview, lightly, so it stays in view.
    pub peek_backdrop: u32,
}

impl ThemeColors {
    /// The window's background: its tint, or a gradient between tints.
    pub(super) fn window_background(&self) -> Background {
        if self.window_tint == self.window_tint_end {
            return rgba(self.window_tint).into();
        }
        let angle = crate::platform::theme_config::theme()
            .window
            .gradient_angle_degrees;
        linear_gradient(
            angle,
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
        let dark = crate::platform::is_dark_appearance(appearance);
        CACHE.with_borrow_mut(|cache| {
            *cache.entry((dark, style)).or_insert_with(|| {
                let theme = crate::platform::theme_config::theme();
                let colors = if dark {
                    &theme.palette.dark
                } else {
                    &theme.palette.light
                };
                let colors = BaseColors {
                    background: color(&colors.background),
                    text: color(&colors.text),
                    container: color(&colors.container),
                    selected: color(&colors.selected),
                };
                let error = color(if dark {
                    &theme.colors.error.dark
                } else {
                    &theme.colors.error.light
                });
                let surfaces = theme.transparency.for_setting(style.transparency);
                Self::from_colors(colors, error, surfaces, style, dark)
            })
        })
    }

    fn from_colors(
        colors: BaseColors,
        error: Rgba,
        surfaces: &crate::platform::theme_config::SurfaceOpacity,
        style: WindowStyle,
        dark: bool,
    ) -> Self {
        let theme = crate::platform::theme_config::theme();
        let opacity = &theme.opacity;
        let text_secondary = mix_colors(colors.text, colors.background, opacity.text_secondary);
        let (saturation, lightness) = if dark {
            (
                theme.window.dark_tint_saturation,
                theme.window.dark_tint_lightness,
            )
        } else {
            (
                theme.window.light_tint_saturation,
                theme.window.light_tint_lightness,
            )
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
            Some(hue) if style.gradient => (tint(hue), tint(hue + theme.window.gradient_turn)),
            Some(hue) => (tint(hue), tint(hue)),
        };

        let menu_border = to_rgba_token(colors.text.opacity(opacity.menu_border));
        let accent = accent_color(AccentColor::from_window_color(style.color), dark);
        let accent_adjustment = color(if dark {
            &theme.colors.accent_hover_background.dark
        } else {
            &theme.colors.accent_hover_background.light
        });
        Self {
            window_tint: to_rgba_token(window_tint),
            window_tint_end: to_rgba_token(window_tint_end),
            page_background: to_rgb_token(colors.background),
            text_primary: to_rgb_token(colors.text),
            text_secondary: to_rgb_token(text_secondary),
            selection: to_rgba_token(colors.selected),
            field_error_border: to_rgba_token(error),
            error: to_rgb_token(error),
            error_hover_surface: to_rgba_token(error.opacity(opacity.error_hover)),
            performance_palette: PerformancePalette {
                surface: to_rgba_token(
                    colors
                        .container
                        .opacity(surfaces.raised.min(opacity.performance_surface)),
                ),
                text: to_rgb_token(colors.text),
                secondary_text: to_rgb_token(text_secondary),
            },
            // Raised surfaces let the window's frosted background show through,
            // but stay opaque enough to read over a busy page.
            menu_surface: to_rgba_token(colors.container.opacity(surfaces.raised)),
            shortcut_icon_surface: to_rgba_token(
                colors.container.opacity(opacity.shortcut_icon_surface),
            ),
            raised_blur: surfaces.raised_blur,
            // A light hairline that separates raised surfaces from what is under
            // them without a dark outline.
            menu_border,
            control_border: menu_border,
            // A tint over the window rather than a layer of its own, so stacking
            // it never makes the window less see-through.
            surface: to_rgba_token(colors.text.opacity(opacity.surface_tint)),
            hover_surface: to_rgba_token(colors.text.opacity(opacity.hover)),
            surface_hover: to_rgba_token(colors.text.opacity(opacity.surface_hover)),
            selected_surface: to_rgba_token(colors.text.opacity(opacity.selected)),
            chosen: to_rgb_token(accent),
            logo_tint: None,
            chosen_hover: to_rgb_token(mix_colors(
                accent,
                accent_adjustment,
                1.0 - opacity.accent_hover_blend,
            )),
            soft_accent_border: to_rgba_token(accent.opacity(opacity.soft_accent_border)),
            on_chosen: if dark {
                to_rgb_token(color(&theme.colors.chosen_foreground.dark))
            } else {
                to_rgb_token(color(&theme.colors.chosen_foreground.light))
            },
            suggestion_address: to_rgb_token(accent),
            modal_backdrop: to_rgba_token(
                color(if dark {
                    &theme.colors.modal_backdrop.dark
                } else {
                    &theme.colors.modal_backdrop.light
                })
                .opacity(opacity.modal_backdrop),
            ),
            peek_backdrop: to_rgba_token(
                color(if dark {
                    &theme.colors.peek_backdrop.dark
                } else {
                    &theme.colors.peek_backdrop.light
                })
                .opacity(opacity.peek_backdrop),
            ),
        }
    }
}

/// Accent tones stay readable as filled controls in either appearance, while
/// the swatches use the softer colours shown in Appearance settings.
fn accent_color(accent: AccentColor, dark: bool) -> Rgba {
    let accents = &crate::platform::theme_config::theme().colors.accents;
    let accent = accents
        .get(accent.key())
        .expect("every shell accent must have theme tokens");
    color(if dark { &accent.dark } else { &accent.light })
}

/// The softer accent tones displayed in the palette picker.
pub(super) fn accent_swatch(accent: AccentColor) -> u32 {
    let accents = &crate::platform::theme_config::theme().colors.accents;
    let accent = accents
        .get(accent.key())
        .expect("every shell accent must have theme tokens");
    to_rgb_token(color(&accent.swatch))
}

/// Dark ink on the palette's intentionally soft accent swatches.
pub(super) fn accent_swatch_mark() -> u32 {
    to_rgb_token(color(
        &crate::platform::theme_config::theme()
            .colors
            .accent_swatch_mark
            .value,
    ))
}

fn color(value: &str) -> Rgba {
    let hex = value
        .strip_prefix('#')
        .expect("theme colors must use #RRGGBB notation");
    assert_eq!(hex.len(), 6, "theme colors must use #RRGGBB notation");
    rgb(u32::from_str_radix(hex, 16).expect("theme colors must contain hexadecimal digits"))
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
