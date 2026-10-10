//! Pages visited and searches made, which the omnibox suggests and a new tab
//! page or settings can list.
//!
//! Times are Unix seconds supplied by the caller, so the rules stay
//! independent of clocks and storage. `photon-storage` saves a history and
//! loads it back with [`History::from_parts`].

use photon_omnibox::{PastSearch, Suggestions, VisitedPage, suggest};

use super::browsing_data::ClearBrowsingData;

/// How many pages the history keeps; the least recently visited go first.
const MAX_PAGES: usize = 10_000;
/// How many searches the history keeps.
const MAX_SEARCHES: usize = 1_000;

/// A page in the history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    /// When the page was last visited, in Unix seconds.
    pub last_visit: u64,
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
}

impl History {
    /// A history holding previously saved entries.
    pub fn from_parts(pages: Vec<HistoryEntry>, searches: Vec<SearchEntry>) -> Self {
        Self { pages, searches }
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
        match self.pages.iter_mut().find(|entry| entry.url == url) {
            Some(entry) => {
                entry.visit_count += 1;
                entry.last_visit = now;
                if !title.is_empty() {
                    title.clone_into(&mut entry.title);
                }
            }
            None => {
                self.pages.push(HistoryEntry {
                    url: url.to_owned(),
                    title: title.to_owned(),
                    visit_count: 1,
                    last_visit: now,
                });
                trim_oldest(&mut self.pages, MAX_PAGES, |entry| entry.last_visit);
            }
        }
    }

    /// Updates the title of a page already in the history, returning
    /// whether it changed.
    pub fn set_title(&mut self, url: &str, title: &str) -> bool {
        match self.pages.iter_mut().find(|entry| entry.url == url) {
            Some(entry) if entry.title != title => {
                title.clone_into(&mut entry.title);
                true
            }
            _ => false,
        }
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
                trim_oldest(&mut self.searches, MAX_SEARCHES, |entry| entry.last_used);
            }
        }
    }

    /// Forgets the page at `url`.
    pub fn remove_page(&mut self, url: &str) {
        self.pages.retain(|entry| entry.url != url);
    }

    /// Forgets a past search.
    pub fn remove_search(&mut self, query: &str) {
        self.searches.retain(|entry| entry.query != query);
    }

    /// Forgets what `request` asks for: pages and searches last used at or
    /// after its start.
    pub fn clear(&mut self, request: &ClearBrowsingData) {
        if request.history {
            self.pages.retain(|entry| entry.last_visit < request.since);
        }
        if request.searches {
            self.searches
                .retain(|entry| entry.last_used < request.since);
        }
    }

    /// The pages visited most often, most visited first.
    pub fn most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut pages: Vec<&HistoryEntry> = self.pages.iter().collect();
        pages.sort_by(|a, b| {
            b.visit_count
                .cmp(&a.visit_count)
                .then(b.last_visit.cmp(&a.last_visit))
        });
        pages.truncate(limit);
        pages
    }

    /// What the omnibox offers for `input` from this history.
    pub fn omnibox_suggestions(&self, input: &str) -> Suggestions {
        let pages: Vec<VisitedPage> = self
            .pages
            .iter()
            .map(|entry| VisitedPage {
                url: &entry.url,
                title: &entry.title,
                visit_count: entry.visit_count,
                last_visit: entry.last_visit,
            })
            .collect();
        let searches: Vec<PastSearch> = self
            .searches
            .iter()
            .map(|entry| PastSearch {
                query: &entry.query,
                count: entry.count,
                last_used: entry.last_used,
            })
            .collect();
        suggest(input, &pages, &searches)
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
