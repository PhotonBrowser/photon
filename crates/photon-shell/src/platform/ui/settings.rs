//! The browser's settings, shared by every window and saved to the profile.
//!
//! Views read settings with [`Settings::get`] and change them with
//! [`Settings::update`], which notifies everything observing the `Settings`
//! global, so every window follows a change. Changes are saved a moment
//! after they happen, off the main thread, and once more when the app quits.
//! Edits to the profile's config file are picked up while the app is running.

use gpui::{App, BorrowAppContext, Global, Task, WindowAppearance};
use photon_core::SearchEngines;
use photon_core::{BrowserSettings, ThemeMode};
use photon_storage::{Profile, ProfileWriteTicket};
use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::super::app_update::update_app;
use super::super::trace;
use super::motion::{MotionConfig, MotionTimings};
use super::workspace_layout::WorkspaceLayout;

/// How long after a change the settings are saved, so a burst of changes,
/// as while dragging the sidebar's edge, saves once.
const SAVE_DELAY: Duration = Duration::from_millis(500);
/// How often the profile config is checked for edits made outside Photon.
const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_millis(200);
/// Wait for an editor's write to settle before parsing its file.
const CONFIG_RELOAD_SETTLE: Duration = Duration::from_millis(100);
/// Keep the loading chip visible long enough to read on fast reloads.
const CONFIG_RELOAD_MINIMUM_DURATION: Duration = Duration::from_millis(250);
/// How long the completed config reload chip remains visible.
const CONFIG_RELOAD_NOTICE_DURATION: Duration = Duration::from_secs(3);

type UiSettingsRevision = Arc<Mutex<Option<u64>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigReloadNotice {
    Reloading,
    Reloaded(Instant),
    Failed(Instant),
}

/// The settings, and where they are saved; `None` keeps them in memory only.
pub(super) struct Settings {
    settings: BrowserSettings,
    profile: Option<Profile>,
    /// The pending save.
    save: Option<Task<()>>,
    /// Watches the profile config for edits made outside Photon.
    _reload: Option<Task<()>>,
    /// The latest config revision written by settings controls in Photon.
    ui_settings_revision: UiSettingsRevision,
    config_reload_notice: Option<ConfigReloadNotice>,
    /// The workspace layout the settings resolve to, worked out on first
    /// use after each change, as drawing asks for it many times a frame.
    layout: OnceCell<Rc<WorkspaceLayout>>,
}

impl Global for Settings {}

/// Installs settings already loaded off the application thread and keeps them
/// saved in `profile`.
pub(in crate::platform) fn load_settings(
    profile: Option<Profile>,
    mut settings: BrowserSettings,
    revision: Option<u64>,
    cx: &mut App,
) {
    let ui_settings_revision = Arc::new(Mutex::new(None));
    settings = normalize_settings(profile.as_ref(), settings, ui_settings_revision.clone(), cx);
    let reload = profile
        .clone()
        .map(|profile| watch_settings(profile, revision, ui_settings_revision.clone(), cx));
    cx.set_global(Settings {
        settings,
        profile,
        save: None,
        _reload: reload,
        ui_settings_revision: ui_settings_revision.clone(),
        config_reload_notice: None,
        layout: OnceCell::new(),
    });
    cx.on_app_quit(|cx| {
        let save = cx.update_global::<Settings, _>(|store, _| {
            store.save = None;
            store.snapshot().map(|(profile, settings, ticket)| {
                (
                    profile,
                    settings,
                    ticket,
                    store.ui_settings_revision.clone(),
                )
            })
        });
        async move {
            if let Some((profile, settings, ticket, ui_settings_revision)) = save {
                write_settings(&profile, &settings, ticket, &ui_settings_revision);
            }
        }
    })
    .detach();
}

fn normalize_settings(
    profile: Option<&Profile>,
    mut settings: BrowserSettings,
    ui_settings_revision: UiSettingsRevision,
    cx: &mut App,
) -> BrowserSettings {
    if settings.migrate_legacy_favourites()
        && let Some(profile) = profile
    {
        let profile = profile.clone();
        let settings = settings.clone();
        let ticket = profile.begin_settings_write();
        cx.background_executor()
            .spawn(async move {
                match profile.save_settings_if_current(ticket, &settings) {
                    Ok(true) => remember_ui_settings_revision(&profile, &ui_settings_revision),
                    Ok(false) => {}
                    Err(error) => trace(format_args!("saving migrated favourites: {error}")),
                }
            })
            .detach();
    }
    // A search engine this build does not know falls back to the default.
    if SearchEngines::builtin()
        .get(&settings.default_search_engine)
        .is_none()
    {
        settings.default_search_engine = BrowserSettings::default().default_search_engine;
    }
    settings
}

