//! The browser's settings, shared by every window and saved to the profile.
//!
//! Views read settings with [`Settings::get`] and change them with
//! [`Settings::update`], which saves them and notifies everything observing
//! the `Settings` global, so every window follows a change.

use gpui::{App, BorrowAppContext, Global, WindowAppearance};
use photon_core::{BrowserSettings, ThemeMode};
use photon_omnibox::SearchEngines;
use photon_storage::Profile;

use super::super::trace;

/// The settings, and where they are saved; `None` keeps them in memory only.
pub(super) struct Settings {
    settings: BrowserSettings,
    profile: Option<Profile>,
}

impl Global for Settings {}

/// Loads the settings saved in `profile` and keeps them saved there.
pub(in crate::platform) fn load_settings(profile: Option<Profile>, cx: &mut App) {
    let mut settings = profile
        .as_ref()
        .map(Profile::load_settings)
        .unwrap_or_default();
    // A search engine this build does not know falls back to the default.
    if SearchEngines::builtin()
        .get(&settings.default_search_engine)
        .is_none()
    {
        settings.default_search_engine = BrowserSettings::default().default_search_engine;
    }
    cx.set_global(Settings { settings, profile });
}

impl Settings {
    pub(super) fn get(cx: &App) -> &BrowserSettings {
        &cx.global::<Self>().settings
    }

    /// Whether changes are saved between launches.
    pub(super) fn is_saved(cx: &App) -> bool {
        cx.global::<Self>().profile.is_some()
    }

    /// Changes the settings, saves them, and notifies observers. The file is
    /// small, so it is written at once, which keeps saves in order.
    pub(super) fn update(cx: &mut App, update: impl FnOnce(&mut BrowserSettings)) {
        cx.update_global::<Self, _>(|store, _| {
            update(&mut store.settings);
            if let Some(profile) = &store.profile
                && let Err(error) = profile.save_settings(&store.settings)
            {
                trace(format_args!("saving settings: {error}"));
            }
        });
    }

    /// The search engines, with the chosen one as the default.
    pub(super) fn search_engines(cx: &App) -> SearchEngines {
        let mut engines = SearchEngines::builtin();
        engines.set_default(&Self::get(cx).default_search_engine);
        engines
    }

    /// The appearance windows use: the chosen one, or the system's.
    pub(super) fn appearance(system: WindowAppearance, cx: &App) -> WindowAppearance {
        Self::chosen_appearance(cx).unwrap_or(system)
    }

    /// The chosen appearance, or `None` to follow the system.
    pub(super) fn chosen_appearance(cx: &App) -> Option<WindowAppearance> {
        match Self::get(cx).theme {
            ThemeMode::System => None,
            ThemeMode::Light => Some(WindowAppearance::Light),
            ThemeMode::Dark => Some(WindowAppearance::Dark),
        }
    }
}
