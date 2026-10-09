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
        if query.is_empty() && self.find_result.take().is_some() {
            self.state_changed(cx);
        }
    }

    /// Moves to the next match, or the previous one when `forward` is false.
    pub(in super::super) fn find_step(&mut self, forward: bool) {
        self.session.find_in_page_step(forward);
    }

    /// Ends the search and removes its highlights.
    pub(in super::super) fn end_find(&mut self, cx: &mut Context<Self>) {
        self.session.find_in_page_end();
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
        if self.find_result != Some(result) {
            self.find_result = Some(result);
            self.state_changed(cx);
        }
    }
}
