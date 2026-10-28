//! Finding text in the page.

use gpui::Context;

use super::super::super::trace;
use super::PhotonWebView;

/// Where a find-in-page search stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::platform) struct FindResult {
    pub(in crate::platform) current_match_index: usize,
    /// How many matches there are, once the page has counted them.
    pub(in crate::platform) total_match_count: Option<usize>,
}

impl PhotonWebView {
    /// Finds `query` in the page; an empty query clears the search.
    pub(in super::super) fn find(&mut self, query: &str, cx: &mut Context<Self>) {
        self.session.find_in_page(query);
        self.find_pending += 1;
        self.find_active = !query.is_empty();
        // The previous query's count no longer applies; the new one follows.
        if self.find_result.take().is_some() {
            self.state_changed(cx);
        }
    }

    /// Moves to the next match, or the previous one when `forward` is false.
    pub(in super::super) fn find_step(&mut self, forward: bool) {
        self.session.find_in_page_step(forward);
        self.find_pending += 1;
    }

    /// Ends the search and removes its highlights.
    pub(in super::super) fn end_find(&mut self, cx: &mut Context<Self>) {
        self.session.find_in_page_end();
        self.find_active = false;
        if self.find_result.take().is_some() {
            self.state_changed(cx);
        }
    }

    /// Drops the previous page's result; the new page has not been searched.
    pub(in crate::platform) fn clear_find_result(&mut self, cx: &mut Context<Self>) {
        // A new page, or a restarted one, will not answer the old requests.
        self.find_pending = 0;
        if self.find_result.take().is_some() {
            self.state_changed(cx);
        }
    }

    pub(in crate::platform) fn set_find_result(
        &mut self,
        result: FindResult,
        cx: &mut Context<Self>,
    ) {
        trace(format_args!("find result {result:?}"));
        // Engine reports arrive asynchronously, one per request, and may
        // answer an earlier query or outlive the search.
        self.find_pending = self.find_pending.saturating_sub(1);
        if !self.find_active || self.find_pending > 0 {
            return;
        }
        if self.find_result != Some(result) {
            self.find_result = Some(result);
            self.state_changed(cx);
        }
    }
}
