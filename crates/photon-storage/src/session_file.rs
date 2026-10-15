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
const MAX_STRING_BYTES: usize = 64 * 1024;

pub(crate) fn encode(session: &Session) -> io::Result<Vec<u8>> {
    validate_session(session).map_err(invalid_data)?;
    let bytes = serde_json::to_vec(session)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if bytes.len() > MAX_SESSION_FILE_BYTES {
        return Err(invalid_data("session file exceeds its size limit"));
    }
    Ok(bytes)
}

/// Decodes bounded profile data and recovers complete windows and tabs when
/// individual records are damaged.
pub(crate) fn decode(bytes: &[u8]) -> super::ProfileLoadOutcome<Session> {
    if bytes.len() > MAX_SESSION_FILE_BYTES {
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
    let Some(windows_value) = fields.get("windows") else {
        return super::ProfileLoadOutcome::Loaded(Session::default());
    };
    let Value::Array(windows_value) = windows_value else {
        return super::ProfileLoadOutcome::Corrupt;
    };
    if windows_value.len() > MAX_WINDOWS {
        return super::ProfileLoadOutcome::Corrupt;
    }

    let mut partially_recovered = false;
    let mut total_tabs = 0;
    let mut windows = Vec::with_capacity(windows_value.len());
    for window_value in windows_value {
        let Value::Object(fields) = window_value else {
            partially_recovered = true;
            continue;
        };
        let tabs_value = match fields.get("tabs") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(tabs)) => tabs.as_slice(),
            _ => {
                partially_recovered = true;
                continue;
            }
        };
        if tabs_value.len() > MAX_TABS_PER_WINDOW {
            return super::ProfileLoadOutcome::Corrupt;
        }
        total_tabs += tabs_value.len();
        if total_tabs > MAX_TABS {
            return super::ProfileLoadOutcome::Corrupt;
        }

        let mut tabs = Vec::with_capacity(tabs_value.len());
        let mut original_to_recovered = Vec::with_capacity(tabs_value.len());
        for tab_value in tabs_value {
            let Ok(tab) = serde_json::from_value::<SessionTab>(tab_value.clone()) else {
                partially_recovered = true;
                original_to_recovered.push(None);
                continue;
            };
            if validate_tab(&tab).is_err() {
                partially_recovered = true;
                original_to_recovered.push(None);
                continue;
            }
            original_to_recovered.push(Some(tabs.len()));
            tabs.push(tab);
        }

        let active = match fields.get("active") {
            None | Some(Value::Null) => None,
            Some(value) => match value.as_u64().and_then(|index| usize::try_from(index).ok()) {
                Some(index) if index < original_to_recovered.len() => {
                    match original_to_recovered[index] {
                        Some(recovered_index) => Some(recovered_index),
                        None => {
                            partially_recovered = true;
                            None
                        }
                    }
                }
                _ => {
                    partially_recovered = true;
                    None
                }
            },
        };
        windows.push(photon_core::WindowSession { tabs, active });
    }

    let session = Session { windows };
    if partially_recovered {
        super::ProfileLoadOutcome::PartiallyRecovered(session)
    } else {
        super::ProfileLoadOutcome::Loaded(session)
    }
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
    use crate::ProfileLoadOutcome;

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
        assert!(matches!(
            decode(&json_with_windows(MAX_WINDOWS)),
            ProfileLoadOutcome::Loaded(_)
        ));
        assert!(matches!(
            decode(&json_with_tabs(MAX_TABS_PER_WINDOW)),
            ProfileLoadOutcome::Loaded(_)
        ));
        assert!(matches!(
            decode(&json_with_total_tabs(MAX_TABS)),
            ProfileLoadOutcome::Loaded(_)
        ));
    }

    #[test]
    fn rejects_sessions_over_window_and_tab_limits() {
        assert!(matches!(
            decode(&json_with_windows(MAX_WINDOWS + 1)),
            ProfileLoadOutcome::Corrupt
        ));
        assert!(matches!(
            decode(&json_with_tabs(MAX_TABS_PER_WINDOW + 1)),
            ProfileLoadOutcome::Corrupt
        ));
        assert!(matches!(
            decode(&json_with_total_tabs(MAX_TABS + 1)),
            ProfileLoadOutcome::Corrupt
        ));
    }

    #[test]
    fn rejects_long_strings_and_excessive_nesting() {
        let max_url = "x".repeat(MAX_URL_BYTES);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"url":"{max_url}"}}]}}]}}"#);
        assert!(matches!(
            decode(bytes.as_bytes()),
            ProfileLoadOutcome::Loaded(_)
        ));

        let long_url = "x".repeat(MAX_URL_BYTES + 1);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"url":"{long_url}"}}]}}]}}"#);
        let ProfileLoadOutcome::PartiallyRecovered(session) = decode(bytes.as_bytes()) else {
            panic!("skips a tab with an oversized URL");
        };
        assert!(session.is_empty());

        let long_title = "x".repeat(MAX_TITLE_BYTES + 1);
        let bytes = format!(r#"{{"windows":[{{"tabs":[{{"title":"{long_title}"}}]}}]}}"#);
        assert!(matches!(
            decode(bytes.as_bytes()),
            ProfileLoadOutcome::PartiallyRecovered(_)
        ));

        let nested = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert!(matches!(
            decode(nested.as_bytes()),
            ProfileLoadOutcome::Corrupt
        ));

        let accepted_items = serde_json::json!({"extra": vec![true; MAX_COLLECTION_ITEMS]});
        assert!(matches!(
            decode(&serde_json::to_vec(&accepted_items).unwrap()),
            ProfileLoadOutcome::Loaded(_)
        ));
        let rejected_items = serde_json::json!({"extra": vec![true; MAX_COLLECTION_ITEMS + 1]});
        assert!(matches!(
            decode(&serde_json::to_vec(&rejected_items).unwrap()),
            ProfileLoadOutcome::Corrupt
        ));
    }

    #[test]
    fn recovers_good_tabs_from_a_partially_damaged_session() {
        let bytes = br#"{"windows":[{"tabs":[{"url":"https://ok.example/","title":"OK","space":1},17],"active":0}]}"#;
        let ProfileLoadOutcome::PartiallyRecovered(session) = decode(bytes) else {
            panic!("recovers the valid tab");
        };
        assert_eq!(session.windows[0].tabs.len(), 1);
        assert_eq!(session.windows[0].active, Some(0));
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
