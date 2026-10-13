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
/// Page candidates retained before non-page rows can be deduplicated.
const MAX_PAGE_CANDIDATES: usize = MAX_ROWS + MAX_PAST_SEARCHES + 2;

/// A history page and its cached searchable text.
#[derive(Clone, Copy, Debug)]
pub struct IndexedVisitedPage<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub normalized_display_text: Option<&'a str>,
    pub normalized_title: Option<&'a str>,
    pub visit_count: u32,
    pub last_visit: u64,
}

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
    if input.trim().is_empty() {
        return Suggestions::default();
    }
    let fields: Vec<_> = pages
        .iter()
        .map(|page| {
            let normalized_display_text = normalized_display_address(page.url);
            let normalized_title = normalized_match_text(page.title);
            (normalized_display_text, normalized_title)
        })
        .collect();
    let indexed_pages = pages
        .iter()
        .zip(&fields)
        .map(|(page, fields)| IndexedVisitedPage {
            url: page.url,
            title: page.title,
            normalized_display_text: fields.0.as_deref(),
            normalized_title: fields.1.as_deref(),
            visit_count: page.visit_count,
            last_visit: page.last_visit,
        });
    suggest_with_indexed(input, engines, indexed_pages, searches.iter().copied())
}

/// Suggestions using searchable text normalized when each history page changed.
pub fn suggest_with_indexed<'a, 'b>(
    input: &str,
    engines: &SearchEngines,
    pages: impl IntoIterator<Item = IndexedVisitedPage<'a>>,
    searches: impl IntoIterator<Item = PastSearch<'b>>,
) -> Suggestions {
    let text = input.trim();
    let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return Suggestions::default();
    }

    let mut ranked_pages = Vec::with_capacity(MAX_PAGE_CANDIDATES);
    for page in pages {
        let Some(score) = page_score(&page, &terms) else {
            continue;
        };
        let candidate = RankedPage { page, score };
        let insertion = ranked_pages
            .iter()
            .position(|current| ranks_before(&candidate, current))
            .unwrap_or(ranked_pages.len());
        if insertion < MAX_PAGE_CANDIDATES {
            ranked_pages.insert(insertion, candidate);
            ranked_pages.truncate(MAX_PAGE_CANDIDATES);
        }
    }

    // A leading page completes the text when its address continues it.
    let completing = !input.ends_with(char::is_whitespace)
        && terms.len() == 1
        && ranked_pages.first().is_some_and(|candidate| {
            starts_with_ignoring_case(display_address_slice(candidate.page.url), text)
        });
    let completion = completing.then(|| {
        let address = display_address_slice(ranked_pages[0].page.url);
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

    let mut pages = ranked_pages
        .into_iter()
        .map(|candidate| page_suggestion(candidate.page, &terms));
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

#[derive(Clone, Copy)]
struct RankedPage<'a> {
    page: IndexedVisitedPage<'a>,
    score: u32,
}

fn ranks_before(a: &RankedPage<'_>, b: &RankedPage<'_>) -> bool {
    a.score > b.score || (a.score == b.score && a.page.last_visit > b.page.last_visit)
}

/// A visited page's score, when every term matches its title or address.
fn page_score(page: &IndexedVisitedPage<'_>, terms: &[String]) -> Option<u32> {
    let mut score = 0;
    for (index, term) in terms.iter().enumerate() {
        let in_text = word_start_match_normalized(page.normalized_display_text, term);
        let in_title = word_start_match_normalized(page.normalized_title, term);
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
    }
    score += page.visit_count.min(20) * 10;
    Some(score)
}

/// Builds rows only for the limited set of pages that can be shown.
fn page_suggestion(page: IndexedVisitedPage<'_>, terms: &[String]) -> Suggestion {
    let mut text_matches = Vec::new();
    let mut title_matches = Vec::new();
    for term in terms {
        text_matches.extend(word_start_match_normalized(
            page.normalized_display_text,
            term,
        ));
        title_matches.extend(word_start_match_normalized(page.normalized_title, term));
    }
    Suggestion {
        kind: SuggestionKind::Page {
            title: page.title.to_owned(),
            title_matches,
        },
        text: display_address_slice(page.url).to_owned(),
        text_matches,
        url: page.url.to_owned(),
    }
}

/// Past searches that start with the text, other than the text itself, most
/// used first.
fn past_searches<'a>(
    text: &str,
    searches: impl IntoIterator<Item = PastSearch<'a>>,
    engines: &SearchEngines,
) -> Vec<Suggestion> {
    let mut matching: [Option<PastSearch<'a>>; MAX_PAST_SEARCHES] = [None; MAX_PAST_SEARCHES];
    let mut matching_len = 0;
    for search in searches.into_iter().filter(|search| {
        starts_with_ignoring_case(search.query, text) && search.query.len() > text.len()
    }) {
        let insertion = matching
            .iter()
            .take(matching_len)
            .position(|current| {
                let current = current.expect("filled search slots contain a search");
                search.count > current.count
                    || (search.count == current.count && search.last_used > current.last_used)
            })
            .unwrap_or(matching_len);
        if insertion < MAX_PAST_SEARCHES {
            let shift_end = matching_len.min(MAX_PAST_SEARCHES - 1);
            for index in (insertion..shift_end).rev() {
                matching[index + 1] = matching[index];
            }
            matching[insertion] = Some(search);
            matching_len = (matching_len + 1).min(MAX_PAST_SEARCHES);
        }
    }
    matching
        .into_iter()
        .take(matching_len)
        .flatten()
        .map(|search| Suggestion {
            kind: SuggestionKind::PastSearch,
            text: search.query.to_owned(),
            text_matches: std::iter::once(0..text.len()).collect(),
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

/// Where each word of `query` starts a word of `text`, ignoring case, or
/// `None` when one does not. An empty query matches with no ranges.
pub fn match_words(text: &str, query: &str) -> Option<Vec<Range<usize>>> {
    query
        .split_whitespace()
        .map(|term| word_start_match(text, &term.to_lowercase()))
        .collect()
}

/// Where `term` starts a word of `haystack`, ignoring case: at its start or
/// after a character that is not a letter or digit.
fn word_start_match(haystack: &str, term: &str) -> Option<Range<usize>> {
    let lower = haystack.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; match only where it did not.
    if lower.len() != haystack.len() {
        return None;
    }
    word_start_match_lower(&lower, term)
}

fn word_start_match_normalized(lower: Option<&str>, term: &str) -> Option<Range<usize>> {
    word_start_match_lower(lower?, term)
}

fn word_start_match_lower(lower: &str, term: &str) -> Option<Range<usize>> {
    lower.match_indices(term).find_map(|(start, _)| {
        let at_word_start = lower[..start]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric());
        at_word_start.then(|| start..start + term.len())
    })
}

/// Lowercase text once for indexed matching, retaining only range-safe values.
pub fn normalized_match_text(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    (lower.len() == text.len()).then_some(lower)
}

/// Lowercase the displayed address once for indexed matching.
pub fn normalized_display_address(url: &str) -> Option<String> {
    normalized_match_text(display_address_slice(url))
}

fn starts_with_ignoring_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// An address as people write it: without `https://`, `www.` or a lone
/// trailing slash.
pub fn display_address(url: &str) -> String {
    display_address_slice(url).to_owned()
}

fn display_address_slice(url: &str) -> &str {
    let address = url.strip_prefix("https://").unwrap_or(url);
    let address = address.strip_prefix("www.").unwrap_or(address);
    address.strip_suffix('/').unwrap_or(address)
}

/// Just the site an address is on: its host without `www.`, and its port if
/// it names one. An address without a host, such as `about:blank`, is written
/// as [`display_address`] would.
pub fn display_domain(url: &str) -> String {
    let parsed = url::Url::parse(url).ok();
    let Some((parsed, host)) = parsed
        .as_ref()
        .and_then(|parsed| Some((parsed, parsed.host_str()?)))
        .filter(|(_, host)| !host.is_empty())
    else {
        return display_address(url);
    };
    let host = host.strip_prefix("www.").unwrap_or(host);
    match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
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
    fn keeps_the_best_page_matches_for_a_large_history() {
        let urls: Vec<_> = (0..100)
            .map(|index| format!("https://example.com/reference/{index}"))
            .collect();
        let pages: Vec<_> = urls
            .iter()
            .enumerate()
            .map(|(index, url)| VisitedPage {
                url,
                title: "Reference",
                visit_count: 1,
                last_visit: index as u64,
            })
            .collect();
        let suggestions = suggest("reference", &pages, &[]);
        let page_visits: Vec<_> = suggestions
            .rows
            .iter()
            .filter(|row| matches!(row.kind, SuggestionKind::Page { .. }))
            .map(|row| row.url.as_str())
            .collect();

        assert_eq!(page_visits.len(), 7);
        assert_eq!(page_visits[0], "https://example.com/reference/99");
        assert_eq!(page_visits[6], "https://example.com/reference/93");
    }

    #[test]
    fn writes_just_the_site_of_an_address() {
        assert_eq!(
            display_domain("https://www.example.com/docs?page=2#intro"),
            "example.com"
        );
        assert_eq!(display_domain("https://docs.rs/gpui"), "docs.rs");
        assert_eq!(
            display_domain("http://localhost:8080/admin"),
            "localhost:8080"
        );
        assert_eq!(display_domain("about:blank"), "about:blank");
        assert_eq!(
            display_domain("file:///Users/me/page.html"),
            "file:///Users/me/page.html"
        );
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

    #[test]
    fn matches_every_word_at_word_starts() {
        assert_eq!(
            match_words("Find in Page", "find pa"),
            Some(vec![0..4, 8..10])
        );
        assert_eq!(match_words("Find in Page", "age"), None);
        assert_eq!(match_words("Find in Page", "find tab"), None);
        assert_eq!(match_words("Anything", "  "), Some(Vec::new()));
    }
}
