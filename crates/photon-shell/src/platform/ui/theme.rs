//! Theme selection and semantic shell color tokens.
//!
//! Views use these role names instead of choosing colors locally. Values come
//! from the bundled `theme.toml`; this module resolves them into GPUI colors.

use gpui::{
    App, Background, ColorExt, Rgba, Window, WindowAppearance, hsla, hsla_to_rgba,
    linear_color_stop, linear_gradient, rgb, rgb_to_hsla, rgba,
};
use photon_core::{Space, SpaceColors, SpaceTint, Transparency, WindowColor};
use photon_performance_ui::PerformancePalette;

use super::appearance_preview::AppearancePreview;
use super::settings::Settings;
use std::{cell::RefCell, collections::VecDeque};

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
    let preview = AppearancePreview::get(cx);
    let active = super::window_space::window_space(window, cx);
    let (color, colors) = preview
        .tint
        .filter(|(id, _, _)| *id == active.id)
        .map(|(_, color, colors)| (color, colors))
        .unwrap_or((active.color, space_colors(active)));
    let mut palette = ThemeColors::for_appearance(
        appearance,
        WindowStyle {
            transparency: settings.transparency,
            color,
            gradient: active.gradient,
            tint: colors
                .primary()
                .or_else(|| Some(default_tint(window.appearance()))),
            colors: Some(colors),
            transparency_percent: Some(AppearancePreview::transparency_in(active, cx)),
        },
    );
    // Live drags update colour surfaces without rasterizing a new SVG logo per frame.
    let logo = ThemeColors::for_appearance(
        appearance,
        WindowStyle {
            transparency: settings.transparency,
            transparency_percent: settings.transparency_percent,
            color: active.color,
            tint: active.tint,
            colors: active.colors,
            gradient: active.gradient,
        },
    );
    let has_window_tint = [logo.window_tint, logo.window_tint_end]
        .into_iter()
        .any(|color| rgb_to_hsla(rgba(color)).color.saturation > f32::EPSILON);
    if has_window_tint {
        palette.logo_tint = Some((logo.window_tint | 0xff, logo.window_tint_end | 0xff));
    }

    palette
}

/// What decides a palette besides the appearance.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct WindowStyle {
    transparency: Transparency,
    color: WindowColor,
    gradient: bool,
    tint: Option<SpaceTint>,
    colors: Option<SpaceColors>,
    transparency_percent: Option<u8>,
}

#[derive(Clone, Copy)]
struct BaseColors {
    background: Rgba,
    page: Rgba,
    text: Rgba,
    container: Rgba,
    selected: Rgba,
}

/// Presets and their swatches share the same resolved colour.
pub(super) fn window_color_swatch(color: WindowColor) -> u32 {
    tint_swatch(preset_tint(color))
}

/// Semantic colors used by the shell's controls and surfaces.
#[derive(Clone, Copy)]
pub(super) struct ThemeColors {
    /// The window's background, fading to `window_tint_end` when it is a
    /// gradient; see [`ThemeColors::window_background`].
    pub window_tint: u32,
    pub window_tint_end: u32,
    pub window_tint_middle: Option<u32>,
    pub page_background: u32,
    /// Appearance tint over the native window, retaining configured transparency.
    pub page_surface: u32,
    /// Backdrop blur for surfaces with theme-reduced opacity.
    pub surface_blur: f32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub selection: u32,
    /// Error messages, and the label of a button that deletes or resets.
    pub error: u32,
    /// A button that deletes or resets, under the pointer.
    pub error_hover_surface: u32,
    /// A secure connection's status.
    pub secure: u32,
    pub performance_palette: PerformancePalette,
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
        if self.window_tint_middle.is_some() {
            return gpui::transparent_black().into();
        }
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

    /// Spatial average of a solid, two-stop, or three-stop background for contrast.
    pub(super) fn background_average(&self) -> Rgba {
        let ends = mix_colors(rgba(self.window_tint), rgba(self.window_tint_end), 0.5);
        self.window_tint_middle
            .map(|middle| mix_colors(rgba(middle), ends, 0.5))
            .unwrap_or(ends)
    }

