//! Browser preferences that are independent of the shell and web engine.

use serde::{Deserialize, Serialize};

use super::new_tab::NewTabSettings;
use super::sidebar::SidebarSettings;
use super::spaces::Spaces;

/// The shell's selected appearance.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// How much of the desktop shows through the browser's surfaces.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transparency {
    /// Solid surfaces.
    Off,
    /// A light frosted look that keeps text easy to read.
    #[default]
    Subtle,
    /// More of the desktop shows through.
    Clear,
}

/// The colour the window is tinted with, as in Arc and Zen.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowColor {
    /// The system's grey.
    #[default]
    System,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
}

impl WindowColor {
    pub const ALL: [Self; 9] = [
        Self::System,
        Self::Blue,
        Self::Purple,
        Self::Pink,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
        }
    }
}

/// Where the window shows its tabs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabLayout {
    /// A tab strip in the titlebar, over an address toolbar.
    #[default]
    Horizontal,
    /// A sidebar beside the page.
    Vertical,
}

/// How Photon handles pages that ask to open a separate window.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum PopupPolicy {
    #[default]
    Ask,
    Allow,
    Block,
}

/// User-adjustable browser preferences, persisted as one profile document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct BrowserSettings {
    pub theme: ThemeMode,
    pub transparency: Transparency,
    pub tab_layout: TabLayout,
    pub default_search_engine: String,
    pub popup_policy: PopupPolicy,
    pub new_tab: NewTabSettings,
    pub sidebar: SidebarSettings,
    /// Whether closing the last tab closes its window. When not, the window
    /// stays open on a new tab page.
    pub close_window_with_last_tab: bool,
    /// The spaces, each with its own colour and pinned tabs, and the one
    /// shown.
    pub spaces: Spaces,
    /// Whether unpinned tabs left unused for twelve hours are closed.
    pub archive_tabs: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            transparency: Transparency::default(),
            tab_layout: TabLayout::default(),
            default_search_engine: "google".to_owned(),
            popup_policy: PopupPolicy::Ask,
            new_tab: NewTabSettings::default(),
            sidebar: SidebarSettings::default(),
            close_window_with_last_tab: true,
            spaces: Spaces::default(),
            archive_tabs: false,
        }
    }
}

impl BrowserSettings {
    /// Returns every preference to its default, keeping what people made
    /// themselves: the sidebar's favourites, the new tab page's shortcuts,
    /// and the spaces with their pinned tabs, whose colours reset.
    pub fn reset_preferences(&mut self) {
        let favourites = std::mem::take(&mut self.sidebar.favourites);
        let shortcuts = std::mem::take(&mut self.new_tab.pinned);
        let mut spaces = std::mem::take(&mut self.spaces);
        spaces.reset_colors();
        *self = Self::default();
        self.sidebar.favourites = favourites;
        self.new_tab.pinned = shortcuts;
        self.spaces = spaces;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Shortcut;

    #[test]
    fn reset_keeps_saved_sites_only() {
        let site = Shortcut {
            title: "Example".to_owned(),
            url: "https://example.com/".to_owned(),
        };
        let mut settings = BrowserSettings {
            theme: ThemeMode::Dark,
            tab_layout: TabLayout::Vertical,
            ..BrowserSettings::default()
        };
        let work = settings
            .spaces
            .add(Some("Work".to_owned()), WindowColor::Teal, true);
        settings.sidebar.width = 400;
        settings.sidebar.favourites.push(site.clone());
        settings.new_tab.pinned.push(site.clone());
        settings.new_tab.hidden.push("example.org".to_owned());
        let space = settings.spaces.get_mut(work).unwrap();
        space.pinned_tabs.push(site.clone());
        settings.archive_tabs = true;
        settings.reset_preferences();

        let mut expected = BrowserSettings::default();
        expected.sidebar.favourites.push(site.clone());
        expected.new_tab.pinned.push(site.clone());
        let work = expected
            .spaces
            .add(Some("Work".to_owned()), WindowColor::System, false);
        expected
            .spaces
            .get_mut(work)
            .unwrap()
            .pinned_tabs
            .push(site);
        assert_eq!(settings, expected);
    }
}
