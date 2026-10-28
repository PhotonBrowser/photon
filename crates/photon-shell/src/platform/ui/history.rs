//! What the browser remembers about browsing — visited pages, past searches
//! and page icons — shared by every window and saved to the profile.
//!
//! Changes are saved a moment after they happen, off the main thread, and
//! once more when the app quits. Settings and internal pages read and clear
//! it through the functions here.

use gpui::{App, BorrowAppContext, Global, Task};
use photon_core::{ClearBrowsingData, History, HistoryEntry};
use photon_storage::{DataUsage, Profile, ProfileWriteTicket};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::super::engine::EngineRuntime;
use super::super::trace;
use super::Favicon;
use crate::platform::app_update::update_app;

/// How long after a change the history is saved, so a burst of changes
/// saves once.
const SAVE_DELAY: Duration = Duration::from_secs(2);

struct MostVisitedPages {
    limit: usize,
    pages: Vec<HistoryEntry>,
}

/// Visited pages, past searches and the icons pages showed.
#[derive(Default)]
pub(super) struct BrowsingHistory {
    history: Arc<History>,
    /// Shared by new tab pages; icon loads do not invalidate page rankings.
    most_visited: RefCell<Option<MostVisitedPages>>,
    /// Where the history is saved; `None` keeps it in memory only.
    profile: Option<Profile>,
    /// Icons by page address. `None` records that the page has no saved icon.
    favicons: HashMap<String, Option<Favicon>>,
    /// Loads keyed by page address, so a redraw shares an in-flight read.
    favicon_loads: HashMap<String, u64>,
    next_favicon_load: u64,
    /// The pending save.
    save: Option<Task<()>>,
    /// Orders icon saves against pruning, which run off the main thread.
    favicon_writes: Arc<FaviconWrites>,
}

/// Keeps an icon saved before a page was forgotten from landing after the
/// prune that removes it.
#[derive(Default)]
struct FaviconWrites {
    /// Held while an icon is saved or pruned.
    lock: Mutex<()>,
    /// Bumped when the history is cleared; saves queued earlier are dropped.
    generation: AtomicU64,
    /// Pages forgotten one at a time, whose queued saves are dropped. A page
    /// leaves it when its icon is kept again, on a later visit.
    forgotten: Mutex<HashSet<String>>,
}

impl FaviconWrites {
    fn forget(&self, url: &str) {
        self.forgotten
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(url.to_owned());
    }

    fn remember(&self, url: &str) {
        self.forgotten
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(url);
    }

    fn is_forgotten(&self, url: &str) -> bool {
        self.forgotten
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(url)
    }
}

impl Global for BrowsingHistory {}

