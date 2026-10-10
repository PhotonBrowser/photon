//! Recording the page in the browsing history.

use gpui::Context;

use super::super::history::BrowsingHistory;
use super::PhotonWebView;

impl PhotonWebView {
    /// Records a visit when the page's address changes, and keeps its title
    /// and icon up to date afterwards.
    pub(in super::super) fn record_history(&mut self, cx: &mut Context<Self>) {
        let url = self.state.url.clone();
        if url.is_empty() {
            return;
        }
        if self.recorded_url.as_deref() != Some(url.as_str()) {
            BrowsingHistory::record_visit(&url, &self.state.title, cx);
            self.recorded_url = Some(url.clone());
        } else if !self.state.title.is_empty() {
            BrowsingHistory::set_title(&url, &self.state.title, cx);
        }
        if let Some(favicon) = &self.favicon {
            BrowsingHistory::set_favicon(&url, favicon, cx);
        }
    }
}
