//! Pages visited and searches made, which the omnibox suggests and a new tab
//! page or settings can list.
//!
//! Times are Unix seconds supplied by the caller, so the rules stay
//! independent of clocks and storage. `photon-storage` saves a history and
//! loads it back with [`History::from_parts`].

use crate::omnibox::{
    IndexedVisitedPage, PastSearch, SearchEngines, Suggestions, normalized_display_address,
    normalized_match_text, suggest_with_indexed,
};

use super::browsing_data::ClearBrowsingData;

/// How many pages history and its profile file keep; the least recently
/// visited go first.
pub const HISTORY_MAX_PAGES: usize = 10_000;
/// How many searches history and its profile file keep.
pub const HISTORY_MAX_SEARCHES: usize = 1_000;

/// A page in the history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    /// When the page was last visited, in Unix seconds.
    pub last_visit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PageSearchCache {
    normalized_display_text: Option<String>,
    normalized_title: Option<String>,
}

/// A query searched for from the omnibox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchEntry {
    pub query: String,
    pub count: u32,
    /// When the query was last searched for, in Unix seconds.
    pub last_used: u64,
}

/// Visited web pages and past searches.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct History {
    pages: Vec<HistoryEntry>,
    searches: Vec<SearchEntry>,
    page_search_cache: Vec<PageSearchCache>,
}

impl History {
    /// A history holding previously saved entries.
    pub fn from_parts(pages: Vec<HistoryEntry>, searches: Vec<SearchEntry>) -> Self {
        let page_search_cache = pages.iter().map(cache_page_search_text).collect();
        Self {
            pages,
            searches,
            page_search_cache,
        }
    }

    pub fn pages(&self) -> &[HistoryEntry] {
        &self.pages
    }

    pub fn searches(&self) -> &[SearchEntry] {
        &self.searches
    }

    /// Records a visit to `url` at `now`. Only web pages are remembered.
    pub fn record_visit(&mut self, url: &str, title: &str, now: u64) {
        if !is_web_page(url) {
            return;
        }
        if let Some(index) = self.pages.iter().position(|entry| entry.url == url) {
            let entry = &mut self.pages[index];
            entry.visit_count += 1;
            entry.last_visit = now;
            if !title.is_empty() {
                title.clone_into(&mut entry.title);
                self.page_search_cache[index].normalized_title = normalized_match_text(title);
            }
            return;
        }

        let entry = HistoryEntry {
            url: url.to_owned(),
            title: title.to_owned(),
            visit_count: 1,
            last_visit: now,
        };
        self.page_search_cache.push(cache_page_search_text(&entry));
        self.pages.push(entry);
        if self.pages.len() > HISTORY_MAX_PAGES {
            trim_oldest(&mut self.pages, HISTORY_MAX_PAGES, |entry| entry.last_visit);
            self.rebuild_page_search_cache();
        }
    }

