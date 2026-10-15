//! The history's file format: versioned JSON with bounded fields and lists.

use photon_core::{HISTORY_MAX_PAGES, HISTORY_MAX_SEARCHES, History, HistoryEntry, SearchEntry};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::io;

/// Maximum encoded history size read from or written to a profile.
pub(crate) const MAX_HISTORY_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_URL_BYTES: usize = 4 * 1024;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_QUERY_BYTES: usize = 2 * 1024;

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
struct HistoryFileRead {
    version: u32,
    pages: BoundedVec<PageRead, HISTORY_MAX_PAGES>,
    searches: BoundedVec<SearchRead, HISTORY_MAX_SEARCHES>,
}

#[derive(Deserialize)]
struct PageRead {
    url: BoundedString<MAX_URL_BYTES>,
    title: BoundedString<MAX_TITLE_BYTES>,
    visit_count: u32,
    last_visit: u64,
}

#[derive(Deserialize)]
struct SearchRead {
    query: BoundedString<MAX_QUERY_BYTES>,
    count: u32,
    last_used: u64,
}

struct BoundedString<const MAX: usize>(String);

impl<'de, const MAX: usize> Deserialize<'de> for BoundedString<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringVisitor<const MAX: usize>;

        impl<const MAX: usize> Visitor<'_> for StringVisitor<MAX> {
            type Value = BoundedString<MAX>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a string no longer than {MAX} bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAX {
                    return Err(E::custom("profile string exceeds its size limit"));
                }
                Ok(BoundedString(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() > MAX {
                    return Err(E::custom("profile string exceeds its size limit"));
                }
                Ok(BoundedString(value))
            }
        }

        deserializer.deserialize_string(StringVisitor::<MAX>)
    }
}

struct BoundedVec<T, const MAX: usize>(Vec<T>);

impl<T, const MAX: usize> Default for BoundedVec<T, MAX> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'de, T, const MAX: usize> Deserialize<'de> for BoundedVec<T, MAX>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);

        impl<'de, T, const MAX: usize> Visitor<'de> for VecVisitor<T, MAX>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAX>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a list with at most {MAX} items")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAX));
                while let Some(value) = sequence.next_element()? {
                    if values.len() == MAX {
                        return Err(de::Error::custom("profile list exceeds its item limit"));
                    }
                    values.push(value);
                }
                Ok(BoundedVec(values))
            }
        }

        deserializer.deserialize_seq(VecVisitor::<T, MAX>(std::marker::PhantomData))
    }
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

/// The history in `bytes`, or `None` when it exceeds a limit or cannot be
/// read by this version of Photon.
pub(crate) fn decode(bytes: &[u8]) -> Option<History> {
    if bytes.len() > MAX_HISTORY_FILE_BYTES {
        return None;
    }
    let file: HistoryFileRead = serde_json::from_slice(bytes).ok()?;
    if file.version != VERSION {
        return None;
    }
    let pages = file
        .pages
        .0
        .into_iter()
        .map(|page| HistoryEntry {
            url: page.url.0,
            title: page.title.0,
            visit_count: page.visit_count,
            last_visit: page.last_visit,
        })
        .collect();
    let searches = file
        .searches
        .0
        .into_iter()
        .map(|search| SearchEntry {
            query: search.query.0,
            count: search.count,
            last_used: search.last_used,
        })
        .collect();
    Some(History::from_parts(pages, searches))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(decode(&page_count(HISTORY_MAX_PAGES)).is_some());
        assert!(decode(&page_count(HISTORY_MAX_PAGES + 1)).is_none());

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
        assert!(decode(&search_count(HISTORY_MAX_SEARCHES)).is_some());
        assert!(decode(&search_count(HISTORY_MAX_SEARCHES + 1)).is_none());

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
        assert!(
            decode(&string_fields(
                &"u".repeat(MAX_URL_BYTES),
                &"t".repeat(MAX_TITLE_BYTES),
                &"q".repeat(MAX_QUERY_BYTES),
            ))
            .is_some()
        );
        assert!(
            decode(&string_fields(
                &"u".repeat(MAX_URL_BYTES + 1),
                "title",
                "query",
            ))
            .is_none()
        );
        assert!(
            decode(&string_fields(
                "url",
                &"t".repeat(MAX_TITLE_BYTES + 1),
                "query",
            ))
            .is_none()
        );
        assert!(
            decode(&string_fields(
                "url",
                "title",
                &"q".repeat(MAX_QUERY_BYTES + 1),
            ))
            .is_none()
        );

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
        assert!(decode(&vec![b' '; MAX_HISTORY_FILE_BYTES + 1]).is_none());
        let nested = format!(
            "{{\"unknown\":{}}}",
            "[".repeat(130) + "null" + &"]".repeat(130)
        );
        assert!(decode(nested.as_bytes()).is_none());
    }
}
