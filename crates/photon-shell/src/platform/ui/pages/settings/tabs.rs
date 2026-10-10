//! The tabs section: what closing the last tab does, and closing tabs left
//! unused.

use gpui::{App, prelude::*, px};

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
    let setting = |row, note| {
        v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(group(palette).child(row))
            .child(secondary_text(note, palette))
    };
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(setting(
            switch_row(
                "settings-close-window-with-last-tab",
                "Close window with last tab",
                close_window,
                palette,
                change(move |settings| settings.close_window_with_last_tab = !close_window),
            )
            .into_any_element(),
            "When off, closing the last tab leaves the window open on a new tab page.",
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
            "Pinned tabs stay. Closed tabs can be reopened with ⌘⇧T.",
        ))
}
