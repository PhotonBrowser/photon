//! Suggestions shown in the omnibox while typing: pages visited before, the
//! typed address, a search, and searches made before, with the first being
//! what Enter opens.

use std::ops::Range;

use super::engines::SearchEngines;
use super::resolve::{BUILTIN_ENGINES, OmniboxTarget, resolve_with, search};

/// How many rows the omnibox shows at most.
const MAX_ROWS: usize = 8;
/// How many visited pages rank above the search row.
const PAGES_BEFORE_SEARCH: usize = 3;

/// A page visited before, which the omnibox can suggest.
#[derive(Clone, Copy, Debug)]
pub struct VisitedPage<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub visit_count: u32,
    /// Larger is more recent.
    pub last_visit: u64,
}

/// A search made before, which the omnibox can suggest again.
#[derive(Clone, Copy, Debug)]
pub struct PastSearch<'a> {
    pub query: &'a str,
    pub count: u32,
    /// Larger is more recent.
    pub last_used: u64,
}

/// How many past searches the omnibox shows at most.
const MAX_PAST_SEARCHES: usize = 3;

/// What choosing a suggestion does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SuggestionKind {
    /// Return to a page visited before.
    Page {
        title: String,
        /// The typed terms' matches in `title`, as byte ranges.
        title_matches: Vec<Range<usize>>,
    },
    /// Open the typed address.
    Address,
    /// Search with the named engine.
    Search { engine: String },
    /// Search again for a query searched for before.
    PastSearch,
}

impl SuggestionKind {
    /// Whether the row comes from the history, so it can be forgotten.
    pub fn is_remembered(&self) -> bool {
        matches!(self, Self::Page { .. } | Self::PastSearch)
    }
}

/// A row in the omnibox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    /// The address as people write it, or the query.
    pub text: String,
    /// The typed terms' matches in `text`, as byte ranges.
    pub text_matches: Vec<Range<usize>>,
    /// The address to load.
    pub url: String,
}

/// What the omnibox offers for typed text.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Suggestions {
    /// The rows, the first being what Enter opens.
    pub rows: Vec<Suggestion>,
    /// The first row's address, which starts with the typed text, for the
    /// omnibox to complete inline.
    pub completion: Option<String>,
}

/// Suggestions for typed text with Photon's built-in engines.
pub fn suggest(input: &str, pages: &[VisitedPage], searches: &[PastSearch]) -> Suggestions {
    suggest_with(
        input,
        BUILTIN_ENGINES.get_or_init(SearchEngines::builtin),
        pages,
        searches,
    )
}

/// Suggestions for typed text with a specific engine registry.
///
/// Visited pages match when every typed word starts a word in their title or
/// address. A page whose address starts with the text leads and completes the
/// text inline. Otherwise the typed address leads when the text is one, and a
/// search for it when not. The search row stays among the first few rows so a
/// word that looks like a host can still be searched for. Past searches that
/// continue the text follow it.
pub fn suggest_with(
    input: &str,
    engines: &SearchEngines,
    pages: &[VisitedPage],
    searches: &[PastSearch],
) -> Suggestions {
    let text = input.trim();
    let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return Suggestions::default();
    }

    let mut matches: Vec<(u32, &VisitedPage, Suggestion)> = pages
        .iter()
        .filter_map(|page| page_suggestion(page, &terms).map(|(score, row)| (score, page, row)))
        .collect();
    matches.sort_by(|(a_score, a, _), (b_score, b, _)| {
        b_score.cmp(a_score).then(b.last_visit.cmp(&a.last_visit))
    });

    // A leading page completes the text when its address continues it.
    let completing = !input.ends_with(char::is_whitespace)
        && terms.len() == 1
        && matches
            .first()
            .is_some_and(|(_, _, row)| starts_with_ignoring_case(&row.text, text));
    let completion = completing.then(|| {
        let address = &matches[0].2.text;
        format!("{text}{}", &address[text.len()..])
    });

    let search_row = search_suggestion(text, engines);
    let typed = match resolve_with(text, engines) {
        Ok(OmniboxTarget::Url { url, .. }) => Some(Suggestion {
            kind: SuggestionKind::Address,
            text: display_address(&url),
            text_matches: Vec::new(),
            url,
        }),
        Ok(OmniboxTarget::Search { .. }) => None,
        Err(_) => return Suggestions::default(),
    };

    let mut pages = matches.into_iter().map(|(_, _, row)| row);
    let mut rows = Vec::with_capacity(MAX_ROWS);
    if completing {
        rows.extend(pages.next());
    } else if let Some(typed) = typed {
        rows.push(typed);
    }
    let leading_search = rows.is_empty();
    if leading_search {
        rows.extend(search_row.clone());
    }
    rows.extend(pages.by_ref().take(PAGES_BEFORE_SEARCH));
    if !leading_search {
        rows.extend(search_row);
    }
    rows.extend(past_searches(text, searches, engines));
    rows.extend(pages);
    rows.dedup_by(|a, b| a.url == b.url);
    rows.truncate(MAX_ROWS);
    Suggestions { rows, completion }
}

