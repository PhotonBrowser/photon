//! Browser preferences that are independent of the shell and web engine.

use serde::{Deserialize, Serialize};

use super::new_tab::NewTabSettings;

/// The shell's selected appearance.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
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
    pub default_search_engine: String,
    pub popup_policy: PopupPolicy,
    pub new_tab: NewTabSettings,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            default_search_engine: "google".to_owned(),
            popup_policy: PopupPolicy::Ask,
            new_tab: NewTabSettings::default(),
        }
    }
}
