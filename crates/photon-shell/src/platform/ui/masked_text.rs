//! A text field that shows a mark in place of each character, for passwords.
//!
//! GPUI-CE's editable text cannot mask its contents yet, so the field holds
//! only marks and the secret is kept here. After each edit the field is read
//! for what changed, the secret is updated, and the field is reset to marks.

use gpui::{App, AppContext as _, Context, Entity, Subscription};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

/// The character shown for each character of the secret.
const MASK: char = '\u{2022}';

/// The secret behind a masked field.
#[derive(Default)]
pub(super) struct MaskedText {
    secret: String,
}

impl MaskedText {
    pub(super) fn secret(&self) -> &str {
        &self.secret
    }

    /// Takes in an edit to the field, which shows `visible` with the caret at
    /// byte `caret`. Returns the marks to show instead and the caret's byte
    /// offset among them, or `None` when the field already shows only marks.
    ///
    /// An edit inserts the text just before the caret that is not marks and
    /// keeps the marks around it, which covers typing, pasting, deleting in
    /// either direction and replacing a selection.
    fn apply_edit(&mut self, visible: &str, caret: usize) -> Option<(String, usize)> {
        let secret: Vec<char> = self.secret.chars().collect();
        let visible: Vec<char> = visible.chars().collect();
        if visible.len() == secret.len() && visible.iter().all(|&c| c == MASK) {
            return None;
        }

        let caret = visible.len().min(visible_prefix_chars(&visible, caret));
        let inserted = visible[..caret]
            .iter()
            .rev()
            .take_while(|&&c| c != MASK)
            .count();
        let kept_before = caret - inserted;
        let kept_after = visible.len() - caret;
        let updated: String = if kept_before + kept_after <= secret.len() {
            secret[..kept_before]
                .iter()
                .chain(&visible[kept_before..caret])
                .chain(&secret[secret.len() - kept_after..])
                .collect()
        } else {
            // Not a single edit, as when an undo brings back other text: keep
            // what is shown, so nothing typed is lost.
            visible.iter().filter(|&&c| c != MASK).collect()
        };

        let length = updated.chars().count();
        let caret = caret.min(length);
        self.secret = updated;
        Some((MASK.to_string().repeat(length), caret * MASK.len_utf8()))
    }
}

/// How many characters of `text` come before byte `offset`.
fn visible_prefix_chars(text: &[char], offset: usize) -> usize {
    let mut bytes = 0;
    text.iter()
        .take_while(|c| {
            bytes += c.len_utf8();
            bytes <= offset
        })
        .count()
}

/// Creates a field state that masks what is typed into it, with the secret
/// kept in `secret` on `V`.
pub(super) fn masked_input<V: 'static>(
    secret: impl Fn(&mut V) -> &mut MaskedText + 'static,
    cx: &mut Context<V>,
) -> (Entity<EditableTextState>, Subscription) {
    let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
    let subscription = cx.subscribe(&input, move |view, input, _: &TextChanged, cx| {
        let (visible, caret) = read(&input, cx);
        if let Some((masked, caret)) = secret(view).apply_edit(&visible, caret) {
            input.update(cx, |input, cx| {
                input.emplace(&masked, cx);
                input.move_to(caret, cx);
            });
        }
    });
    (input, subscription)
}

fn read(input: &Entity<EditableTextState>, cx: &App) -> (String, usize) {
    let input = input.read(cx);
    (input.as_str().to_owned(), input.selected_range().end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(count: usize) -> String {
        MASK.to_string().repeat(count)
    }

    fn masked(secret: &str) -> MaskedText {
        MaskedText {
            secret: secret.into(),
        }
    }

    #[test]
    fn typing_appends_and_masks() {
        let mut text = MaskedText::default();
        assert_eq!(text.apply_edit("a", 1), Some((marks(1), 3)));
        let visible = format!("{}b", marks(1));
        assert_eq!(
            text.apply_edit(&visible, visible.len()),
            Some((marks(2), 6))
        );
        assert_eq!(text.secret(), "ab");
    }

    #[test]
    fn a_field_showing_only_marks_is_unchanged() {
        let mut text = masked("ab");
        assert_eq!(text.apply_edit(&marks(2), 6), None);
    }

    #[test]
    fn typing_in_the_middle_inserts_there() {
        let mut text = masked("ac");
        let visible = format!("{}b{}", marks(1), marks(1));
        assert_eq!(text.apply_edit(&visible, 4), Some((marks(3), 6)));
        assert_eq!(text.secret(), "abc");
    }

    #[test]
    fn deleting_backward_removes_before_the_caret() {
        let mut text = masked("abc");
        // Backspace after "b" leaves the caret after "a".
        assert_eq!(text.apply_edit(&marks(2), 3), Some((marks(2), 3)));
        assert_eq!(text.secret(), "ac");
    }

    #[test]
    fn deleting_forward_removes_after_the_caret() {
        let mut text = masked("abc");
        assert_eq!(text.apply_edit(&marks(2), 0), Some((marks(2), 0)));
        assert_eq!(text.secret(), "bc");
    }

    #[test]
    fn pasting_over_a_selection_replaces_it() {
        let mut text = masked("abcd");
        // "bc" selected and replaced with "XYZ".
        let visible = format!("{}XYZ{}", marks(1), marks(1));
        assert_eq!(text.apply_edit(&visible, 6), Some((marks(5), 12)));
        assert_eq!(text.secret(), "aXYZd");
    }

    #[test]
    fn multibyte_characters_are_kept_whole() {
        let mut text = MaskedText::default();
        assert_eq!(text.apply_edit("é", 2), Some((marks(1), 3)));
        assert_eq!(text.secret(), "é");
    }
}