    /// Native overlay used only when a third independently editable stop is present.
    pub(super) fn gradient_overlay(&self, radius: f32) -> Option<impl gpui::IntoElement> {
        use gpui::{IntoElement, Styled, canvas};
        let middle = self.window_tint_middle?;
        let colors = [self.window_tint, middle, self.window_tint_end];
        let angle = crate::platform::theme_config::theme()
            .window
            .gradient_angle_degrees;
        Some(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    super::theme_gradient::paint(bounds, colors, angle, radius, window)
                },
            )
            .absolute()
            .inset_0()
            .size_full()
            .into_any_element(),
        )
    }

    /// Keep only recent palettes: live colour drags must not grow an unbounded cache.
    fn for_appearance(appearance: WindowAppearance, style: WindowStyle) -> Self {
        type Cache = VecDeque<((bool, WindowStyle), ThemeColors)>;
        thread_local! { static CACHE: RefCell<Cache> = const { RefCell::new(VecDeque::new()) }; }
        let key = (crate::platform::is_dark_appearance(appearance), style);
        CACHE.with_borrow_mut(|cache| {
            if let Some((_, palette)) = cache.iter().find(|(cached, _)| *cached == key) {
                return *palette;
            }
            let palette = Self::resolve(appearance, style);
            if cache.len() == 6 {
                cache.pop_front();
            }
            cache.push_back((key, palette));
            palette
        })
    }

    fn resolve(appearance: WindowAppearance, style: WindowStyle) -> Self {
        let dark = crate::platform::is_dark_appearance(appearance);
        let theme = crate::platform::theme_config::theme();
        let colors = if dark {
            &theme.palette.dark
        } else {
            &theme.palette.light
        };
        let colors = BaseColors {
            background: color(&colors.background),
            page: color(&colors.page),
            text: color(&colors.text),
            container: color(&colors.container),
            selected: color(&colors.selected),
        };
        let error = color(if dark {
            &theme.colors.error.dark
        } else {
            &theme.colors.error.light
        });
        let secure = color(if dark {
            &theme.colors.secure.dark
        } else {
            &theme.colors.secure.light
        });
        let surfaces = style
            .transparency_percent
            .map(|percent| theme.transparency.for_percent(percent));
        Self::from_colors(
            colors,
            error,
            secure,
            surfaces
                .as_ref()
                .unwrap_or_else(|| theme.transparency.for_setting(style.transparency)),
            style,
            dark,
        )
    }

    fn from_colors(
        colors: BaseColors,
        error: Rgba,
        secure: Rgba,
        surfaces: &crate::platform::theme_config::SurfaceOpacity,
        style: WindowStyle,
        dark: bool,
    ) -> Self {
        let theme = crate::platform::theme_config::theme();
        let opacity = &theme.opacity;
        let text_secondary = mix_colors(colors.text, colors.background, opacity.text_secondary);
        // Window tint is independent of page/menu appearance.
        let resolved = style.tint.unwrap_or_else(|| preset_tint(style.color));
        let tint = |hue: f32| -> Rgba {
            let selected = hsla_to_rgba(hsla(
                hue.rem_euclid(1.0),
                f32::from(resolved.saturation.min(100)) / 100.0,
                f32::from(resolved.lightness.min(100)) / 100.0,
                1.0,
            ));
            selected.opacity(surfaces.window)
        };
        let hue = f32::from(resolved.hue % 360) / 360.0;
        let chosen = style
            .colors
            .map(gradient_colors)
            .filter(|colors| !colors.colors().is_empty());
        let color_at = |color: SpaceTint| tint_background(color).opacity(surfaces.window);
        let window_tint = chosen
            .as_ref()
            .map(|colors| color_at(colors.colors()[0]))
            .unwrap_or_else(|| tint(hue));
        let window_tint_middle = chosen
            .as_ref()
            .filter(|colors| colors.colors().len() == 3)
            .map(|colors| to_rgba_token(color_at(colors.colors()[1])));
        let window_tint_end = if let Some(colors) = &chosen {
            color_at(
                *colors
                    .colors()
                    .last()
                    .expect("a space palette has at least one colour"),
            )
        } else if style.gradient {
            tint(hue + theme.window.gradient_turn)
        } else {
            window_tint
        };

        let menu_border = to_rgba_token(colors.text.opacity(opacity.menu_border));
        let accent = style
            .colors
            .and_then(|colors| colors.primary())
            .map(tint_background)
            .unwrap_or_else(|| accent_color(AccentColor::from_window_color(style.color), dark));
        let accent_adjustment = color(if dark {
            &theme.colors.accent_hover_background.dark
        } else {
            &theme.colors.accent_hover_background.light
        });
        Self {
            window_tint: to_rgba_token(window_tint),
            window_tint_end: to_rgba_token(window_tint_end),
            window_tint_middle,
            page_background: to_rgb_token(colors.page),
            page_surface: to_rgba_token(colors.page.opacity(surfaces.page)),
            surface_blur: surfaces.surface_blur,
            text_primary: to_rgb_token(colors.text),
            text_secondary: to_rgb_token(text_secondary),
            selection: to_rgba_token(colors.selected),
            error: to_rgb_token(error),
            error_hover_surface: to_rgba_token(error.opacity(opacity.error_hover)),
            secure: to_rgb_token(secure),
            performance_palette: PerformancePalette {
                surface: to_rgba_token(
                    colors
                        .container
                        .opacity(surfaces.raised.min(opacity.performance_surface)),
                ),
                text: to_rgb_token(colors.text),
                secondary_text: to_rgb_token(text_secondary),
            },
            // Raised surfaces stay fully opaque, regardless of window transparency.
            menu_surface: to_rgba_token(colors.container.opacity(surfaces.raised)),
            shortcut_icon_surface: to_rgba_token(
                colors.container.opacity(opacity.shortcut_icon_surface),
            ),
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
            on_chosen: contrasting_text(accent),
            suggestion_address: to_rgb_token(legible_on(accent, colors.text, colors.container)),
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

/// The preset coordinates in the same field used for custom colours.
pub(super) fn preset_tint(preset: WindowColor) -> SpaceTint {
    let tokens = &crate::platform::theme_config::theme().window;
    SpaceTint {
        hue: (window_color::hue(preset).unwrap_or(0.0) * 360.0).round() as u16,
        saturation: if preset == WindowColor::System {
            0
        } else {
            (tokens.swatch_saturation * 100.0).round() as u8
        },
        lightness: (if preset == WindowColor::System {
            tokens.light_tint_lightness
        } else {
            tokens.swatch_lightness
        } * 100.0)
            .round() as u8,
    }
}

/// Put the primary at the leading stop, or in the centre of a three-colour gradient.
pub(super) fn gradient_colors(colors: SpaceColors) -> SpaceColors {
    let values = colors.colors();
    let primary = colors.primary_index();
    match values.len() {
        2 => SpaceColors::from_colors(&[values[primary], values[1 - primary]]),
        3 => SpaceColors::from_colors(&[
            values[(primary + 1) % 3],
            values[primary],
            values[(primary + 2) % 3],
        ]),
        _ => colors,
    }
}

/// Native appearance supplies the default shell colour, independently of page mode.
pub(super) fn default_tint(appearance: WindowAppearance) -> SpaceTint {
    let theme = crate::platform::theme_config::theme();
    let window = if crate::platform::is_dark_appearance(appearance) {
        &theme.palette.dark.window
    } else {
        &theme.palette.light.window
    };
    let hsl = rgb_to_hsla(color(window)).color;
    SpaceTint {
        hue: hsl.hue.into_positive_degrees().round() as u16 % 360,
        saturation: (hsl.saturation * 100.0).round() as u8,
        lightness: (hsl.lightness * 100.0).round() as u8,
    }
}

/// Resolve old solid/gradient profiles into explicit, independently editable colours.
pub(super) fn space_colors(space: &Space) -> SpaceColors {
    if let Some(colors) = space.colors {
        return colors;
    }
    if space.color == WindowColor::System && space.tint.is_none() && !space.gradient {
        return SpaceColors::default();
    }
    let first = space.tint.unwrap_or_else(|| preset_tint(space.color));
    if !space.gradient {
        return SpaceColors::from_colors(&[first]);
    }
    let mut second = first;
    second.hue = (f32::from(first.hue)
        + crate::platform::theme_config::theme().window.gradient_turn * 360.0)
        .round() as u16
        % 360;
    SpaceColors::from_colors(&[first, second])
}

/// A saved custom tint represented by a space's colour dot.
pub(super) fn tint_swatch(tint: SpaceTint) -> u32 {
    to_rgb_token(tint_background(tint))
}

/// The contrast tinted text, such as suggestion addresses, keeps against
/// what it sits on: WCAG's minimum for body text.
const MIN_TINTED_TEXT_CONTRAST: f32 = 4.5;

fn relative_luminance(color: Rgba) -> f32 {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.color.red)
        + 0.7152 * linear(color.color.green)
        + 0.0722 * linear(color.color.blue)
}

fn contrast_ratio(a: Rgba, b: Rgba) -> f32 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `tint`, moved toward `text` only as far as it takes to read on
/// `background`, so a dark accent stays legible on a dark surface.
fn legible_on(tint: Rgba, text: Rgba, background: Rgba) -> Rgba {
    const STEPS: u8 = 10;
    (0..=STEPS)
        .map(|step| mix_colors(text, tint, f32::from(step) / f32::from(STEPS)))
        .find(|candidate| contrast_ratio(*candidate, background) >= MIN_TINTED_TEXT_CONTRAST)
        .unwrap_or(text)
}

/// Relative luminance chooses the more legible of black and white ink.
pub(super) fn contrasting_text(background: Rgba) -> u32 {
    let luminance = relative_luminance(background);
    let tokens = &crate::platform::theme_config::theme().picker;
    if luminance > 0.179 {
        to_rgb_token(color(&tokens.ink_dark))
    } else {
        to_rgb_token(color(&tokens.ink_light))
    }
}

/// Internal pages retain the window material and choose ink for the composited tint.
pub(super) fn page_palette(window: &Window, cx: &App) -> ThemeColors {
    let mut result = palette(window, cx);
    let surface = rgba(result.page_surface);
    let background = mix_colors(surface, result.background_average(), surface.alpha);
    let ink = contrasting_text(background);
    let opacity = &crate::platform::theme_config::theme().opacity;
    result.text_primary = ink;
    result.text_secondary = to_rgb_token(mix_colors(rgb(ink), background, opacity.text_secondary));
    result.surface = to_rgba_token(rgb(ink).opacity(opacity.surface_tint));
    result.menu_surface = result.surface;
    result.hover_surface = to_rgba_token(rgb(ink).opacity(opacity.hover));
    result.surface_hover = to_rgba_token(rgb(ink).opacity(opacity.surface_hover));
    result.selected_surface = to_rgba_token(rgb(ink).opacity(opacity.selected));
    result.control_border = to_rgba_token(rgb(ink).opacity(opacity.menu_border));
    result.menu_border = result.control_border;
    result
}

/// Chrome follows its tint's contrast, independently of internal pages and menus.
pub(super) fn chrome_palette(window: &Window, cx: &App) -> ThemeColors {
    let mut result = palette(window, cx);
    let start = result.background_average();
    let ink = contrasting_text(start);
    let dark = ink
        == to_rgb_token(color(
            &crate::platform::theme_config::theme().picker.ink_light,
        ));
    let settings = Settings::get(cx);
    let space = super::window_space::window_space(window, cx);
    let chrome = ThemeColors::for_appearance(
        if dark {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        },
        WindowStyle {
            transparency: settings.transparency,
            color: space.color,
            gradient: space.gradient,
            tint: space.tint,
            colors: space.colors,
            transparency_percent: Some(AppearancePreview::transparency_in(space, cx)),
        },
    );
    result.text_primary = ink;
    result.text_secondary = to_rgb_token(mix_colors(
        rgb(ink),
        start,
        crate::platform::theme_config::theme()
            .opacity
            .text_secondary,
    ));
    result.surface = chrome.surface;
    result.hover_surface = chrome.hover_surface;
    result.surface_hover = chrome.surface_hover;
    result.selected_surface = chrome.selected_surface;
    result.control_border = chrome.control_border;
    result
}

/// The selected colour stays independent of transparency.
pub(super) fn tint_background(tint: SpaceTint) -> Rgba {
    hsla_to_rgba(hsla(
        f32::from(tint.hue % 360) / 360.0,
        f32::from(tint.saturation.min(100)) / 100.0,
        f32::from(tint.lightness.min(100)) / 100.0,
        1.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tinted_text_lightens_a_dark_accent_on_a_dark_surface() {
        let navy = rgb(0x1a2a4a);
        let white = rgb(0xffffff);
        let panel = rgb(0x262626);
        let legible = legible_on(navy, white, panel);
        assert!(contrast_ratio(legible, panel) >= MIN_TINTED_TEXT_CONTRAST);
        assert_ne!(to_rgb_token(legible), to_rgb_token(white));
        let bright = rgb(0x8ab4f8);
        assert_eq!(
            to_rgb_token(legible_on(bright, white, panel)),
            to_rgb_token(bright)
        );
    }
    #[test]
    fn explicit_gradient_keeps_each_colour_and_collapses_back_to_a_solid() {
        let a = SpaceTint {
            hue: 210,
            saturation: 60,
            lightness: 40,
        };
        let b = SpaceTint {
            hue: 150,
            saturation: 70,
            lightness: 50,
        };
        let c = SpaceTint {
            hue: 270,
            saturation: 55,
            lightness: 60,
        };
        let mut space = Space::default();
        space.set_colors(SpaceColors::from_colors(&[a, b, c]).with_primary(1));
        let style = WindowStyle {
            transparency: Transparency::Off,
            transparency_percent: Some(0),
            color: space.color,
            tint: space.tint,
            colors: space.colors,
            gradient: true,
        };
        let palette = ThemeColors::for_appearance(WindowAppearance::Light, style);
        assert_eq!(palette.window_tint >> 8, tint_swatch(c));
        assert_eq!(palette.window_tint_middle.unwrap() >> 8, tint_swatch(b));
        assert_eq!(palette.window_tint_end >> 8, tint_swatch(a));
        let solid = ThemeColors::for_appearance(
            WindowAppearance::Light,
            WindowStyle {
                colors: Some(SpaceColors::from_colors(&[b])),
                ..style
            },
        );
        assert!(solid.window_tint_middle.is_none());
        assert_eq!(solid.window_tint, solid.window_tint_end);
    }
    #[test]
    fn window_tint_is_independent_of_page_appearance() {
        let style = WindowStyle {
            transparency: Transparency::Off,
            color: WindowColor::Blue,
            gradient: true,
            transparency_percent: None,
            colors: None,
            tint: Some(SpaceTint {
                lightness: 40,
                ..SpaceTint::default()
            }),
        };
        let light = ThemeColors::for_appearance(WindowAppearance::Light, style);
        let dark = ThemeColors::for_appearance(WindowAppearance::Dark, style);
        assert_eq!(light.window_tint, dark.window_tint);
        assert_eq!(light.window_tint_end, dark.window_tint_end);
        assert_ne!(light.page_background, dark.page_background);
        assert_ne!(light.menu_surface, dark.menu_surface);
    }
    #[test]
    fn solid_system_theme_separates_page_and_window_surfaces() {
        let style = WindowStyle {
            transparency: Transparency::Off,
            color: WindowColor::System,
            gradient: false,
            tint: None,
            colors: None,
            transparency_percent: None,
        };
        for appearance in [WindowAppearance::Light, WindowAppearance::Dark] {
            let palette = ThemeColors::for_appearance(appearance, style);
            assert_ne!(palette.window_tint >> 8, palette.page_background);
            assert!(rgba(palette.page_surface).alpha > 0.3);
        }
    }
    #[gpui::test]
    fn translucent_pages_use_tint_contrast_while_menus_follow_appearance(
        cx: &mut gpui::TestAppContext,
    ) {
        let mut settings = photon_core::BrowserSettings {
            theme: photon_core::ThemeMode::Dark,
            transparency: Transparency::Clear,
            ..Default::default()
        };
        let id = settings.spaces.active().id;
        settings.spaces.get_mut(id).unwrap().tint = Some(SpaceTint {
            saturation: 0,
            lightness: 90,
            ..Default::default()
        });
        cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let page = page_palette(window, cx);
            let menu = palette(window, cx);
            assert!(
                rgba(page.page_surface).alpha < 1.0,
                "pages must retain native transparency"
            );
            assert_eq!(
                page.text_primary, 0x000000,
                "pale translucent pages need dark ink even in dark appearance"
            );
            assert_eq!(
                menu.text_primary, 0xffffff,
                "dark menus keep their own foreground"
            );
            Settings::update(cx, |settings| settings.transparency = Transparency::Off);
            let page = page_palette(window, cx);
            assert!(rgba(page.page_surface).alpha < 1.0);
            assert_eq!(page.text_primary, 0x000000);
        });
    }
    #[test]
    fn foreground_chooses_black_or_white_for_legibility() {
        assert_eq!(contrasting_text(rgb(0xffffff)), 0x000000);
        assert_eq!(contrasting_text(rgb(0x000000)), 0xffffff);
        assert_eq!(contrasting_text(rgb(0x0044dd)), 0xffffff);
        assert_eq!(contrasting_text(rgb(0xffee44)), 0x000000);
    }
}
