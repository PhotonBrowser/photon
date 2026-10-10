//! Typing in the omnibox: suggesting, completing a visited address inline,
//! moving through suggestions, and choosing or forgetting one.

use gpui::{Action, Context, InteractiveElement, Window};
use gpui_elements::editable_text::actions::{
    DeleteLeft, DeleteRight, DeleteToLineEnd, DeleteToLineStart, DeleteWordLeft, DeleteWordRight,
    NavDown, NavUp,
};
use photon_core::SuggestionKind;

use super::super::history::BrowsingHistory;
use super::Omnibox;

impl Omnibox {
    /// Suggests what the typed text could open and completes a visited
    /// address inline, unless the edit removed text. Shows nothing while the
    /// field holds the page's own address.
    pub(super) fn text_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).as_str().to_owned();
        if self.filled.as_deref() == Some(text.as_str()) {
            return;
        }
        self.filled = None;
        self.invalid = false;
        let deleting = std::mem::take(&mut self.deleting);
        if text == self.webview.read(cx).omnibox_url() {
            self.close_suggestions(cx);
        } else {
            self.typed = text;
            self.suggest(!deleting, cx);
        }
        cx.notify();
    }

    /// Suggests for the text as typed, highlighting the first suggestion and
    /// completing it inline when `complete`. Without a completion the field
    /// and its caret are left as they are.
    fn suggest(&mut self, complete: bool, cx: &mut Context<Self>) {
        let suggestions = BrowsingHistory::suggestions(&self.typed, cx);
        self.suggestions = suggestions.rows;
        self.completion = suggestions.completion.filter(|_| complete);
        self.selected = 0;
        self.hovered = None;
        if let Some(completion) = self.completion.clone() {
            self.fill(&completion, self.typed.len(), cx);
        }
    }

    /// Puts `text` in the field without it counting as an edit, selecting
    /// from `selection_start` to the end.
    fn fill(&mut self, text: &str, selection_start: usize, cx: &mut Context<Self>) {
        self.filled = Some(text.to_owned());
        self.input.update(cx, |input, cx| {
            if input.as_str() != text {
                input.emplace(text, cx);
            }
            input.move_to(selection_start, cx);
            input.select_to(text.len(), cx);
        });
    }

    pub(super) fn close_suggestions(&mut self, cx: &mut Context<Self>) {
        self.typed.clear();
        self.completion = None;
        self.selected = 0;
        self.hovered = None;
        if !self.suggestions.is_empty() {
            self.suggestions.clear();
            cx.notify();
        }
    }

    /// Notes edits that remove text, so the edit is not completed again, and
    /// moves the highlight with the arrow keys while there are suggestions.
    pub(super) fn handle_editing_keys<E: InteractiveElement>(
        &self,
        element: E,
        cx: &mut Context<Self>,
    ) -> E {
        element
            .capture_action(cx.listener(Self::select_previous))
            .capture_action(cx.listener(Self::select_next))
            .capture_action(cx.listener(Self::note_deletion::<DeleteLeft>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteRight>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteWordLeft>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteWordRight>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteToLineStart>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteToLineEnd>))
    }

    fn note_deletion<A: Action>(&mut self, _: &A, _: &mut Window, cx: &mut Context<Self>) {
        self.deleting = true;
        cx.propagate();
    }

    fn select_previous(&mut self, _: &NavUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.suggestions.is_empty() {
            cx.propagate();
            return;
        }
        self.select(self.selected.saturating_sub(1), cx);
    }

    fn select_next(&mut self, _: &NavDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.suggestions.is_empty() {
            cx.propagate();
            return;
        }
        self.select((self.selected + 1).min(self.suggestions.len() - 1), cx);
    }

    /// Highlights the suggestion at `index` and shows it in the field: the
    /// typed text and its completion for the first, otherwise the
    /// suggestion's address or query.
    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        let (text, selection_start) = match (index, &self.completion) {
            (0, Some(completion)) => (completion.clone(), self.typed.len()),
            (0, None) => (self.typed.clone(), self.typed.len()),
            _ => {
                let text = self.suggestions[index].text.clone();
                let end = text.len();
                (text, end)
            }
        };
        self.fill(&text, selection_start, cx);
        cx.notify();
    }

    /// Opens the suggestion at `index`, remembering it when it is a search.
    pub(super) fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(suggestion) = self.suggestions.get(index).cloned() else {
            return;
        };
        if matches!(
            suggestion.kind,
            SuggestionKind::Search { .. } | SuggestionKind::PastSearch
        ) {
            BrowsingHistory::record_search(&suggestion.text, cx);
        }
        self.close_suggestions(cx);
        self.open(&suggestion.url, window, cx);
    }

    /// Forgets the page or search at `index` and suggests again for the
    /// text as typed.
    pub(super) fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(suggestion) = self.suggestions.get(index).cloned() else {
            return;
        };
        match suggestion.kind {
            SuggestionKind::Page { .. } => BrowsingHistory::remove_page(&suggestion.url, cx),
            SuggestionKind::PastSearch => BrowsingHistory::remove_search(&suggestion.text, cx),
            SuggestionKind::Address | SuggestionKind::Search { .. } => return,
        }
        // The field may show the removed row; return to the typed text.
        self.suggest(true, cx);
        self.select(0, cx);
    }

    /// Follows the pointer over the rows. Leaving one row and entering the
    /// next can arrive in either order.
    pub(super) fn hover(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if index.is_some() || self.hovered.is_some() {
            self.hovered = index;
            cx.notify();
        }
    }
}