    /// Adds pages imported from another browser, returning how many were
    /// new. A page already here keeps the later visit and the larger visit
    /// count, and gains a title if it had none. Only web pages are kept, and
    /// the oldest are dropped past the limit.
    pub fn import_pages(&mut self, imported: impl IntoIterator<Item = HistoryEntry>) -> usize {
        let mut index: std::collections::HashMap<String, usize> = self
            .pages
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.url.clone(), position))
            .collect();
        let mut added = 0;
        for entry in imported {
            if !is_web_page(&entry.url) {
                continue;
            }
            match index.get(&entry.url) {
                Some(&position) => {
                    let existing = &mut self.pages[position];
                    existing.visit_count = existing.visit_count.max(entry.visit_count);
                    if entry.last_visit > existing.last_visit {
                        existing.last_visit = entry.last_visit;
                    }
                    if existing.title.is_empty() {
                        existing.title = entry.title;
                    }
                }
                None => {
                    index.insert(entry.url.clone(), self.pages.len());
                    self.pages.push(entry);
                    added += 1;
                }
            }
        }
        if self.pages.len() > HISTORY_MAX_PAGES {
            trim_oldest(&mut self.pages, HISTORY_MAX_PAGES, |entry| entry.last_visit);
        }
        self.rebuild_page_search_cache();
        added
    }

    /// Updates the title of a page already in the history, returning
    /// whether it changed.
    pub fn set_title(&mut self, url: &str, title: &str) -> bool {
        let Some(index) = self.pages.iter().position(|entry| entry.url == url) else {
            return false;
        };
        let entry = &mut self.pages[index];
        if entry.title == title {
            return false;
        }
        title.clone_into(&mut entry.title);
        self.page_search_cache[index].normalized_title = normalized_match_text(title);
        true
    }

    /// Records a search for `query` at `now`.
    pub fn record_search(&mut self, query: &str, now: u64) {
        let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
        if query.is_empty() {
            return;
        }
        match self.searches.iter_mut().find(|entry| entry.query == query) {
            Some(entry) => {
                entry.count += 1;
                entry.last_used = now;
            }
            None => {
                self.searches.push(SearchEntry {
                    query,
                    count: 1,
                    last_used: now,
                });
                trim_oldest(&mut self.searches, HISTORY_MAX_SEARCHES, |entry| {
                    entry.last_used
                });
            }
        }
    }

    /// Forgets the page at `url`.
    pub fn remove_page(&mut self, url: &str) {
        let mut pages = Vec::with_capacity(self.pages.len());
        let mut search_cache = Vec::with_capacity(self.page_search_cache.len());
        for (page, cached) in self.pages.drain(..).zip(self.page_search_cache.drain(..)) {
            if page.url != url {
                pages.push(page);
                search_cache.push(cached);
            }
        }
        self.pages = pages;
        self.page_search_cache = search_cache;
    }

    /// Forgets a past search.
    pub fn remove_search(&mut self, query: &str) {
        self.searches.retain(|entry| entry.query != query);
    }

    /// Forgets what `request` asks for: pages and searches last used at or
    /// after its start.
    pub fn clear(&mut self, request: &ClearBrowsingData) {
        if request.history {
            let mut pages = Vec::with_capacity(self.pages.len());
            let mut search_cache = Vec::with_capacity(self.page_search_cache.len());
            for (page, cached) in self.pages.drain(..).zip(self.page_search_cache.drain(..)) {
                if page.last_visit < request.since {
                    pages.push(page);
                    search_cache.push(cached);
                }
            }
            self.pages = pages;
            self.page_search_cache = search_cache;
        }
        if request.searches {
            self.searches
                .retain(|entry| entry.last_used < request.since);
        }
    }

    /// The pages visited most often, most visited first.
    pub fn most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        if limit == 0 {
            return Vec::new();
        }
        let mut pages: Vec<&HistoryEntry> = self.pages.iter().collect();
        let compare = |a: &&HistoryEntry, b: &&HistoryEntry| {
            b.visit_count
                .cmp(&a.visit_count)
                .then(b.last_visit.cmp(&a.last_visit))
        };
        if limit < pages.len() {
            pages.select_nth_unstable_by(limit, compare);
            pages.truncate(limit);
        }
        pages.sort_by(compare);
        pages
    }

    /// What the omnibox offers for `input` from this history.
    pub fn omnibox_suggestions(&self, input: &str) -> Suggestions {
        self.omnibox_suggestions_with(input, &SearchEngines::builtin())
    }

    /// What the omnibox offers for `input` using `engines` for search rows.
    pub fn omnibox_suggestions_with(&self, input: &str, engines: &SearchEngines) -> Suggestions {
        let pages = self
            .pages
            .iter()
            .zip(&self.page_search_cache)
            .map(|(entry, cached)| IndexedVisitedPage {
                url: &entry.url,
                title: &entry.title,
                normalized_display_text: cached.normalized_display_text.as_deref(),
                normalized_title: cached.normalized_title.as_deref(),
                visit_count: entry.visit_count,
                last_visit: entry.last_visit,
            });
        let searches = self.searches.iter().map(|entry| PastSearch {
            query: &entry.query,
            count: entry.count,
            last_used: entry.last_used,
        });
        suggest_with_indexed(input, engines, pages, searches)
    }

    fn rebuild_page_search_cache(&mut self) {
        self.page_search_cache = self.pages.iter().map(cache_page_search_text).collect();
    }
}

fn cache_page_search_text(page: &HistoryEntry) -> PageSearchCache {
    PageSearchCache {
        normalized_display_text: normalized_display_address(&page.url),
        normalized_title: normalized_match_text(&page.title),
    }
}

