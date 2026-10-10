//! Browser preferences that are independent of the shell and web engine.

use serde::{Deserialize, Serialize};

use super::new_tab::NewTabSettings;
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
        }
    }
}
