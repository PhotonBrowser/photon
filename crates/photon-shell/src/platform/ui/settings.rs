//! The browser's settings, shared by every window and saved to the profile.
//!
//! Views read settings with [`Settings::get`] and change them with
//! [`Settings::update`], which notifies everything observing the `Settings`
//! global, so every window follows a change. Changes are saved a moment
//! after they happen, off the main thread, and once more when the app quits.

use gpui::{App, BorrowAppContext, Global, Task, WindowAppearance};
use photon_core::{BrowserSettings, ThemeMode};
use photon_omnibox::SearchEngines;
use photon_storage::Profile;
use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;

use super::super::app_update::update_app;
use super::super::trace;
use super::workspace_layout::WorkspaceLayout;

/// How long after a change the settings are saved, so a burst of changes,
/// as while dragging the sidebar's edge, saves once.
const SAVE_DELAY: Duration = Duration::from_millis(500);

/// Keeps saves in the order they were made.
static WRITING: Mutex<()> = Mutex::new(());

/// The settings, and where they are saved; `None` keeps them in memory only.
pub(super) struct Settings {
    settings: BrowserSettings,
    profile: Option<Profile>,
    /// The pending save.
    save: Option<Task<()>>,
    /// The workspace layout the settings resolve to, worked out on first
    /// use after each change, as drawing asks for it many times a frame.
    layout: OnceCell<Rc<WorkspaceLayout>>,
}

impl Global for Settings {}

/// Loads the settings saved in `profile` and keeps them saved there.
pub(in crate::platform) fn load_settings(profile: Option<Profile>, cx: &mut App) {
    let mut settings = profile
        .as_ref()
        .map(Profile::load_settings)
        .unwrap_or_default();
    if settings.migrate_legacy_favourites()
        && let Some(profile) = &profile
        && let Err(error) = profile.save_settings(&settings)
    {
        trace(format_args!("saving migrated favourites: {error}"));
    }
    // A search engine this build does not know falls back to the default.
    if SearchEngines::builtin()
        .get(&settings.default_search_engine)
        .is_none()
    {
        settings.default_search_engine = BrowserSettings::default().default_search_engine;
    }
    cx.set_global(Settings {
        settings,
        profile,
        save: None,
        layout: OnceCell::new(),
    });
    cx.on_app_quit(|cx| {
        let save = cx.update_global::<Settings, _>(|store, _| {
            store.save = None;
            store.snapshot()
        });
        async move {
            if let Some((profile, settings)) = save {
                write_settings(&profile, &settings);
            }
        }
    })
    .detach();
}

impl Settings {
    pub(super) fn get(cx: &App) -> &BrowserSettings {
        &cx.global::<Self>().settings
    }

    /// Whether changes are saved between launches.
    pub(super) fn is_saved(cx: &App) -> bool {
        cx.global::<Self>().profile.is_some()
    }

    pub(super) fn config_path(cx: &App) -> Option<std::path::PathBuf> {
        cx.global::<Self>()
            .profile
            .as_ref()
            .map(|profile| profile.settings_path().to_path_buf())
    }

    /// Changes the settings, notifies observers, and saves them shortly.
    pub(super) fn update(cx: &mut App, update: impl FnOnce(&mut BrowserSettings)) {
        cx.update_global::<Self, _>(|store, cx| {
            update(&mut store.settings);
            store.layout = OnceCell::new();
            store.save_soon(cx);
        });
    }

    /// Saves the settings a moment from now, unless a save is waiting.
    fn save_soon(&mut self, cx: &mut App) {
        if self.profile.is_none() || self.save.is_some() {
            return;
        }
        self.save = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let save = update_app(cx, |cx| {
                cx.update_global::<Self, _>(|store, _| {
                    store.save = None;
                    store.snapshot()
                })
            })
            .await
            .flatten();
            // Clearing `save` above dropped this task's handle, which
            // cancels it at its next await, so the write runs detached.
            if let Some((profile, settings)) = save {
                cx.background_executor()
                    .spawn(async move { write_settings(&profile, &settings) })
                    .detach();
            }
        }));
    }

    /// The workspace layout, resolved by `resolve` once per change.
    pub(super) fn layout(
        cx: &App,
        resolve: impl FnOnce(&BrowserSettings) -> WorkspaceLayout,
    ) -> Rc<WorkspaceLayout> {
        let store = cx.global::<Self>();
        store
            .layout
            .get_or_init(|| Rc::new(resolve(&store.settings)))
            .clone()
    }

    fn snapshot(&self) -> Option<(Profile, BrowserSettings)> {
        Some((self.profile.clone()?, self.settings.clone()))
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

fn write_settings(profile: &Profile, settings: &BrowserSettings) {
    let _writing = WRITING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Err(error) = profile.save_settings(settings) {
        trace(format_args!("saving settings: {error}"));
    }
}
