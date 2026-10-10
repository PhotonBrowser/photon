//! The history's file format: versioned JSON.

use photon_core::{History, HistoryEntry, SearchEntry};
use serde::{Deserialize, Serialize};

/// Bumped when the format changes incompatibly; older files are discarded.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct HistoryFile {
    version: u32,
    pages: Vec<Page>,
    searches: Vec<Search>,
}

#[derive(Serialize, Deserialize)]
struct Page {
    url: String,
    title: String,
    visit_count: u32,
    last_visit: u64,
}

#[derive(Serialize, Deserialize)]
struct Search {
    query: String,
    count: u32,
    last_used: u64,
}

pub(crate) fn encode(history: &History) -> Vec<u8> {
    let file = HistoryFile {
        version: VERSION,
        pages: history
            .pages()
            .iter()
            .map(|entry| Page {
                url: entry.url.clone(),
                title: entry.title.clone(),
                visit_count: entry.visit_count,
                last_visit: entry.last_visit,
            })
            .collect(),
        searches: history
            .searches()
            .iter()
            .map(|entry| Search {
                query: entry.query.clone(),
                count: entry.count,
                last_used: entry.last_used,
            })
            .collect(),
    };
    serde_json::to_vec(&file).expect("history serializes")
}

/// The history in `bytes`, or `None` when they are not a history this
/// version of Photon can read.
pub(crate) fn decode(bytes: &[u8]) -> Option<History> {
    let file: HistoryFile = serde_json::from_slice(bytes).ok()?;
    if file.version != VERSION {
        return None;
    }
    let pages = file
        .pages
        .into_iter()
        .map(|page| HistoryEntry {
            url: page.url,
            title: page.title,
            visit_count: page.visit_count,
            last_visit: page.last_visit,
        })
        .collect();
    let searches = file
        .searches
        .into_iter()
        .map(|search| SearchEntry {
            query: search.query,
            count: search.count,
            last_used: search.last_used,
        })
        .collect();
    Some(History::from_parts(pages, searches))
}