fn is_web_page(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Drops the least recently used entries beyond `limit`.
fn trim_oldest<T>(entries: &mut Vec<T>, limit: usize, last_used: impl Fn(&T) -> u64) {
    if entries.len() > limit {
        entries.sort_by_key(|entry| std::cmp::Reverse(last_used(entry)));
        entries.truncate(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_visits_and_keeps_the_latest_title() {
        let mut history = History::default();
        history.record_visit("https://example.com/", "", 10);
        history.set_title("https://example.com/", "Example");
        history.record_visit("https://example.com/", "", 20);
        let entry = &history.pages()[0];
        assert_eq!(entry.visit_count, 2);
        assert_eq!(entry.title, "Example");
        assert_eq!(entry.last_visit, 20);
    }

    #[test]
    fn most_visited_selects_the_requested_top_entries_in_order() {
        let history = History::from_parts(
            vec![
                HistoryEntry {
                    url: "https://older.example/".into(),
                    title: "Older".into(),
                    visit_count: 4,
                    last_visit: 10,
                },
                HistoryEntry {
                    url: "https://popular.example/".into(),
                    title: "Popular".into(),
                    visit_count: 8,
                    last_visit: 5,
                },
                HistoryEntry {
                    url: "https://recent.example/".into(),
                    title: "Recent".into(),
                    visit_count: 4,
                    last_visit: 20,
                },
            ],
            Vec::new(),
        );

        assert_eq!(
            history
                .most_visited(2)
                .iter()
                .map(|page| page.url.as_str())
                .collect::<Vec<_>>(),
            ["https://popular.example/", "https://recent.example/"]
        );
        assert_eq!(history.most_visited(0).len(), 0);
        assert_eq!(history.most_visited(10).len(), 3);
    }

    #[test]
    fn remembers_only_web_pages() {
        let mut history = History::default();
        history.record_visit("about:newtab", "New Tab", 1);
        history.record_visit("file:///tmp/page.html", "Page", 1);
        assert!(history.pages().is_empty());
    }

    #[test]
    fn suggests_visited_pages_and_past_searches() {
        let mut history = History::default();
        history.record_visit("https://discord.com/channels/me", "Discord", 1);
        history.record_search("discord  status", 2);
        let suggestions = history.omnibox_suggestions("disc");
        assert_eq!(
            suggestions.completion.as_deref(),
            Some("discord.com/channels/me")
        );
        assert!(
            suggestions
                .rows
                .iter()
                .any(|row| row.text == "discord status")
        );
        history.remove_page("https://discord.com/channels/me");
        assert_eq!(history.omnibox_suggestions("disc").completion, None);
    }

    #[test]
    fn keeps_cached_suggestion_text_in_sync_with_history_changes() {
        let mut history = History::default();
        history.record_visit("https://first.example/", "First", 1);
        history.record_visit("https://second.example/", "Second", 2);
        assert!(history.set_title("https://first.example/", "Updated"));

        let suggestions = history.omnibox_suggestions("updated");
        assert!(
            suggestions
                .rows
                .iter()
                .any(|row| row.url == "https://first.example/")
        );

        history.remove_page("https://first.example/");
        let suggestions = history.omnibox_suggestions("second");
        assert!(
            suggestions
                .rows
                .iter()
                .any(|row| row.url == "https://second.example/")
        );
        assert!(
            suggestions
                .rows
                .iter()
                .all(|row| row.url != "https://first.example/")
        );
    }

    #[test]
    fn suggests_one_and_two_character_inputs_from_a_full_history() {
        let pages = (0..HISTORY_MAX_PAGES)
            .map(|index| HistoryEntry {
                url: format!("https://site{index}.example/home"),
                title: "Example page".to_owned(),
                visit_count: 1,
                last_visit: index as u64,
            })
            .collect();
        let history = History::from_parts(pages, Vec::new());

        for query in ["e", "ex"] {
            let suggestions = history.omnibox_suggestions(query);
            assert_eq!(suggestions.rows.len(), 8);
            assert_eq!(suggestions.rows[1].url, "https://site9999.example/home");
        }
    }

    #[test]
    fn clears_what_was_used_since_a_time() {
        let mut history = History::default();
        history.record_visit("https://old.example/", "Old", 10);
        history.record_visit("https://new.example/", "New", 30);
        history.record_search("old query", 10);
        history.record_search("new query", 30);
        history.clear(&ClearBrowsingData {
            since: 20,
            history: true,
            ..ClearBrowsingData::default()
        });
        assert_eq!(history.pages().len(), 1);
        assert_eq!(history.pages()[0].url, "https://old.example/");
        assert_eq!(history.searches().len(), 2);
        history.clear(&ClearBrowsingData::everything());
        assert!(history.pages().is_empty() && history.searches().is_empty());
    }

    #[test]
    fn imports_pages_and_merges_ones_already_known() {
        let mut history = History::default();
        history.record_visit("https://known.example/", "", 50);
        let imported = [
            HistoryEntry {
                url: "https://known.example/".into(),
                title: "Known".into(),
                visit_count: 7,
                last_visit: 40,
            },
            HistoryEntry {
                url: "https://new.example/".into(),
                title: "New".into(),
                visit_count: 2,
                last_visit: 30,
            },
            HistoryEntry {
                url: "chrome://settings".into(),
                title: "Settings".into(),
                visit_count: 1,
                last_visit: 30,
            },
        ];
        assert_eq!(history.import_pages(imported), 1);
        assert_eq!(history.pages().len(), 2);
        let known = &history.pages()[0];
        assert_eq!(
            (known.title.as_str(), known.visit_count, known.last_visit),
            ("Known", 7, 50)
        );
        assert!(
            history
                .omnibox_suggestions("new")
                .rows
                .iter()
                .any(|row| row.url == "https://new.example/")
        );
    }

    #[test]
    fn lists_the_most_visited_pages() {
        let mut history = History::default();
        history.record_visit("https://a.example/", "A", 1);
        for now in 2..5 {
            history.record_visit("https://b.example/", "B", now);
        }
        let most = history.most_visited(1);
        assert_eq!(most.len(), 1);
        assert_eq!(most[0].url, "https://b.example/");
    }
}
