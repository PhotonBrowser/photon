//! What the browser remembers about browsing — visited pages, past searches
//! and page icons — shared by every window and saved to the profile.
//!
//! Changes are saved a moment after they happen, off the main thread, and
//! once more when the app quits. Settings and internal pages read and clear
//! it through the functions here.

use gpui::{App, BorrowAppContext, Global, Task};
use photon_core::{ClearBrowsingData, History, HistoryEntry, Suggestions};
use photon_omnibox::SearchEngines;
use photon_storage::{DataUsage, Profile};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::super::engine::EngineRuntime;
use super::super::trace;
use super::Favicon;

/// How long after a change the history is saved, so a burst of changes
/// saves once.
const SAVE_DELAY: Duration = Duration::from_secs(2);

/// Visited pages, past searches and the icons pages showed.
#[derive(Default)]
pub(super) struct BrowsingHistory {
    history: History,
    /// Where the history is saved; `None` keeps it in memory only.
    profile: Option<Profile>,
    /// Icons by page address. `None` records that the page has no saved icon.
    favicons: HashMap<String, Option<Favicon>>,
    /// The pending save.
    save: Option<Task<()>>,
}

impl Global for BrowsingHistory {}

/// Loads the history saved in `profile` and keeps it saved there.
pub(in crate::platform) fn load_browsing_history(profile: Option<Profile>, cx: &mut App) {
    let history = profile
        .as_ref()
        .map(Profile::load_history)
        .unwrap_or_default();
    cx.set_global(BrowsingHistory {
        history,
        profile,
        ..BrowsingHistory::default()
    });
    cx.on_app_quit(|cx| {
        let save = BrowsingHistory::update(cx, |browsing, _| {
            browsing.save = None;
            browsing.snapshot()
        });
        async move {
            if let Some((profile, history)) = save {
                write_history(&profile, &history);
            }
        }
    })
    .detach();
}

impl BrowsingHistory {
    fn update<R>(cx: &mut App, update: impl FnOnce(&mut Self, &mut App) -> R) -> R {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        cx.update_global(update)
    }

    /// What the omnibox offers for `input`.
    pub(super) fn suggestions(input: &str, engines: &SearchEngines, cx: &App) -> Suggestions {
        match cx.try_global::<Self>() {
            Some(browsing) => browsing.history.omnibox_suggestions_with(input, engines),
            None => History::default().omnibox_suggestions_with(input, engines),
        }
    }

    /// The pages visited most often, for a new tab page.
    pub(super) fn most_visited(limit: usize, cx: &App) -> Vec<HistoryEntry> {
        cx.try_global::<Self>().map_or_else(Vec::new, |browsing| {
            browsing
                .history
                .most_visited(limit)
                .into_iter()
                .cloned()
                .collect()
        })
    }

    /// Measures how much disk the profile's data takes, off the main thread.
    /// `None` when nothing is saved.
    pub(super) fn measure_data_usage(cx: &App) -> Task<Option<DataUsage>> {
        let profile = cx
            .try_global::<Self>()
            .and_then(|browsing| browsing.profile.clone());
        cx.background_executor()
            .spawn(async move { profile.map(|profile| profile.data_usage()) })
    }

    /// The icon `url` showed when last visited, loading it from the profile
    /// the first time it is asked for.
    pub(super) fn favicon(url: &str, cx: &mut App) -> Option<Favicon> {
        // Drawing asks for icons every frame; answer from memory without
        // touching the global once an icon has been looked up.
        if let Some(known) = cx
            .try_global::<Self>()
            .and_then(|browsing| browsing.favicons.get(url))
        {
            return known.clone();
        }
        Self::update(cx, |browsing, _| {
            if let Some(favicon) = browsing.favicons.get(url) {
                return favicon.clone();
            }
            let favicon = browsing
                .profile
                .as_ref()
                .and_then(|profile| profile.load_favicon(url))
                .and_then(Favicon::from_pixels);
            browsing.favicons.insert(url.to_owned(), favicon.clone());
            favicon
        })
    }

