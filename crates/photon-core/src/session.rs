//! The open windows and their tabs, saved so the browser can reopen them as
//! they were after a restart.

use serde::{Deserialize, Serialize};

use super::spaces::SpaceId;

/// Every open window, front to back.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Session {
    pub windows: Vec<WindowSession>,
}

/// A window's everyday tabs, which pinned tabs and favourites do not count
/// among: those reopen from the spaces that keep them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct WindowSession {
    pub tabs: Vec<SessionTab>,
    /// The tab in use, as a position in `tabs`, when it is one of them.
    pub active: Option<usize>,
}

/// A tab's page and the space it belongs to.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SessionTab {
    pub url: String,
    pub title: String,
    pub space: SpaceId,
}

impl Session {
    /// Whether there is anything to reopen.
    pub fn is_empty(&self) -> bool {
        self.windows.iter().all(|window| window.tabs.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_of_empty_windows_has_nothing_to_reopen() {
        let mut session = Session {
            windows: vec![WindowSession::default()],
        };
        assert!(session.is_empty());
        session.windows[0].tabs.push(SessionTab {
            url: "https://example.com/".to_owned(),
            ..SessionTab::default()
        });
        assert!(!session.is_empty());
    }
}