/// Installs history already loaded off the application thread and keeps it
/// saved in `profile`.
pub(in crate::platform) fn load_browsing_history(
    profile: Option<Profile>,
    history: History,
    cx: &mut App,
) {
    cx.set_global(BrowsingHistory {
        history: Arc::new(history),
        profile,
        ..BrowsingHistory::default()
    });
    cx.on_app_quit(|cx| {
        let save = BrowsingHistory::update(cx, |browsing, _| {
            browsing.save = None;
            browsing.snapshot_for_write()
        });
        async move {
            if let Some((profile, history, ticket)) = save {
                write_history(&profile, &history, ticket);
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

    /// A cheap snapshot for running an omnibox query away from the UI thread.
    pub(super) fn suggestion_history(cx: &App) -> Arc<History> {
        cx.try_global::<Self>().map_or_else(
            || Arc::new(History::default()),
            |browsing| browsing.history.clone(),
        )
    }

    /// The pages visited most often, for a new tab page.
    pub(super) fn most_visited(limit: usize, cx: &App) -> Vec<HistoryEntry> {
        if limit == 0 {
            return Vec::new();
        }
        cx.try_global::<Self>().map_or_else(Vec::new, |browsing| {
            let mut cached = browsing.most_visited.borrow_mut();
            if cached.as_ref().is_some_and(|ranked| ranked.limit < limit) {
                *cached = None;
            }
            let ranked = cached.get_or_insert_with(|| MostVisitedPages {
                limit,
                pages: browsing
                    .history
                    .most_visited(limit)
                    .into_iter()
                    .cloned()
                    .collect(),
            });
            ranked.pages.iter().take(limit).cloned().collect()
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

    /// The icon `url` showed when last visited, loading it from the profile in
    /// the background the first time it is asked for.
    pub(super) fn favicon(url: &str, cx: &mut App) -> Option<Favicon> {
        // This is called during rendering. Mutable global access notifies all
        // history observers, even if its closure does not change anything.
        if let Some(browsing) = cx.try_global::<Self>() {
            if let Some(favicon) = browsing.favicons.get(url) {
                return favicon.clone();
            }
            if browsing.favicon_loads.contains_key(url) {
                return None;
            }
        }
        let (cached, request) = Self::update(cx, |browsing, _| {
            if let Some(favicon) = browsing.favicons.get(url) {
                return (favicon.clone(), None);
            }
            let Some(profile) = browsing.profile.clone() else {
                browsing.favicons.insert(url.to_owned(), None);
                return (None, None);
            };
            let Some(request_id) = browsing.reserve_favicon_load(url) else {
                return (None, None);
            };
            (None, Some((profile, request_id)))
        });
        if let Some((profile, request_id)) = request {
            let url = url.to_owned();
            let read_url = url.clone();
            let read = cx.background_executor().spawn(async move {
                profile
                    .load_favicon(&read_url)
                    .and_then(Favicon::from_pixels)
            });
            let app = cx.to_async();
            app.spawn(async move |cx| {
                let favicon = read.await;
                update_app(cx, move |cx| {
                    if !cx.has_global::<Self>() {
                        return;
                    }
                    cx.update_global::<Self, _>(|browsing, _| {
                        Self::complete_favicon_load(Some(browsing), &url, request_id, favicon);
                    });
                })
                .await;
            })
            .detach();
        }
        cached
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
            browsing.favicon_loads.remove(url);
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
            let (url, favicon) = (url.to_owned(), favicon.clone());
            let writes = browsing.favicon_writes.clone();
            let generation = writes.generation.load(Ordering::SeqCst);
            writes.remember(&url);
            cx.background_executor()
                .spawn(async move {
                    let _held = writes
                        .lock
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    if writes.generation.load(Ordering::SeqCst) != generation
                        || writes.is_forgotten(&url)
                    {
                        return;
                    }
                    if let Err(error) = profile.save_favicon(&url, favicon.pixels()) {
                        trace(format_args!("saving page icon: {error}"));
                    }
                })
                .detach();
        });
    }

    /// Forgets a visited page and its icon.
    pub(super) fn remove_page(url: &str, cx: &mut App) {
        Self::update(cx, |browsing, _| {
            browsing.favicons.remove(url);
            browsing.favicon_loads.remove(url);
            browsing.favicon_writes.forget(url);
        });
        Self::change(cx, |history| {
            history.remove_page(url);
            true
        });
        Self::prune_favicons(false, cx);
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
            Self::update(cx, |browsing, _| {
                browsing.favicons.clear();
                browsing.favicon_loads.clear();
            });
            Self::prune_favicons(true, cx);
        }
        let app = cx.to_async();
        runtime.clear_browsing_data(&request, move || {
            app.spawn(async move |cx| update_app(cx, done).await)
                .detach();
        });
    }

    /// Changes the history and, when `change` reports it changed, saves it
    /// shortly.
    fn change(cx: &mut App, change: impl FnOnce(&mut History) -> bool) {
        Self::update(cx, |browsing, cx| {
            let changed = change(Arc::make_mut(&mut browsing.history));
            if changed {
                *browsing.most_visited.get_mut() = None;
            }
            if changed && browsing.profile.is_some() && browsing.save.is_none() {
                browsing.save = Some(cx.spawn(async move |cx| {
                    cx.background_executor().timer(SAVE_DELAY).await;
                    let save = update_app(cx, |cx| {
                        cx.update_global::<Self, _>(|browsing, _| {
                            browsing.save = None;
                            browsing.snapshot_for_write()
                        })
                    })
                    .await
                    .flatten();
                    // Clearing `save` above dropped this task's handle, which
                    // cancels it at its next await, so the write runs detached.
                    if let Some((profile, history, ticket)) = save {
                        cx.background_executor()
                            .spawn(async move { write_history(&profile, &history, ticket) })
                            .detach();
                    }
                }));
            }
        });
    }

    /// Removes saved icons of pages no longer in the history, off the main
    /// thread. Clearing the whole history also drops every icon save still
    /// queued; forgetting one page drops only its own.
    fn prune_favicons(clearing: bool, cx: &mut App) {
        let Some((profile, history, writes)) = Self::update(cx, |browsing, _| {
            let writes = browsing.favicon_writes.clone();
            if clearing {
                writes.generation.fetch_add(1, Ordering::SeqCst);
            }
            browsing
                .snapshot()
                .map(|(profile, history)| (profile, history, writes))
        }) else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                let _held = writes
                    .lock
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if let Err(error) = profile.prune_favicons(&history) {
                    trace(format_args!("removing page icons: {error}"));
                }
            })
            .detach();
    }

    fn snapshot(&self) -> Option<(Profile, Arc<History>)> {
        Some((self.profile.clone()?, self.history.clone()))
    }

    fn snapshot_for_write(&self) -> Option<(Profile, Arc<History>, ProfileWriteTicket)> {
        let profile = self.profile.clone()?;
        let ticket = profile.begin_history_write();
        Some((profile, self.history.clone(), ticket))
    }

    fn reserve_favicon_load(&mut self, url: &str) -> Option<u64> {
        if self.favicon_loads.contains_key(url) {
            return None;
        }
        self.next_favicon_load = self.next_favicon_load.wrapping_add(1);
        let request_id = self.next_favicon_load;
        self.favicon_loads.insert(url.to_owned(), request_id);
        Some(request_id)
    }

    fn complete_favicon_load(
        owner: Option<&mut Self>,
        url: &str,
        request_id: u64,
        favicon: Option<Favicon>,
    ) -> bool {
        let Some(owner) = owner else {
            return false;
        };
        if owner.favicon_loads.get(url) != Some(&request_id) {
            return false;
        }
        owner.favicon_loads.remove(url);
        owner.favicons.insert(url.to_owned(), favicon);
        true
    }
}

fn write_history(profile: &Profile, history: &History, ticket: ProfileWriteTicket) {
    if let Err(error) = profile.save_history_if_current(ticket, history) {
        eprintln!("failed to save history: {error}");
    }
}

/// The current time in Unix seconds.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::BrowsingHistory;
    use std::{cell::Cell, rc::Rc};

    #[gpui::test]
    fn cached_favicon_reads_do_not_invalidate_history_observers(cx: &mut gpui::TestAppContext) {
        let url = "https://example.com/";
        cx.update(|cx| {
            let mut history = BrowsingHistory::default();
            history.favicons.insert(url.to_owned(), None);
            cx.set_global(history);
        });
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let _subscription = cx.update(|cx| {
            cx.observe_global::<BrowsingHistory>(move |_| observed.set(observed.get() + 1))
        });

        for _ in 0..10 {
            cx.update(|cx| assert!(BrowsingHistory::favicon(url, cx).is_none()));
        }
        assert_eq!(
            notifications.get(),
            0,
            "rendering cached icons must stay idle"
        );
    }

    #[gpui::test]
    fn pending_favicon_reads_do_not_invalidate_history_observers(cx: &mut gpui::TestAppContext) {
        let url = "https://example.com/";
        cx.update(|cx| {
            let mut history = BrowsingHistory::default();
            history.reserve_favicon_load(url);
            cx.set_global(history);
        });
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let _subscription = cx.update(|cx| {
            cx.observe_global::<BrowsingHistory>(move |_| observed.set(observed.get() + 1))
        });

        for _ in 0..10 {
            cx.update(|cx| assert!(BrowsingHistory::favicon(url, cx).is_none()));
        }
        assert_eq!(
            notifications.get(),
            0,
            "in-flight icon reads must stay idle"
        );
    }

    #[gpui::test]
    fn most_visited_cache_follows_visits_titles_and_removals(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            BrowsingHistory::record_visit("https://a.example/", "A", cx);
            BrowsingHistory::record_visit("https://b.example/", "B", cx);
            assert_eq!(BrowsingHistory::most_visited(1, cx).len(), 1);
            assert_eq!(BrowsingHistory::most_visited(2, cx).len(), 2);
            BrowsingHistory::record_visit("https://b.example/", "B", cx);
            assert_eq!(BrowsingHistory::most_visited(1, cx)[0].title, "B");
            BrowsingHistory::set_title("https://b.example/", "Updated", cx);
            assert_eq!(BrowsingHistory::most_visited(1, cx)[0].title, "Updated");
            BrowsingHistory::remove_page("https://b.example/", cx);
            assert_eq!(BrowsingHistory::most_visited(2, cx)[0].title, "A");
            assert_eq!(BrowsingHistory::most_visited(2, cx).len(), 1);
            assert!(BrowsingHistory::most_visited(0, cx).is_empty());
        });
    }

    #[test]
    fn favicon_loads_coalesce_by_url() {
        let mut history = BrowsingHistory::default();

        let request = history
            .reserve_favicon_load("https://example.com/")
            .expect("reserves first load");

        assert_eq!(history.reserve_favicon_load("https://example.com/"), None);
        assert_eq!(history.favicon_loads.len(), 1);
        assert_eq!(
            history.favicon_loads.get("https://example.com/"),
            Some(&request)
        );
    }

    #[test]
    fn stale_or_closed_favicon_loads_are_ignored() {
        let mut history = BrowsingHistory::default();
        let url = "https://example.com/";
        let stale_request = history.reserve_favicon_load(url).expect("reserves load");
        history.favicon_loads.remove(url);
        let current_request = history
            .reserve_favicon_load(url)
            .expect("reserves replacement");

        assert!(!BrowsingHistory::complete_favicon_load(
            Some(&mut history),
            url,
            stale_request,
            None,
        ));
        assert!(history.favicon_loads.contains_key(url));
        assert!(!BrowsingHistory::complete_favicon_load(
            None,
            url,
            current_request,
            None,
        ));
        assert!(history.favicon_loads.contains_key(url));
    }

    #[test]
    fn favicon_load_completion_caches_missing_and_failed_results() {
        let mut history = BrowsingHistory::default();
        let url = "https://example.com/";
        let request = history.reserve_favicon_load(url).expect("reserves load");

        assert!(BrowsingHistory::complete_favicon_load(
            Some(&mut history),
            url,
            request,
            None,
        ));
        assert!(history.favicon_loads.is_empty());
        assert!(history.favicons.contains_key(url));
        assert!(matches!(history.favicons.get(url), Some(None)));
    }
}
