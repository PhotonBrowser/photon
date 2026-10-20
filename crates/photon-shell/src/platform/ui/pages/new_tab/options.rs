//! What the new tab page's layout options do, shared by its customise panel
//! and the settings page, which each lay them out in their own style.

use gpui::App;
use photon_core::{NewTabSettings, SHORTCUT_COUNTS};

use super::super::super::ClickHandler;
use super::super::super::controls::Choice;
use super::super::super::settings::Settings;
use super::super::change;

/// The new tab page's current layout options.
pub(in super::super) struct LayoutOptions {
    pub(in super::super) show_logo: bool,
    pub(in super::super) show_shortcuts: bool,
    pub(in super::super) has_removed: bool,
}

impl LayoutOptions {
    pub(in super::super) fn read(cx: &App) -> Self {
        let layout = &Settings::get(cx).new_tab;
        Self {
            show_logo: layout.show_logo,
            show_shortcuts: layout.show_shortcuts,
            has_removed: layout.has_removed(),
        }
    }

    pub(in super::super) fn toggle_logo(&self) -> ClickHandler {
        let show = !self.show_logo;
        change(move |settings| settings.new_tab.show_logo = show)
    }

    pub(in super::super) fn toggle_shortcuts(&self) -> ClickHandler {
        let show = !self.show_shortcuts;
        change(move |settings| settings.new_tab.show_shortcuts = show)
    }

    pub(in super::super) fn restore_removed(&self) -> ClickHandler {
        Box::new(|_, _, cx| {
            if !Settings::get(cx).new_tab.has_removed() {
                return;
            }
            Settings::update(cx, |settings| {
                NewTabSettings::restore_removed(&mut settings.new_tab)
            });
            super::super::super::show_alert(cx, "Removed shortcuts restored");
        })
    }

    /// The numbers of shortcuts to choose from, the current one chosen.
    pub(in super::super) fn shortcut_counts(cx: &App) -> Vec<Choice> {
        let limit = Settings::get(cx).new_tab.shortcut_limit();
        SHORTCUT_COUNTS
            .into_iter()
            .map(|count| Choice {
                label: count.to_string().into(),
                chosen: count == limit,
                on_choose: change(move |settings| settings.new_tab.shortcut_count = count),
            })
            .collect()
    }
}
