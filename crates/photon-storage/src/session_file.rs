//! Bounds for saved windows and tabs before they enter browser state.

use photon_core::{Session, SessionTab};
use serde_json::Value;
use std::io;

/// Maximum encoded session size read from or written to a profile.
pub(crate) const MAX_SESSION_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_WINDOWS: usize = 64;
const MAX_TABS_PER_WINDOW: usize = 256;
const MAX_TABS: usize = 2_048;
const MAX_URL_BYTES: usize = 8 * 1024;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_COLLECTION_ITEMS: usize = 4_096;
const MAX_STRING_BYTES: usize = 8 * 1024;

pub(crate) fn encode(session: &Session) -> io::Result<Vec<u8>> {
    validate_session(session).map_err(invalid_data)?;
    let bytes = serde_json::to_vec(session)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if bytes.len() > MAX_SESSION_FILE_BYTES {
        return Err(invalid_data("session file exceeds its size limit"));
    }
    Ok(bytes)
}

/// Decodes bounded profile data. The source bytes are left untouched on error.
pub(crate) fn decode(bytes: &[u8]) -> Option<Session> {
    if bytes.len() > MAX_SESSION_FILE_BYTES {
        return None;
    }
    let document: Value = serde_json::from_slice(bytes).ok()?;
    validate_json_tree(&document, 0).ok()?;
    validate_session_value(&document).ok()?;
    let session: Session = serde_json::from_value(document).ok()?;
    validate_session(&session).ok()?;
    Some(session)
}

fn validate_session(session: &Session) -> Result<(), &'static str> {
    if session.windows.len() > MAX_WINDOWS {
        return Err("session has too many windows");
    }
    let mut total_tabs = 0;
    for window in &session.windows {
        if window.tabs.len() > MAX_TABS_PER_WINDOW {
            return Err("session window has too many tabs");
        }
        total_tabs += window.tabs.len();
        if window
            .active
            .is_some_and(|active| active >= window.tabs.len())
        {
            return Err("session has an invalid active tab index");
        }
        for tab in &window.tabs {
            validate_tab(tab)?;
        }
    }
    if total_tabs > MAX_TABS {
        return Err("session has too many tabs");
    }
    Ok(())
}

fn validate_tab(tab: &SessionTab) -> Result<(), &'static str> {
    if tab.url.len() > MAX_URL_BYTES || tab.title.len() > MAX_TITLE_BYTES {
        return Err("session tab contains a string that exceeds its size limit");
    }
    Ok(())
}

fn validate_session_value(document: &Value) -> Result<(), &'static str> {
    let Value::Object(fields) = document else {
        return Err("session must be a JSON object");
    };
    let Some(windows) = fields.get("windows") else {
        return Ok(());
    };
    let Value::Array(windows) = windows else {
        return Err("session windows must be a list");
    };
    if windows.len() > MAX_WINDOWS {
        return Err("session has too many windows");
    }

    let mut total_tabs = 0;
    for window in windows {
        let Value::Object(fields) = window else {
            continue;
        };
        let Some(tabs) = fields.get("tabs") else {
            continue;
        };
        let Value::Array(tabs) = tabs else {
            continue;
        };
        if tabs.len() > MAX_TABS_PER_WINDOW {
            return Err("session window has too many tabs");
        }
        total_tabs += tabs.len();
        for tab in tabs {
            let Value::Object(fields) = tab else {
                continue;
            };
            if fields
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.len() > MAX_URL_BYTES)
                || fields
                    .get("title")
                    .and_then(Value::as_str)
                    .is_some_and(|title| title.len() > MAX_TITLE_BYTES)
            {
                return Err("session tab contains a string that exceeds its size limit");
            }
        }
    }
    if total_tabs > MAX_TABS {
        return Err("session has too many tabs");
    }

    Ok(())
}

