//! The appearance section: light, dark or following macOS, and how much of
//! the desktop shows through the browser.

use gpui::{App, prelude::*, px};
use photon_core::{ThemeMode, Transparency};

use super::super::super::controls::{Choice, choices};
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::layout::{label, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx);
    let themes = [
        (ThemeMode::System, "System"),
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
    ]
    .into_iter()
    .map(|(theme, label)| Choice {
        label: label.into(),
        chosen: current.theme == theme,
        on_choose: change(move |settings| settings.theme = theme),
    })
    .collect();
    let transparencies = [
        (Transparency::Off, "Off"),
        (Transparency::Subtle, "Subtle"),
        (Transparency::Clear, "Clear"),
    ]
    .into_iter()
    .map(|(transparency, label)| Choice {
        label: label.into(),
        chosen: current.transparency == transparency,
        on_choose: change(move |settings| settings.transparency = transparency),
    })
    .collect();
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Theme", palette))
                .child(choices("settings-theme", "Theme", themes, palette)),
        )
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Transparency", palette))
                .child(choices(
                    "settings-transparency",
                    "Transparency",
                    transparencies,
                    palette,
                ))
                .child(secondary_text(
                    "How much of the desktop shows through tabs, toolbars, menus and pages.",
                    palette,
                )),
        )
}
