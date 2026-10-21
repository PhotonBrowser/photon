//! Data definitions for the shell's semantic theme tokens.

use photon_core::Transparency;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub(crate) struct ThemeDefinition {
    pub(crate) palette: Appearance<BasePalette>,
    pub(crate) colors: ColorTokens,
    pub(crate) opacity: OpacityTokens,
    pub(crate) transparency: TransparencyTokens,
    pub(crate) window: WindowTokens,
    pub(crate) radii: RadiusTokens,
}

#[derive(Deserialize)]
pub(crate) struct Appearance<T> {
    pub(crate) light: T,
    pub(crate) dark: T,
}

#[derive(Deserialize)]
pub(crate) struct BasePalette {
    pub(crate) background: String,
    pub(crate) text: String,
    pub(crate) container: String,
    pub(crate) selected: String,
}

#[derive(Deserialize)]
pub(crate) struct ColorTokens {
    pub(crate) error: Appearance<String>,
    pub(crate) chosen_foreground: Appearance<String>,
    pub(crate) accent_hover_background: Appearance<String>,
    pub(crate) modal_backdrop: Appearance<String>,
    pub(crate) peek_backdrop: Appearance<String>,
    pub(crate) accent_swatch_mark: AccentSwatchMark,
    pub(crate) accents: HashMap<String, AccentTokens>,
}

#[derive(Deserialize)]
pub(crate) struct AccentSwatchMark {
    pub(crate) value: String,
}

#[derive(Deserialize)]
pub(crate) struct AccentTokens {
    pub(crate) light: String,
    pub(crate) dark: String,
    pub(crate) swatch: String,
}

#[derive(Deserialize)]
pub(crate) struct OpacityTokens {
    pub(crate) text_secondary: f32,
    pub(crate) performance_surface: f32,
    pub(crate) shortcut_icon_surface: f32,
    pub(crate) error_hover: f32,
    pub(crate) disabled_control: f32,
    pub(crate) tab_drag_source: f32,
    pub(crate) sleeping_tab: f32,
    pub(crate) space_dot_idle: f32,
    pub(crate) background_noise_max: f32,
    pub(crate) modal_backdrop: f32,
    pub(crate) peek_backdrop: f32,
    pub(crate) menu_border: f32,
    pub(crate) hover: f32,
    pub(crate) surface_tint: f32,
    pub(crate) selected: f32,
    pub(crate) surface_hover: f32,
    pub(crate) accent_hover_blend: f32,
    pub(crate) soft_accent_border: f32,
}

#[derive(Deserialize)]
pub(crate) struct TransparencyTokens {
    pub(crate) off: SurfaceOpacity,
    pub(crate) subtle: SurfaceOpacity,
    pub(crate) clear: SurfaceOpacity,
}

#[derive(Deserialize)]
pub(crate) struct SurfaceOpacity {
    pub(crate) window: f32,
    pub(crate) raised: f32,
    pub(crate) raised_blur: f32,
}

