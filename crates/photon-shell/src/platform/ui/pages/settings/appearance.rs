//! The appearance section: light, dark, or following macOS.

use gpui::{App, prelude::*};
use photon_core::ThemeMode;

use super::super::super::controls::{Choice, choices};
use super::super::super::settings::Settings;
use super::super::super::theme::ThemeColors;
use super::super::change;

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx).theme;
    let options = [
        (ThemeMode::System, "System"),
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
    ]
    .into_iter()
    .map(|(theme, label)| Choice {
        label: label.into(),
        chosen: current == theme,
        on_choose: change(move |settings| settings.theme = theme),
    })
    .collect();
    choices("settings-theme", "Appearance", options, palette)
}