/// A visited page's row and how well it matches, when every term matches.
fn page_suggestion(page: &VisitedPage, terms: &[String]) -> Option<(u32, Suggestion)> {
    let text = display_address(page.url);
    let mut score = 0;
    let mut text_matches = Vec::new();
    let mut title_matches = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let in_text = word_start_match(&text, term);
        let in_title = word_start_match(page.title, term);
        if in_text.is_none() && in_title.is_none() {
            return None;
        }
        // Each term counts once, by its best match; the text continuing the
        // address ranks highest.
        let text_score = in_text.as_ref().map_or(0, |range| {
            if index == 0 && range.start == 0 {
                1000
            } else {
                400
            }
        });
        let title_score = if in_title.is_some() { 300 } else { 0 };
        score += text_score.max(title_score);
        text_matches.extend(in_text);
        title_matches.extend(in_title);
    }
    score += page.visit_count.min(20) * 10;
    Some((
        score,
        Suggestion {
            kind: SuggestionKind::Page {
                title: page.title.to_owned(),
                title_matches,
            },
            text,
            text_matches,
            url: page.url.to_owned(),
        },
    ))
}

/// Past searches that start with the text, other than the text itself, most
/// used first.
fn past_searches(text: &str, searches: &[PastSearch], engines: &SearchEngines) -> Vec<Suggestion> {
    let mut matching: Vec<&PastSearch> = searches
        .iter()
        .filter(|search| {
            starts_with_ignoring_case(search.query, text) && search.query.len() > text.len()
        })
        .collect();
    matching.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used.cmp(&a.last_used)));
    matching
        .into_iter()
        .take(MAX_PAST_SEARCHES)
        .map(|search| Suggestion {
            kind: SuggestionKind::PastSearch,
            text: search.query.to_owned(),
            text_matches: vec![0..text.len()],
            url: engines.default_engine().search_url(search.query),
        })
        .collect()
}

fn search_suggestion(text: &str, engines: &SearchEngines) -> Option<Suggestion> {
    match search(text, engines) {
        OmniboxTarget::Search { url, query, engine } => Some(Suggestion {
            kind: SuggestionKind::Search {
                engine: engine.name.into_owned(),
            },
            text_matches: Vec::new(),
            text: query,
            url,
        }),
        OmniboxTarget::Url { .. } => None,
    }
}

/// Where `term` starts a word of `haystack`, ignoring case: at its start or
/// after a character that is not a letter or digit.
fn word_start_match(haystack: &str, term: &str) -> Option<Range<usize>> {
    let lower = haystack.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; match only where it did not.
    if lower.len() != haystack.len() {
        return None;
    }
    lower.match_indices(term).find_map(|(start, _)| {
        let at_word_start = lower[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric());
        at_word_start.then(|| start..start + term.len())
    })
}

