//! The history's file format: versioned JSON with bounded fields and lists.

use photon_core::{HISTORY_MAX_PAGES, HISTORY_MAX_SEARCHES, History, HistoryEntry, SearchEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;

/// Maximum encoded history size read from or written to a profile.
pub(crate) const MAX_HISTORY_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_URL_BYTES: usize = 4 * 1024;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_QUERY_BYTES: usize = 2 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_COLLECTION_ITEMS: usize = HISTORY_MAX_PAGES;
const MAX_STRING_BYTES: usize = 8 * 1024;

/// Bumped when the format changes incompatibly; older files are rejected.
const VERSION: u32 = 1;

#[derive(Serialize)]
struct HistoryFile<'a> {
    version: u32,
    pages: Vec<Page<'a>>,
    searches: Vec<Search<'a>>,
}

#[derive(Serialize)]
struct Page<'a> {
    url: &'a str,
    title: &'a str,
    visit_count: u32,
    last_visit: u64,
}

#[derive(Serialize)]
struct Search<'a> {
    query: &'a str,
    count: u32,
    last_used: u64,
}

#[derive(Deserialize)]
struct PageRead {
    url: String,
    title: String,
    visit_count: u32,
    last_visit: u64,
}

#[derive(Deserialize)]
struct SearchRead {
    query: String,
    count: u32,
    last_used: u64,
}

