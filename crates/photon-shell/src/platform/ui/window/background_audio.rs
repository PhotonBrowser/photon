//! Sound from tabs out of view: the footer shows a speaker while any tab
//! other than the one shown plays sound, and its menu goes to or mutes each.

use gpui::{Context, Menu, MenuItem, Window};

use super::BrowserWindow;
use super::menu::{MenuCommand, command_item};
use super::tab_summary::tab_summary;

/// The longest a tab's title runs in the menu before it is cut short.
const MAX_TITLE_CHARS: usize = 40;

struct AudibleTab {
    index: usize,
    title: String,
    muted: bool,
}

impl BrowserWindow {
    /// The tabs other than the active one that are playing sound.
    fn background_audio_tabs(&self, cx: &Context<Self>) -> Vec<AudibleTab> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != self.active_tab)
            .filter_map(|(index, tab)| {
                let view = tab.content.webview()?.read(cx);
                view.audio_playing.then(|| AudibleTab {
                    index,
                    title: short_title(&tab_summary(tab, cx).title),
                    muted: view.audio_muted,
                })
            })
            .collect()
    }

    /// Whether every tab out of view that plays sound is muted, or `None`
    /// when none plays sound.
    pub(super) fn background_audio_muted(&self, cx: &Context<Self>) -> Option<bool> {
        let tabs = self.background_audio_tabs(cx);
        (!tabs.is_empty()).then(|| tabs.iter().all(|tab| tab.muted))
    }

    pub(super) fn show_background_audio_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tabs = self.background_audio_tabs(cx);
        let items = match tabs.as_slice() {
            [] => return,
            [tab] => tab_items(tab, true),
            tabs => tabs
                .iter()
                .map(|tab| {
                    MenuItem::submenu(Menu::new(tab.title.clone()).items(tab_items(tab, false)))
                })
                .collect(),
        };
        self.show_context_menu(window, items, cx);
    }
}

/// Going to the tab and muting it, named after the tab when `named`.
fn tab_items(tab: &AudibleTab, named: bool) -> Vec<MenuItem> {
    let (go, mute) = match (named, tab.muted) {
        (true, false) => (
            format!("Go to {}", tab.title),
            format!("Mute {}", tab.title),
        ),
        (true, true) => (
            format!("Go to {}", tab.title),
            format!("Unmute {}", tab.title),
        ),
        (false, false) => ("Go to Tab".to_owned(), "Mute".to_owned()),
        (false, true) => ("Go to Tab".to_owned(), "Unmute".to_owned()),
    };
    vec![
        command_item(go, MenuCommand::ActivateTab(tab.index)),
        command_item(mute, MenuCommand::ToggleTabAudio(tab.index)),
    ]
}

fn short_title(title: &str) -> String {
    let title = title.trim();
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.to_owned();
    }
    let cut: String = title.chars().take(MAX_TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::short_title;

    #[test]
    fn long_titles_are_cut_short() {
        assert_eq!(short_title("  Radio  "), "Radio");
        let long = "a".repeat(60);
        let short = short_title(&long);
        assert_eq!(short.chars().count(), super::MAX_TITLE_CHARS);
        assert!(short.ends_with('…'));
    }
}
