//! Typing in the omnibox: suggesting, completing a visited address inline,
//! moving through suggestions, and choosing or forgetting one.

use gpui::{Action, Context, InteractiveElement, Window};
use gpui_elements::editable_text::actions::{
    DeleteLeft, DeleteRight, DeleteToLineEnd, DeleteToLineStart, DeleteWordLeft, DeleteWordRight,
    NavDown, NavUp, Tab,
};
use photon_core::SuggestionKind;

use super::super::history::BrowsingHistory;
use super::super::settings::Settings;
use super::Omnibox;
use std::sync::Arc;

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
        if text == self.current_url {
            self.close_suggestions(cx);
        } else {
            self.typed = text;
            self.suggest(!deleting, cx);
        }
        cx.notify();
    }

    /// Suggests again when the history itself changes while suggesting, as
    /// when a row is forgotten, but not when it only loads a page icon.
    pub(super) fn history_changed(&mut self, cx: &mut Context<Self>) {
        let history = BrowsingHistory::suggestion_history(cx);
        let unchanged = self
            .suggested_from
            .as_ref()
            .is_some_and(|from| Arc::ptr_eq(from, &history));
        if unchanged || self.typed.is_empty() {
            return;
        }
        self.suggest(false, cx);
    }

    /// Suggests for the text as typed, highlighting the first suggestion and
    /// completing it inline when `complete`. Without a completion the field
    /// and its caret are left as they are.
    fn suggest(&mut self, complete: bool, cx: &mut Context<Self>) {
        self.suggestion_generation = self.suggestion_generation.wrapping_add(1);
        let generation = self.suggestion_generation;
        // The rows for the earlier text stay until the new ones arrive, so the
        // panel neither closes nor replays its entrance between keystrokes.
        self.completion = None;
        self.selected = 0;

        let typed = self.typed.clone();
        let query_text = typed.clone();
        let engines = Settings::search_engines(cx);
        let history = BrowsingHistory::suggestion_history(cx);
        self.suggested_from = Some(history.clone());
        self.suggestion_task = Some(cx.spawn(async move |this, cx| {
            let suggestions = cx
                .background_executor()
                .spawn(async move { history.omnibox_suggestions_with(&query_text, &engines) })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.suggestion_generation != generation || this.typed != typed {
                    return;
                }
                this.suggestion_task = None;
                this.suggestions = suggestions.rows;
                this.hovered = this.hovered.filter(|&index| index < this.suggestions.len());
                this.completion = suggestions.completion.filter(|_| complete);
                if let Some(completion) = this.completion.clone() {
                    let selection_start = this.typed.len();
                    this.fill(&completion, selection_start, cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
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

    /// Removes the inline completion without changing the typed prefix or
    /// closing the suggestion panel.
    pub(super) fn dismiss_completion(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(completion) = self.completion.as_deref() else {
            return false;
        };
        if self.selected != 0 || completion == self.typed {
            return false;
        }

        let typed = self.typed.clone();
        self.completion = None;
        self.fill(&typed, typed.len(), cx);
        cx.notify();
        true
    }

    pub(super) fn close_suggestions(&mut self, cx: &mut Context<Self>) {
        self.suggestion_generation = self.suggestion_generation.wrapping_add(1);
        self.suggestion_task = None;
        self.suggested_from = None;
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
            .capture_action(cx.listener(Self::focus_next))
            .capture_action(cx.listener(Self::note_deletion::<DeleteLeft>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteRight>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteWordLeft>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteWordRight>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteToLineStart>))
            .capture_action(cx.listener(Self::note_deletion::<DeleteToLineEnd>))
    }

    fn focus_next(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
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
        cx.stop_propagation();
        self.select(self.selected.saturating_sub(1), cx);
    }

    fn select_next(&mut self, _: &NavDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.suggestions.is_empty() {
            cx.propagate();
            return;
        }
        cx.stop_propagation();
        self.select((self.selected + 1).min(self.suggestions.len() - 1), cx);
    }

    /// Highlights the suggestion at `index` and shows it in the field: the
    /// typed text and its completion for the first, otherwise the
    /// suggestion's address or query.
    pub(super) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
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
        let typed = self.typed.clone();
        self.suggest(true, cx);
        self.fill(&typed, typed.len(), cx);
    }

    /// Follows the pointer over the rows. Leaving one row and entering the
    /// next can arrive in either order.
    pub(super) fn hover(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        let next = if hovered {
            Some(index)
        } else if self.hovered == Some(index) {
            None
        } else {
            self.hovered
        };
        if self.hovered != next {
            self.hovered = next;
            cx.notify();
        }
    }
}