fn watch_settings(
    profile: Profile,
    mut revision: Option<u64>,
    ui_settings_revision: UiSettingsRevision,
    cx: &mut App,
) -> Task<()> {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(CONFIG_RELOAD_INTERVAL).await;
            let profile_to_check = profile.clone();
            let Some(candidate) = cx
                .background_executor()
                .spawn(async move { profile_to_check.settings_revision().ok() })
                .await
            else {
                continue;
            };
            if revision == Some(candidate) {
                continue;
            }

            cx.background_executor().timer(CONFIG_RELOAD_SETTLE).await;
            let profile_to_check = profile.clone();
            let confirmed = cx
                .background_executor()
                .spawn(async move { profile_to_check.settings_revision().ok() })
                .await;
            if confirmed != Some(candidate) {
                continue;
            }

            let is_ui_settings_write = {
                let mut ui_revision = ui_settings_revision
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if *ui_revision == Some(candidate) {
                    *ui_revision = None;
                    true
                } else {
                    false
                }
            };
            if is_ui_settings_write {
                revision = Some(candidate);
                continue;
            }

            // Remember invalid edits too, and retry only after the file changes
            // again. This keeps an in-progress or malformed edit from replacing
            // the last settings that were loaded successfully.
            revision = Some(candidate);
            let profile_to_prepare = profile.clone();
            if update_app(cx, move |cx| {
                Settings::prepare_reload(&profile_to_prepare, cx);
            })
            .await
            .is_none()
            {
                break;
            }
            let reloading_started = Instant::now();
            let profile_to_reload = profile.clone();
            match cx
                .background_executor()
                .spawn(async move { profile_to_reload.reload_settings() })
                .await
            {
                Ok((settings, loaded_revision)) => {
                    revision = Some(loaded_revision);
                    let remaining =
                        CONFIG_RELOAD_MINIMUM_DURATION.saturating_sub(reloading_started.elapsed());
                    if !remaining.is_zero() {
                        cx.background_executor().timer(remaining).await;
                    }
                    let profile = profile.clone();
                    if update_app(cx, move |cx| {
                        Settings::apply_reload(&profile, settings, cx);
                    })
                    .await
                    .is_none()
                    {
                        break;
                    }
                }
                Err(error) => {
                    trace(format_args!("reloading config.toml: {error}"));
                    if update_app(cx, Settings::fail_reload).await.is_none() {
                        break;
                    }
                }
            }
        }
    })
}

impl Settings {
    fn prepare_reload(profile: &Profile, cx: &mut App) {
        profile.begin_settings_write();
        cx.update_global::<Self, _>(|store, _| {
            store.save = None;
            store.config_reload_notice = Some(ConfigReloadNotice::Reloading);
        });
    }

    fn apply_reload(profile: &Profile, settings: BrowserSettings, cx: &mut App) {
        let ui_settings_revision = cx.global::<Self>().ui_settings_revision.clone();
        let settings = normalize_settings(Some(profile), settings, ui_settings_revision, cx);
        cx.update_global::<Self, _>(|store, _| {
            // A file reload supersedes any unsaved in-memory changes and their
            // pending write; the profile write ticket was already invalidated.
            store.save = None;
            if store.settings != settings {
                store.settings = settings;
                store.layout = OnceCell::new();
            }
            store.config_reload_notice = Some(ConfigReloadNotice::Reloaded(Instant::now()));
        });
    }

    fn fail_reload(cx: &mut App) {
        cx.update_global::<Self, _>(|store, _| {
            store.config_reload_notice = Some(ConfigReloadNotice::Failed(Instant::now()));
        });
    }

    pub(super) fn config_reload_notice(cx: &App) -> Option<ConfigReloadNotice> {
        cx.global::<Self>().config_reload_notice
    }

    pub(super) fn config_reload_notice_duration() -> Duration {
        CONFIG_RELOAD_NOTICE_DURATION
    }

    /// The reusable motion framework's view of the saved and system settings.
    pub(super) fn motion(cx: &App) -> MotionConfig {
        let settings = Self::get(cx).animations;
        MotionConfig {
            enabled: settings.enabled,
            reduce_motion: cx.reduce_motion(),
            effect_strength: f32::from(settings.effect_strength_percent.min(300)) / 100.0,
            timings: MotionTimings {
                quick: Duration::from_millis(u64::from(settings.quick_duration_ms.min(2_000))),
                standard: Duration::from_millis(u64::from(
                    settings.standard_duration_ms.min(2_000),
                )),
                gentle: Duration::from_millis(u64::from(settings.gentle_duration_ms.min(2_000))),
            },
        }
    }

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
                    store.snapshot().map(|(profile, settings, ticket)| {
                        (
                            profile,
                            settings,
                            ticket,
                            store.ui_settings_revision.clone(),
                        )
                    })
                })
            })
            .await
            .flatten();
            // Clearing `save` above dropped this task's handle, which
            // cancels it at its next await, so the write runs detached.
            if let Some((profile, settings, ticket, ui_settings_revision)) = save {
                cx.background_executor()
                    .spawn(async move {
                        write_settings(&profile, &settings, ticket, &ui_settings_revision)
                    })
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

    fn snapshot(&self) -> Option<(Profile, BrowserSettings, ProfileWriteTicket)> {
        let profile = self.profile.clone()?;
        let ticket = profile.begin_settings_write();
        Some((profile, self.settings.clone(), ticket))
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

fn write_settings(
    profile: &Profile,
    settings: &BrowserSettings,
    ticket: ProfileWriteTicket,
    ui_settings_revision: &UiSettingsRevision,
) {
    match profile.save_settings_if_current(ticket, settings) {
        Ok(true) => remember_ui_settings_revision(profile, ui_settings_revision),
        Ok(false) => {}
        Err(error) => eprintln!("failed to save settings: {error}"),
    }
}

fn remember_ui_settings_revision(profile: &Profile, ui_settings_revision: &UiSettingsRevision) {
    if let Ok(revision) = profile.settings_revision() {
        *ui_settings_revision
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(revision);
    }
}