fn starts_with_ignoring_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// An address as people write it: without `https://`, `www.` or a lone
/// trailing slash.
pub fn display_address(url: &str) -> String {
    let address = url.strip_prefix("https://").unwrap_or(url);
    let address = address.strip_prefix("www.").unwrap_or(address);
    address.strip_suffix('/').unwrap_or(address).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGES: [VisitedPage; 3] = [
        VisitedPage {
            url: "https://discord.com/channels/me",
            title: "Discord",
            visit_count: 12,
            last_visit: 3,
        },
        VisitedPage {
            url: "https://drive.google.com/",
            title: "Google Drive",
            visit_count: 2,
            last_visit: 2,
        },
        VisitedPage {
            url: "https://www.rust-lang.org/learn",
            title: "Learn Rust",
            visit_count: 1,
            last_visit: 1,
        },
    ];

    fn kinds(suggestions: &Suggestions) -> Vec<&str> {
        suggestions
            .rows
            .iter()
            .map(|row| match &row.kind {
                SuggestionKind::Page { .. } => "page",
                SuggestionKind::Address => "address",
                SuggestionKind::Search { .. } => "search",
                SuggestionKind::PastSearch => "past search",
            })
            .collect()
    }

    #[test]
    fn offers_nothing_for_empty_or_unopenable_text() {
        assert!(suggest("", &PAGES, &[]).rows.is_empty());
        assert!(suggest("   ", &PAGES, &[]).rows.is_empty());
        assert!(suggest("https://", &PAGES, &[]).rows.is_empty());
    }

    #[test]
    fn completes_a_visited_address_inline() {
        let suggestions = suggest("d", &PAGES, &[]);
        assert_eq!(
            suggestions.completion.as_deref(),
            Some("discord.com/channels/me")
        );
        assert_eq!(suggestions.rows[0].url, "https://discord.com/channels/me");
        assert_eq!(suggestions.rows[0].text_matches, vec![0..1]);
        // Drive's title has a word starting with "d", so it follows.
        assert_eq!(kinds(&suggestions), ["page", "page", "search"]);
        assert_eq!(suggestions.rows[2].text, "d");
    }

    #[test]
    fn keeps_the_typed_case_in_the_completion() {
        let suggestions = suggest("Disc", &PAGES, &[]);
        assert_eq!(
            suggestions.completion.as_deref(),
            Some("Discord.com/channels/me")
        );
    }

    #[test]
    fn matches_words_in_titles_and_addresses() {
        let suggestions = suggest("learn rust", &PAGES, &[]);
        assert_eq!(suggestions.completion, None);
        assert_eq!(kinds(&suggestions), ["search", "page"]);
        let SuggestionKind::Page { title_matches, .. } = &suggestions.rows[1].kind else {
            panic!("expected a page");
        };
        assert_eq!(title_matches, &vec![0..5, 6..10]);
    }

    #[test]
    fn does_not_match_inside_words() {
        assert_eq!(kinds(&suggest("oogle", &PAGES, &[])), ["search"]);
    }

    #[test]
    fn leads_with_a_typed_address_nobody_visited() {
        let suggestions = suggest("example.com", &PAGES, &[]);
        assert_eq!(kinds(&suggestions), ["address", "search"]);
        assert_eq!(suggestions.rows[0].url, "https://example.com/");
        assert_eq!(suggestions.rows[0].text, "example.com");
    }

    #[test]
    fn searches_for_a_query() {
        let suggestions = suggest("rust  borrow checker", &[], &[]);
        assert_eq!(kinds(&suggestions), ["search"]);
        assert_eq!(suggestions.rows[0].text, "rust borrow checker");
        assert_eq!(
            suggestions.rows[0].kind,
            SuggestionKind::Search {
                engine: "Google".to_owned()
            }
        );
    }

    #[test]
    fn suggests_past_searches_that_continue_the_text() {
        let searches = [
            PastSearch {
                query: "down arrow unicode",
                count: 1,
                last_used: 4,
            },
            PastSearch {
                query: "dog names",
                count: 3,
                last_used: 2,
            },
            PastSearch {
                query: "cats",
                count: 9,
                last_used: 5,
            },
        ];
        let suggestions = suggest("do", &[], &searches);
        assert_eq!(
            kinds(&suggestions),
            ["search", "past search", "past search"]
        );
        assert_eq!(suggestions.rows[1].text, "dog names");
        assert_eq!(suggestions.rows[1].text_matches, vec![0..2]);
        assert!(suggestions.rows[1].url.contains("dog"));
    }

    #[test]
    fn writes_addresses_the_short_way() {
        assert_eq!(display_address("https://www.example.com/"), "example.com");
        assert_eq!(
            display_address("https://example.com/docs"),
            "example.com/docs"
        );
        assert_eq!(
            display_address("http://localhost:8080/"),
            "http://localhost:8080"
        );
    }
}