fn validate_json_tree(value: &Value, depth: usize) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("session nesting exceeds its depth limit");
    }
    match value {
        Value::String(string) if string.len() > MAX_STRING_BYTES => {
            Err("session string exceeds its size limit")
        }
        Value::Array(items) => {
            if items.len() > MAX_COLLECTION_ITEMS {
                return Err("session list exceeds its item limit");
            }
            items
                .iter()
                .try_for_each(|item| validate_json_tree(item, depth + 1))
        }
        Value::Object(fields) => {
            if fields.len() > MAX_COLLECTION_ITEMS {
                return Err("session object exceeds its field limit");
            }
            for (key, value) in fields {
                if key.len() > 256 {
                    return Err("session field name exceeds its size limit");
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

    fn json_with_windows(count: usize) -> Vec<u8> {
        let windows = (0..count)
            .map(|_| r#"{"tabs":[]}"#)
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"windows":[{windows}]}}"#).into_bytes()
    }

    fn json_with_tabs(count: usize) -> Vec<u8> {
        let tabs = (0..count)
            .map(|_| r#"{"url":"https://example.com/","title":"Example","space":1}"#)
            .collect::<Vec<_>>()
            .join(",");
        format!(r#"{{"windows":[{{"tabs":[{tabs}]}}]}}"#).into_bytes()
    }

    fn json_with_total_tabs(count: usize) -> Vec<u8> {
        let mut remaining = count;
        let mut windows = Vec::new();
        while remaining > 0 {
            let count = remaining.min(MAX_TABS_PER_WINDOW);
            let tabs = (0..count)
                .map(|_| r#"{"url":"https://example.com/","title":"Example","space":1}"#)
                .collect::<Vec<_>>()
                .join(",");
            windows.push(format!(r#"{{"tabs":[{tabs}]}}"#));
            remaining -= count;
        }
        format!(r#"{{"windows":[{}]}}"#, windows.join(",")).into_bytes()
    }

    #[test]
    fn accepts_sessions_at_the_window_and_tab_limits() {
        assert!(decode(&json_with_windows(MAX_WINDOWS)).is_some());
        assert!(decode(&json_with_tabs(MAX_TABS_PER_WINDOW)).is_some());
        assert!(decode(&json_with_total_tabs(MAX_TABS)).is_some());
    }

    #[test]
    fn rejects_sessions_over_window_and_tab_limits() {
        assert!(decode(&json_with_windows(MAX_WINDOWS + 1)).is_none());
        assert!(decode(&json_with_tabs(MAX_TABS_PER_WINDOW + 1)).is_none());
        assert!(decode(&json_with_total_tabs(MAX_TABS + 1)).is_none());
    }

    #[test]
    fn rejects_long_strings_and_excessive_nesting() {
        let max_url = "x".repeat(MAX_URL_BYTES);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"url":"{max_url}"}}]}}]}}"#);
        assert!(decode(bytes.as_bytes()).is_some());

        let long_url = "x".repeat(MAX_URL_BYTES + 1);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"url":"{long_url}"}}]}}]}}"#);
        assert!(decode(bytes.as_bytes()).is_none());

        let long_title = "x".repeat(MAX_TITLE_BYTES + 1);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"title":"{long_title}"}}]}}]}}"#);
        assert!(decode(bytes.as_bytes()).is_none());

        let nested = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert!(decode(nested.as_bytes()).is_none());

        let accepted_items = serde_json::json!({"extra": vec![true; MAX_COLLECTION_ITEMS]});
        assert!(decode(&serde_json::to_vec(&accepted_items).unwrap()).is_some());
        let rejected_items = serde_json::json!({"extra": vec![true; MAX_COLLECTION_ITEMS + 1]});
        assert!(decode(&serde_json::to_vec(&rejected_items).unwrap()).is_none());
    }

    #[test]
    fn save_rejects_an_invalid_active_tab_index() {
        let session = Session {
            windows: vec![photon_core::WindowSession {
                tabs: Vec::new(),
                active: Some(0),
            }],
        };
        assert!(encode(&session).is_err());
    }
}
