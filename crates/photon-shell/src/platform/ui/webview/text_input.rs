//! Platform input methods for the page: dead-key accents, CJK input methods,
//! the emoji picker, and dictation.
//!
//! Ordinary key presses still reach the page as key events, which insert
//! their own text, so pages see the same `keydown` they always did. Only text
//! an input method composes or commits comes through GPUI's input handler and
//! on to the Engine's composition API, as Ladybird's Qt `inputMethodEvent`
//! does. Offsets here count UTF-16 code units from the start of the focused
//! editable's text, as the platforms and the Engine do.

use super::PhotonWebView;
use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, FocusHandle, KeyDownEvent, Pixels,
    UTF16Selection, Window, canvas, prelude::*, utf16_to_utf8_offset,
};
use std::ops::Range;

/// The focused editable's state, as the Engine last reported it.
#[derive(Clone, Debug, Default)]
pub(in crate::platform) struct InputMethodState {
    pub enabled: bool,
    pub cursor: usize,
    pub anchor: usize,
    /// Text just before the cursor, which may start after the editable's start.
    pub text_before_cursor: String,
    pub text_after_cursor: String,
    /// The caret in logical pixels from the page's top-left corner.
    pub caret: Option<Bounds<Pixels>>,
}

impl InputMethodState {
    fn selection(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    /// Where the reported surrounding text starts in the editable's text.
    fn surrounding_start(&self) -> usize {
        self.cursor
            .saturating_sub(utf16_len(&self.text_before_cursor))
    }
}

#[derive(Default)]
pub(super) struct TextInput {
    state: InputMethodState,
    /// The input method's in-progress text, which the Engine shows at the
    /// selection's start until it is committed or cancelled.
    marked: Option<Range<usize>>,
    /// Text the current key press already inserted as a key event. Windows
    /// and X11 deliver a key's character to the input handler as well, so it
    /// is dropped there rather than typed twice.
    key_text: Option<String>,
}

impl TextInput {
    pub(super) fn set_state(&mut self, state: InputMethodState) {
        if !state.enabled {
            self.marked = None;
        }
        self.state = state;
    }

    /// Records the text a key press inserts through its key event.
    pub(super) fn key_pressed(&mut self, event: &KeyDownEvent) {
        let modifiers = event.keystroke.modifiers;
        self.key_text = (!modifiers.control && !modifiers.platform)
            .then(|| event.keystroke.key_char.clone())
            .flatten();
    }

    pub(super) fn key_released(&mut self) {
        self.key_text = None;
    }

    /// Text a single key produced that a key event cannot carry, as when a
    /// Linux input method commits several characters through one key press.
    pub(super) fn text_for_key(event: &KeyDownEvent) -> Option<&str> {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform {
            return None;
        }
        let text = event.keystroke.key_char.as_deref()?;
        (text.chars().nth(1).is_some() && !text.chars().any(char::is_control)).then_some(text)
    }
}

/// Lets the platform's input method reach the page while it has focus.
pub(super) fn input_handler_element(
    focus_handle: FocusHandle,
    view: Entity<PhotonWebView>,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window: &mut Window, cx: &mut App| {
            window.handle_input(&focus_handle, ElementInputHandler::new(bounds, view), cx);
        },
    )
    .absolute()
    .size_full()
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn clamp_i32(value: isize) -> i32 {
    value.clamp(i32::MIN as isize, i32::MAX as isize) as i32
}

impl PhotonWebView {
    pub(in crate::platform) fn set_input_method_state(
        &mut self,
        state: InputMethodState,
        _cx: &mut Context<Self>,
    ) {
        self.text_input.set_state(state);
    }

    /// Ends a composition left open when the page loses focus, keeping its
    /// text as typed, as other browsers do.
    pub(super) fn end_composition(&mut self) {
        if self.text_input.marked.take().is_some() {
            self.session.unmark_text();
        }
    }
}

impl gpui::EntityInputHandler for PhotonWebView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let state = &self.text_input.state;
        let text = format!("{}{}", state.text_before_cursor, state.text_after_cursor);
        let start = state.surrounding_start();
        let end = start + utf16_len(&text);
        let range = range.start.clamp(start, end)..range.end.clamp(start, end);
        *adjusted_range = Some(range.clone());
        let from = utf16_to_utf8_offset(&text, range.start - start);
        let to = utf16_to_utf8_offset(&text, range.end - start);
        Some(text[from..to].to_owned())
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let state = &self.text_input.state;
        if !state.enabled && !ignore_disabled_input {
            return None;
        }
        if let Some(marked) = &self.text_input.marked {
            return Some(UTF16Selection {
                range: marked.end..marked.end,
                reversed: false,
            });
        }
        Some(UTF16Selection {
            range: state.selection(),
            reversed: state.cursor < state.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.text_input.marked.clone()
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.end_composition();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let input = &mut self.text_input;
        if input.marked.is_none() && range.is_none() && input.key_text.as_deref() == Some(text) {
            input.key_text = None;
            return;
        }
        // Committed text replaces the marked text. Without a composition, a
        // range replaces existing text, as when an accent is chosen for the
        // character before the cursor.
        let (start, length) = match (input.marked.take(), range) {
            (None, Some(range)) => (
                clamp_i32(range.start as isize - input.state.cursor as isize),
                clamp_i32(range.len() as isize),
            ),
            _ => (0, 0),
        };
        if text.is_empty() && length == 0 {
            self.session.set_marked_text("");
            return;
        }
        self.session.commit_text(text, start, length);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        let input = &mut self.text_input;
        if new_text.is_empty() {
            input.marked = None;
        } else {
            let start = input
                .marked
                .as_ref()
                .map_or_else(|| input.state.selection().start, |marked| marked.start);
            input.marked = Some(start..start + utf16_len(new_text));
        }
        // The Engine replaces the previous marked text itself.
        self.session.set_marked_text(new_text);
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // The Engine reports only the caret, which is where input method
        // windows such as candidate lists belong.
        let caret = self.text_input.state.caret?;
        Some(Bounds::new(
            element_bounds.origin + caret.origin,
            caret.size,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.text_input.state.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(before: &str, after: &str, cursor: usize) -> InputMethodState {
        InputMethodState {
            enabled: true,
            cursor,
            anchor: cursor,
            text_before_cursor: before.into(),
            text_after_cursor: after.into(),
            caret: None,
        }
    }

    #[test]
    fn surrounding_text_starts_before_the_cursor() {
        // A window of text that starts partway into the editable.
        let state = state("日本", "語", 12);
        assert_eq!(state.surrounding_start(), 10);
    }

    #[test]
    fn surrounding_start_counts_utf16_units() {
        let state = state("a😀", "", 3);
        assert_eq!(state.surrounding_start(), 0);
    }

    #[test]
    fn reversed_selection_orders_its_range() {
        let mut state = state("abc", "", 3);
        state.anchor = 1;
        assert_eq!(state.selection(), 1..3);
    }

    #[test]
    fn disabled_state_ends_a_composition() {
        let mut input = TextInput {
            marked: Some(0..1),
            ..Default::default()
        };
        input.set_state(InputMethodState::default());
        assert!(input.marked.is_none());
    }
}
