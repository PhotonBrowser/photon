//! The tabs section: inactive page sleeping, closing the last tab, and
//! closing tabs left unused.

use crate::platform::shortcuts::ReopenClosedTab;
use gpui::{App, SharedString, prelude::*, px};
use photon_core::{NewTabPosition, TabSleepMode};

use super::super::super::controls::{Choice, choices};
use super::super::super::kbd::action_keys;
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, group_with_choices, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx);
    let close_window = current.close_window_with_last_tab;
    let archive = current.archive_tabs;
    let sleep_mode = current.tab_sleep_mode;
    let new_tab_position = current.new_tab_position;
    let setting = |row, note: SharedString, panel: gpui::Div| {
        v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(panel.child(row))
            .child(secondary_text(note, palette))
    };
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(setting(
            choices(
                "settings-new-tab-position",
                "New tab position",
                [
                    (NewTabPosition::Top, "At top"),
                    (NewTabPosition::AfterLast, "After last"),
                ]
                .into_iter()
                .map(|(position, label)| Choice {
                    label: label.into(),
                    chosen: new_tab_position == position,
                    on_choose: change(move |settings| settings.new_tab_position = position),
                })
                .collect(),
                palette,
            )
            .into_any_element(),
            "In the vertical tab list, choose where new tabs appear. Their slide animation stays on."
                .into(),
            group_with_choices(palette),
        ))
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
            sleep_description(sleep_mode).into(),
            group_with_choices(palette),
        ))
        .child(setting(
            switch_row(
                "settings-close-window-with-last-tab",
                "Close main window with last tab",
                close_window,
                palette,
                change(move |settings| settings.close_window_with_last_tab = !close_window),
            )
            .into_any_element(),
            "Other windows always close with their last tab. When off, the main window stays open on a new tab page; the horizontal layout adds it as a tab."
                .into(),
            group(palette),
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
            group(palette),
        ))
}

fn sleep_description(mode: TabSleepMode) -> &'static str {
    #[cfg(target_os = "macos")]
    {
        match mode {
            TabSleepMode::Off => {
                "Keep pages loaded unless system memory pressure requires a discard. Selecting a discarded page reloads its URL; unsaved page state and in-page history are lost."
            }
            TabSleepMode::Balanced => {
                "After thirty seconds out of view or about five minutes without input, discard its page process. Selecting it reloads the URL; unsaved page state and in-page history are lost."
            }
            TabSleepMode::Maximum => {
                "After five seconds out of view or about one minute without input, discard its page process. Selecting it reloads the URL; unsaved page state and in-page history are lost."
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        match mode {
            TabSleepMode::Off => {
                "Keep pages loaded unless system memory pressure requires a discard. Selecting a discarded page reloads its URL; unsaved page state and in-page history are lost."
            }
            TabSleepMode::Balanced => {
                "After thirty seconds out of view, discard its page process. Selecting it reloads the URL; unsaved page state and in-page history are lost."
            }
            TabSleepMode::Maximum => {
                "After five seconds out of view, discard its page process. Selecting it reloads the URL; unsaved page state and in-page history are lost."
            }
        }
    }
}