    pub(super) fn record_visit(url: &str, title: &str, cx: &mut App) {
        Self::change(cx, |history| {
            history.record_visit(url, title, now());
            true
        });
    }

    pub(super) fn set_title(url: &str, title: &str, cx: &mut App) {
        Self::change(cx, |history| history.set_title(url, title));
    }

    pub(super) fn record_search(query: &str, cx: &mut App) {
        Self::change(cx, |history| {
            history.record_search(query, now());
            true
        });
    }

    /// Keeps `url`'s icon, saving it when it differs from the one kept.
    pub(super) fn set_favicon(url: &str, favicon: &Favicon, cx: &mut App) {
        Self::update(cx, |browsing, cx| {
            let known = browsing.favicons.get(url).and_then(Option::as_ref);
            if known.is_some_and(|known| known.key == favicon.key) {
                return;
            }
            browsing
                .favicons
                .insert(url.to_owned(), Some(favicon.clone()));
            let Some(profile) = browsing.profile.clone() else {
                return;
            };
            let (url, pixels) = (url.to_owned(), favicon.pixels().clone());
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = profile.save_favicon(&url, &pixels) {
                        trace(format_args!("saving page icon: {error}"));
                    }
                })
                .detach();
        });
    }

    /// Forgets a visited page and its icon.
    pub(super) fn remove_page(url: &str, cx: &mut App) {
        Self::update(cx, |browsing, _| browsing.favicons.remove(url));
        Self::change(cx, |history| {
            history.remove_page(url);
            true
        });
        Self::prune_favicons(cx);
    }

    /// Forgets a past search.
    pub(super) fn remove_search(query: &str, cx: &mut App) {
        Self::change(cx, |history| {
            history.remove_search(query);
            true
        });
    }

    /// Deletes what `request` asks for: Photon's history, searches and icons
    /// here, and website data in the Engine. Calls `done` once all of it is
    /// gone.
    pub(super) fn clear(
        request: ClearBrowsingData,
        runtime: &EngineRuntime,
        done: impl FnOnce(&mut App) + 'static,
        cx: &mut App,
    ) {
        Self::change(cx, |history| {
            history.clear(&request);
            true
        });
        if request.history {
            Self::update(cx, |browsing, _| browsing.favicons.clear());
            Self::prune_favicons(cx);
        }
        let app = cx.to_async();
        runtime.clear_browsing_data(&request, move || {
            app.spawn(async move |cx| cx.update(done)).detach();
        });
    }

    /// Changes the history and, when `change` reports it changed, saves it
    /// shortly.
    fn change(cx: &mut App, change: impl FnOnce(&mut History) -> bool) {
        Self::update(cx, |browsing, cx| {
            let changed = change(&mut browsing.history);
            if changed && browsing.profile.is_some() && browsing.save.is_none() {
                browsing.save = Some(cx.spawn(async move |cx| {
                    cx.background_executor().timer(SAVE_DELAY).await;
                    let save = cx.update_global::<Self, _>(|browsing, _| {
                        browsing.save = None;
                        browsing.snapshot()
                    });
                    if let Some((profile, history)) = save {
                        cx.background_executor()
                            .spawn(async move { write_history(&profile, &history) })
                            .await;
                    }
                }));
            }
        });
    }

    /// Removes saved icons of pages no longer in the history, off the main thread.
    fn prune_favicons(cx: &mut App) {
        let Some((profile, history)) = Self::update(cx, |browsing, _| browsing.snapshot()) else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = profile.prune_favicons(&history) {
                    trace(format_args!("removing page icons: {error}"));
                }
            })
            .detach();
    }

    fn snapshot(&self) -> Option<(Profile, History)> {
        Some((self.profile.clone()?, self.history.clone()))
    }
}

fn write_history(profile: &Profile, history: &History) {
    if let Err(error) = profile.save_history(history) {
        trace(format_args!("saving history: {error}"));
    }
}

/// The current time in Unix seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
