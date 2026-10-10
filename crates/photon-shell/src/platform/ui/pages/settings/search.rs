//! The search section: which engine searches from the address bar go to.

use gpui::{App, prelude::*};

use super::super::super::controls::{Choice, choices};
use super::super::super::settings::Settings;
use super::super::super::theme::ThemeColors;
use super::super::change;

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx).default_search_engine.clone();
    let options = Settings::search_engines(cx)
        .engines()
        .iter()
        .map(|engine| {
            let id = engine.id.to_string();
            Choice {
                label: engine.name.to_string().into(),
                chosen: current == id,
                on_choose: change(move |settings| settings.default_search_engine.clone_from(&id)),
            }
        })
        .collect();
    choices("settings-search", "Search engine", options, palette)
}