pub(crate) fn encode(history: &History) -> io::Result<Vec<u8>> {
    if history.pages().len() > HISTORY_MAX_PAGES || history.searches().len() > HISTORY_MAX_SEARCHES
    {
        return Err(invalid_data("history exceeds its item limit"));
    }
    if history
        .pages()
        .iter()
        .any(|page| page.url.len() > MAX_URL_BYTES || page.title.len() > MAX_TITLE_BYTES)
        || history
            .searches()
            .iter()
            .any(|search| search.query.len() > MAX_QUERY_BYTES)
    {
        return Err(invalid_data(
            "history contains a string that exceeds its size limit",
        ));
    }

    let file = HistoryFile {
        version: VERSION,
        pages: history
            .pages()
            .iter()
            .map(|entry| Page {
                url: &entry.url,
                title: &entry.title,
                visit_count: entry.visit_count,
                last_visit: entry.last_visit,
            })
            .collect(),
        searches: history
            .searches()
            .iter()
            .map(|entry| Search {
                query: &entry.query,
                count: entry.count,
                last_used: entry.last_used,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if bytes.len() > MAX_HISTORY_FILE_BYTES {
        return Err(invalid_data("history file exceeds its size limit"));
    }
    Ok(bytes)
}

/// The history in `bytes`, recovering intact rows when individual entries are
/// damaged while retaining the source file for later recovery.
pub(crate) fn decode(bytes: &[u8]) -> super::ProfileLoadOutcome<History> {
    if bytes.len() > MAX_HISTORY_FILE_BYTES {
        return super::ProfileLoadOutcome::Corrupt;
    }
    let document: Value = match serde_json::from_slice(bytes) {
        Ok(document) => document,
        Err(_) => return super::ProfileLoadOutcome::Corrupt,
    };
    if validate_json_tree(&document, 0).is_err() {
        return super::ProfileLoadOutcome::Corrupt;
    }
    let Value::Object(fields) = &document else {
        return super::ProfileLoadOutcome::Corrupt;
    };
    let Some(version) = fields.get("version").and_then(Value::as_u64) else {
        return super::ProfileLoadOutcome::Corrupt;
    };
    if version != u64::from(VERSION) {
        return super::ProfileLoadOutcome::Unsupported;
    }
    let Some(Value::Array(page_values)) = fields.get("pages") else {
        return super::ProfileLoadOutcome::Corrupt;
    };
    let Some(Value::Array(search_values)) = fields.get("searches") else {
        return super::ProfileLoadOutcome::Corrupt;
    };
    if page_values.len() > HISTORY_MAX_PAGES || search_values.len() > HISTORY_MAX_SEARCHES {
        return super::ProfileLoadOutcome::Corrupt;
    }

    let mut partially_recovered = false;
    let mut pages = Vec::with_capacity(page_values.len());
    for value in page_values {
        let Ok(page) = serde_json::from_value::<PageRead>(value.clone()) else {
            partially_recovered = true;
            continue;
        };
        if page.url.len() > MAX_URL_BYTES || page.title.len() > MAX_TITLE_BYTES {
            partially_recovered = true;
            continue;
        }
        pages.push(HistoryEntry {
            url: page.url,
            title: page.title,
            visit_count: page.visit_count,
            last_visit: page.last_visit,
        });
    }

    let mut searches = Vec::with_capacity(search_values.len());
    for value in search_values {
        let Ok(search) = serde_json::from_value::<SearchRead>(value.clone()) else {
            partially_recovered = true;
            continue;
        };
        if search.query.len() > MAX_QUERY_BYTES {
            partially_recovered = true;
            continue;
        }
        searches.push(SearchEntry {
            query: search.query,
            count: search.count,
            last_used: search.last_used,
        });
    }

    let history = History::from_parts(pages, searches);
    if partially_recovered {
        super::ProfileLoadOutcome::PartiallyRecovered(history)
    } else {
        super::ProfileLoadOutcome::Loaded(history)
    }
}

fn validate_json_tree(value: &Value, depth: usize) -> Result<(), ()> {
    if depth > MAX_DEPTH {
        return Err(());
    }
    match value {
        Value::String(string) if string.len() > MAX_STRING_BYTES => Err(()),
        Value::Array(items) => {
            if items.len() > MAX_COLLECTION_ITEMS {
                return Err(());
            }
            items
                .iter()
                .try_for_each(|item| validate_json_tree(item, depth + 1))
        }
        Value::Object(fields) => {
            if fields.len() > MAX_COLLECTION_ITEMS {
                return Err(());
            }
            for (key, value) in fields {
                if key.len() > 256 {
                    return Err(());
                }
                validate_json_tree(value, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProfileLoadOutcome;

    fn page_count(count: usize) -> Vec<u8> {
        let pages = (0..count)
            .map(|_| r#"{"url":"https://example.com/","title":"Example","visit_count":1,"last_visit":1}"#)
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"version":{VERSION},"pages":[{pages}],"searches":[]}}"#).into_bytes()
    }

    fn search_count(count: usize) -> Vec<u8> {
        let searches = (0..count)
            .map(|_| r#"{"query":"photon","count":1,"last_used":1}"#)
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"version":{VERSION},"pages":[],"searches":[{searches}]}}"#).into_bytes()
    }

    fn string_fields(url: &str, title: &str, query: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": VERSION,
            "pages": [{
                "url": url,
                "title": title,
                "visit_count": 1,
                "last_visit": 1
            }],
            "searches": [{"query": query, "count": 1, "last_used": 1}]
        }))
        .unwrap()
    }

    #[test]
    fn history_page_count_is_bounded_at_decode_and_encode() {
        assert!(matches!(
            decode(&page_count(HISTORY_MAX_PAGES)),
            ProfileLoadOutcome::Loaded(_)
        ));
        assert!(matches!(
            decode(&page_count(HISTORY_MAX_PAGES + 1)),
            ProfileLoadOutcome::Corrupt
        ));

        let pages = (0..=HISTORY_MAX_PAGES)
            .map(|index| HistoryEntry {
                url: format!("https://{index}.example/"),
                title: String::new(),
                visit_count: 1,
                last_visit: index as u64,
            })
            .collect();
        let history = History::from_parts(pages, Vec::new());
        assert!(encode(&history).is_err());
    }

    #[test]
    fn history_search_count_is_bounded_at_decode_and_encode() {
        assert!(matches!(
            decode(&search_count(HISTORY_MAX_SEARCHES)),
            ProfileLoadOutcome::Loaded(_)
        ));
        assert!(matches!(
            decode(&search_count(HISTORY_MAX_SEARCHES + 1)),
            ProfileLoadOutcome::Corrupt
        ));

        let searches = (0..=HISTORY_MAX_SEARCHES)
            .map(|index| SearchEntry {
                query: format!("query {index}"),
                count: 1,
                last_used: index as u64,
            })
            .collect();
        let history = History::from_parts(Vec::new(), searches);
        assert!(encode(&history).is_err());
    }

    #[test]
    fn history_strings_are_bounded_at_decode_and_encode() {
        assert!(matches!(
            decode(&string_fields(
                &"u".repeat(MAX_URL_BYTES),
                &"t".repeat(MAX_TITLE_BYTES),
                &"q".repeat(MAX_QUERY_BYTES),
            )),
            ProfileLoadOutcome::Loaded(_)
        ));
        assert!(matches!(
            decode(&string_fields(
                &"u".repeat(MAX_URL_BYTES + 1),
                "title",
                "query",
            )),
            ProfileLoadOutcome::PartiallyRecovered(_)
        ));
        assert!(matches!(
            decode(&string_fields(
                "url",
                &"t".repeat(MAX_TITLE_BYTES + 1),
                "query",
            )),
            ProfileLoadOutcome::PartiallyRecovered(_)
        ));
        assert!(matches!(
            decode(&string_fields(
                "url",
                "title",
                &"q".repeat(MAX_QUERY_BYTES + 1),
            )),
            ProfileLoadOutcome::PartiallyRecovered(_)
        ));

        let history = History::from_parts(
            vec![HistoryEntry {
                url: "u".repeat(MAX_URL_BYTES + 1),
                title: String::new(),
                visit_count: 1,
                last_visit: 1,
            }],
            Vec::new(),
        );
        assert!(encode(&history).is_err());
    }

    #[test]
    fn rejects_oversized_files_and_malformed_nested_json() {
        assert!(matches!(
            decode(&vec![b' '; MAX_HISTORY_FILE_BYTES + 1]),
            ProfileLoadOutcome::Corrupt
        ));
        let nested = format!(
            "{{\"unknown\":{}}}",
            "[".repeat(130) + "null" + &"]".repeat(130)
        );
        assert!(matches!(
            decode(nested.as_bytes()),
            ProfileLoadOutcome::Corrupt
        ));
    }

    #[test]
    fn reports_unsupported_versions_and_recovers_valid_rows() {
        let unsupported = br#"{"version":99,"pages":[],"searches":[]}"#;
        assert_eq!(decode(unsupported), ProfileLoadOutcome::Unsupported);

        let damaged = br#"{"version":1,"pages":[{"url":"https://ok.example/","title":"OK","visit_count":1,"last_visit":1},{"url":7}],"searches":[]}"#;
        let ProfileLoadOutcome::PartiallyRecovered(history) = decode(damaged) else {
            panic!("recovers the valid row");
        };
        assert_eq!(history.pages().len(), 1);
    }
}
