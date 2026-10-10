//! Browser preferences that are independent of the shell and web engine.

use serde::{Deserialize, Serialize};

use super::new_tab::{NewTabSettings, Shortcut};
use super::sidebar::SidebarSettings;

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
    pub window_color: WindowColor,
    /// Whether the window colour fades into a neighbouring one.
    pub window_gradient: bool,
    pub tab_layout: TabLayout,
    pub default_search_engine: String,
    pub popup_policy: PopupPolicy,
    pub new_tab: NewTabSettings,
    pub sidebar: SidebarSettings,
    /// Whether closing the last tab closes its window. When not, the window
    /// stays open on a new tab page.
    pub close_window_with_last_tab: bool,
    /// Tabs pinned above the others, at the addresses they were pinned at,
    /// opened again at launch.
    pub pinned_tabs: Vec<Shortcut>,
    /// Whether unpinned tabs left unused for twelve hours are closed.
    pub archive_tabs: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            transparency: Transparency::default(),
            window_color: WindowColor::default(),
            window_gradient: false,
            tab_layout: TabLayout::default(),
            default_search_engine: "google".to_owned(),
            popup_policy: PopupPolicy::Ask,
            new_tab: NewTabSettings::default(),
            sidebar: SidebarSettings::default(),
            close_window_with_last_tab: true,
            pinned_tabs: Vec::new(),
            archive_tabs: false,
        }
    }
}

impl BrowserSettings {
    /// Returns every preference to its default, keeping the sites people
    /// saved themselves: the sidebar's favourites, the new tab page's
    /// shortcuts and the pinned tabs.
    pub fn reset_preferences(&mut self) {
        let favourites = std::mem::take(&mut self.sidebar.favourites);
        let shortcuts = std::mem::take(&mut self.new_tab.pinned);
        let pinned_tabs = std::mem::take(&mut self.pinned_tabs);
        *self = Self::default();
        self.sidebar.favourites = favourites;
        self.new_tab.pinned = shortcuts;
        self.pinned_tabs = pinned_tabs;
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
            window_color: WindowColor::Teal,
            tab_layout: TabLayout::Vertical,
            ..BrowserSettings::default()
        };
        settings.sidebar.width = 400;
        settings.sidebar.favourites.push(site.clone());
        settings.new_tab.pinned.push(site.clone());
        settings.new_tab.hidden.push("example.org".to_owned());
        settings.pinned_tabs.push(site.clone());
        settings.archive_tabs = true;
        settings.reset_preferences();

        let mut expected = BrowserSettings::default();
        expected.sidebar.favourites.push(site.clone());
        expected.new_tab.pinned.push(site.clone());
        expected.pinned_tabs.push(site);
        assert_eq!(settings, expected);
    }
}