impl TransparencyTokens {
    pub(crate) fn for_setting(&self, transparency: Transparency) -> &SurfaceOpacity {
        match transparency {
            Transparency::Off => &self.off,
            Transparency::Subtle => &self.subtle,
            Transparency::Clear => &self.clear,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct WindowTokens {
    pub(crate) gradient_angle_degrees: f32,
    pub(crate) gradient_turn: f32,
    pub(crate) light_tint_saturation: f32,
    pub(crate) light_tint_lightness: f32,
    pub(crate) dark_tint_saturation: f32,
    pub(crate) dark_tint_lightness: f32,
    pub(crate) swatch_saturation: f32,
    pub(crate) swatch_lightness: f32,
    pub(crate) hues: HashMap<String, f32>,
}

#[derive(Deserialize)]
pub(crate) struct RadiusTokens {
    pub(crate) control: f32,
    pub(crate) small_button: f32,
    pub(crate) keycap: f32,
    pub(crate) surface: f32,
    pub(crate) menu_item: f32,
    pub(crate) tooltip: f32,
    pub(crate) omnibox: f32,
    pub(crate) sidebar_omnibox_button: f32,
    pub(crate) page: f32,
    pub(crate) page_smoothing: f32,
    pub(crate) theme_accent_panel: f32,
    pub(crate) checkbox: f32,
    pub(crate) segment: f32,
    pub(crate) sidebar_collapsed_item: f32,
    pub(crate) sidebar_item: f32,
    pub(crate) sidebar_reveal: f32,
}

pub(crate) fn theme() -> &'static ThemeDefinition {
    static THEME: OnceLock<ThemeDefinition> = OnceLock::new();
    THEME.get_or_init(|| {
        let theme: ThemeDefinition = toml::from_str(include_str!("../../theme.toml"))
            .expect("the bundled Photon theme.toml must be valid");
        validate_theme(&theme);
        theme
    })
}

pub(crate) fn radius_tokens() -> &'static RadiusTokens {
    &theme().radii
}

fn validate_theme(theme: &ThemeDefinition) {
    let colors = &theme.colors;
    let mut values = vec![
        &theme.palette.light.background,
        &theme.palette.light.text,
        &theme.palette.light.container,
        &theme.palette.light.selected,
        &theme.palette.dark.background,
        &theme.palette.dark.text,
        &theme.palette.dark.container,
        &theme.palette.dark.selected,
        &colors.error.light,
        &colors.error.dark,
        &colors.chosen_foreground.light,
        &colors.chosen_foreground.dark,
        &colors.accent_hover_background.light,
        &colors.accent_hover_background.dark,
        &colors.modal_backdrop.light,
        &colors.modal_backdrop.dark,
        &colors.peek_backdrop.light,
        &colors.peek_backdrop.dark,
        &colors.accent_swatch_mark.value,
    ];
    for accent in colors.accents.values() {
        values.extend([&accent.light, &accent.dark, &accent.swatch]);
    }
    for value in values {
        let Some(hex) = value.strip_prefix('#') else {
            panic!("theme colors must use #RRGGBB notation: {value}");
        };
        assert!(
            hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "theme colors must use #RRGGBB notation: {value}"
        );
    }

    let opacity = &theme.opacity;
    for (name, value) in [
        ("text_secondary", opacity.text_secondary),
        ("performance_surface", opacity.performance_surface),
        ("shortcut_icon_surface", opacity.shortcut_icon_surface),
        ("error_hover", opacity.error_hover),
        ("disabled_control", opacity.disabled_control),
        ("tab_drag_source", opacity.tab_drag_source),
        ("sleeping_tab", opacity.sleeping_tab),
        ("space_dot_idle", opacity.space_dot_idle),
        ("background_noise_max", opacity.background_noise_max),
        ("modal_backdrop", opacity.modal_backdrop),
        ("peek_backdrop", opacity.peek_backdrop),
        ("menu_border", opacity.menu_border),
        ("hover", opacity.hover),
        ("surface_tint", opacity.surface_tint),
        ("selected", opacity.selected),
        ("surface_hover", opacity.surface_hover),
        ("accent_hover_blend", opacity.accent_hover_blend),
        ("soft_accent_border", opacity.soft_accent_border),
    ] {
        assert!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "theme opacity token {name} must be between 0 and 1"
        );
    }

    let radii = &theme.radii;
    for (name, value) in [
        ("control", radii.control),
        ("small_button", radii.small_button),
        ("keycap", radii.keycap),
        ("surface", radii.surface),
        ("menu_item", radii.menu_item),
        ("tooltip", radii.tooltip),
        ("omnibox", radii.omnibox),
        ("sidebar_omnibox_button", radii.sidebar_omnibox_button),
        ("page", radii.page),
        ("theme_accent_panel", radii.theme_accent_panel),
        ("checkbox", radii.checkbox),
        ("segment", radii.segment),
        ("sidebar_collapsed_item", radii.sidebar_collapsed_item),
        ("sidebar_item", radii.sidebar_item),
        ("sidebar_reveal", radii.sidebar_reveal),
    ] {
        assert!(
            value.is_finite() && value >= 0.0,
            "theme radius token {name} must be a nonnegative logical pixel value"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::theme;

    #[test]
    fn bundled_theme_has_complete_accent_and_window_color_tokens() {
        let theme = theme();
        for accent in [
            "graphite", "blue", "green", "red", "orange", "teal", "purple", "pink",
        ] {
            assert!(theme.colors.accents.contains_key(accent));
        }
        for hue in [
            "blue", "purple", "pink", "red", "orange", "yellow", "green", "teal",
        ] {
            assert!(theme.window.hues.contains_key(hue));
        }
        assert!(theme.radii.page.is_finite() && theme.radii.page >= 0.0);
        assert!(theme.opacity.surface_tint.is_finite());
    }
}
