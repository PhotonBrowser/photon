//! The tabs section: inactive page sleeping, closing the last tab, and
//! closing tabs left unused.

use gpui::{App, SharedString, prelude::*, px};
use photon_core::TabSleepMode;
use photon_shortcuts::ReopenClosedTab;

use super::super::super::controls::{Choice, choices};
use super::super::super::kbd::action_keys;
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx);
    let close_window = current.close_window_with_last_tab;
    let archive = current.archive_tabs;
    let sleep_mode = current.tab_sleep_mode;
    let setting = |row, note: SharedString| {
        v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(group(palette).child(row))
            .child(secondary_text(note, palette))
    };
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(setting(
            choices(
                "settings-tab-sleep",
                "Inactive tabs",
                [
                    (TabSleepMode::Off, "Off"),
                    (TabSleepMode::Balanced, "Balanced"),
                    (TabSleepMode::Maximum, "Maximum"),
                ]
                .into_iter()
                .map(|(mode, label)| Choice {
                    label: label.into(),
                    chosen: sleep_mode == mode,
                    on_choose: change(move |settings| settings.tab_sleep_mode = mode),
                })
                .collect(),
                palette,
            )
            .into_any_element(),
            match sleep_mode {
                TabSleepMode::Off => "Keep automatic tab sleeping disabled.".into(),
                TabSleepMode::Balanced => "After one minute in another tab, stop rendering loaded, silent pages and keep their live state.".into(),
                TabSleepMode::Maximum => "After five seconds in another tab, stop rendering loaded, silent pages and keep their live state.".into(),
            },
        ))
        .child(setting(
            switch_row(
                "settings-close-window-with-last-tab",
                "Close window with last tab",
                close_window,
                palette,
                change(move |settings| settings.close_window_with_last_tab = !close_window),
            )
            .into_any_element(),
            "When off, closing the last tab leaves the window open without tabs.".into(),
        ))
        .child(setting(
            switch_row(
                "settings-archive-tabs",
                "Close unused tabs after 12 hours",
                archive,
                palette,
                change(move |settings| settings.archive_tabs = !archive),
            )
            .into_any_element(),
            match action_keys(&ReopenClosedTab, cx) {
                Some(keys) => format!("Pinned tabs stay. Closed tabs can be reopened with {keys}."),
                None => "Pinned tabs stay. Closed tabs can be reopened from the History menu."
                    .to_owned(),
            }
            .into(),
        ))
}
