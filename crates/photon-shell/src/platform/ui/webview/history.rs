//! Recording the page in the browsing history.

use gpui::Context;

use super::super::history::BrowsingHistory;
use super::PhotonWebView;

/// What a tab last recorded in the history, so unchanged state is not
/// recorded again on every page update.
#[derive(Default, PartialEq)]
pub(super) struct Recorded {
    url: String,
    title: String,
    favicon: Option<u64>,
}

impl PhotonWebView {
    /// Records a visit when the page's address changes, and keeps its title
    /// and icon up to date afterwards.
    pub(in super::super) fn record_history(&mut self, cx: &mut Context<Self>) {
        let url = &self.state.url;
        if url.is_empty() {
            return;
        }
        let current = Recorded {
            url: url.clone(),
            title: self.state.title.clone(),
            favicon: self.favicon.as_ref().map(|favicon| favicon.key),
        };
        if current == self.recorded {
            return;
        }
        if current.url != self.recorded.url {
            BrowsingHistory::record_visit(&current.url, &current.title, cx);
        } else if current.title != self.recorded.title && !current.title.is_empty() {
            BrowsingHistory::set_title(&current.url, &current.title, cx);
        }
        if current.favicon != self.recorded.favicon
            && let Some(favicon) = &self.favicon
        {
            BrowsingHistory::set_favicon(&current.url, favicon, cx);
        }
        self.recorded = current;
    }
}
