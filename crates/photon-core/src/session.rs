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
    /// The space the window showed. Older sessions lack it, and their
    /// windows open in the space new windows do.
    pub space: Option<SpaceId>,
    /// Two of `tabs` shown side by side, which reopen as a split.
    pub split: Option<SessionSplit>,
}

/// A split of two saved tabs, as positions in their window's `tabs`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SessionSplit {
    pub leading: usize,
    pub trailing: usize,
    /// The leading pane's share of the width, in thousandths.
    pub leading_share: u16,
}

impl SessionSplit {
    /// The leading pane's share of the width, from 0 to 1.
    pub fn share(self) -> f32 {
        f32::from(self.leading_share) / 1000.0
    }

    /// The split of the tabs at `leading` and `trailing`, the leading one
    /// taking `share` of the width.
    pub fn new(leading: usize, trailing: usize, share: f32) -> Self {
        Self {
            leading,
            trailing,
            leading_share: (share.clamp(0.0, 1.0) * 1000.0).round() as u16,
        }
    }

    /// Whether both positions name distinct tabs among `count`.
    pub fn fits(self, count: usize) -> bool {
        self.leading != self.trailing && self.leading < count && self.trailing < count
    }
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

    #[test]
    fn windows_saved_before_spaces_were_remembered_have_none() {
        let window: WindowSession = serde_json::from_str(r#"{"tabs":[],"active":null}"#).unwrap();
        assert_eq!(window.space, None);
    }

    #[test]
    fn windows_saved_before_splits_were_remembered_have_none() {
        let window: WindowSession = serde_json::from_str(r#"{"tabs":[],"active":null}"#).unwrap();
        assert_eq!(window.split, None);
    }

    #[test]
    fn a_split_keeps_its_share_and_checks_its_tabs() {
        let split = SessionSplit::new(0, 2, 0.4);
        assert_eq!(split.leading_share, 400);
        assert!((split.share() - 0.4).abs() < f32::EPSILON);
        assert!(split.fits(3));
        assert!(!split.fits(2));
        assert!(!SessionSplit::new(1, 1, 0.5).fits(3));
    }
}
