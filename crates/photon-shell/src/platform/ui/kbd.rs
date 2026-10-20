//! Keyboard shortcuts drawn in the platform's own notation. [`action_kbd`]
//! shows whatever keys are bound to an action, so labels follow the keymap.

use gpui::{Action, App, Div, KeyBinding, Modifiers, SharedString, div, prelude::*, px, rgb, rgba};

use super::layout::h_stack;
use super::{metrics, theme::ThemeColors};

/// The keycaps for the keys bound to `action`, if any are. With several
/// bindings, the one that takes precedence.
pub(super) fn action_kbd(action: &dyn Action, palette: ThemeColors, cx: &App) -> Option<Div> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let binding = keymap.bindings_for_action(action).last()?;
    Some(kbd(binding, palette))
}

/// The keys bound to `action` as text in the platform's notation, for a
/// sentence that mentions them.
pub(super) fn action_keys(action: &dyn Action, cx: &App) -> Option<String> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let binding = keymap.bindings_for_action(action).last()?;
    let notation = crate::platform::keyboard_notation();
    let keystrokes = binding.keystrokes().iter().map(|keystroke| {
        keycap_labels(keystroke.modifiers(), keystroke.key(), notation)
            .iter()
            .map(SharedString::as_ref)
            .collect::<Vec<_>>()
            .join(notation.separator)
    });
    Some(keystrokes.collect::<Vec<_>>().join(" "))
}

/// The keycaps for `binding`, one keystroke after another.
pub(super) fn kbd(binding: &KeyBinding, palette: ThemeColors) -> Div {
    let notation = crate::platform::keyboard_notation();
    let caps = binding
        .keystrokes()
        .iter()
        .flat_map(|keystroke| keycap_labels(keystroke.modifiers(), keystroke.key(), notation))
        .map(|label| keycap(label, palette));
    h_stack()
        .flex_shrink_0()
        .items_center()
        .gap(px(metrics::KBD_GAP))
        .children(caps)
}

/// A themed keycap for a fixed keyboard hint that is not bound to an action.
pub(super) fn keycap_label(label: impl Into<SharedString>, palette: ThemeColors) -> Div {
    keycap(label.into(), palette)
}

fn keycap(label: SharedString, palette: ThemeColors) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(metrics::KBD_HEIGHT))
        .min_w(px(metrics::KBD_HEIGHT))
        .px(px(metrics::KBD_PADDING))
        .rounded(px(metrics::keycap_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.surface))
        .text_size(px(metrics::KBD_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .child(label)
}

/// A keystroke's caps: its modifiers in the platform's order, then its key.
fn keycap_labels(
    modifiers: &Modifiers,
    key: &str,
    notation: crate::platform::KeyboardNotation,
) -> Vec<SharedString> {
    let held: &[(bool, &str, &str)] = &[
        (modifiers.function, "fn", "Fn"),
        (modifiers.control, "⌃", "Ctrl"),
        (modifiers.alt, "⌥", "Alt"),
        (modifiers.shift, "⇧", "Shift"),
        (modifiers.platform, "⌘", notation.platform_key),
    ];
    held.iter()
        .filter(|(down, ..)| *down)
        .map(|&(_, mac_label, label)| {
            SharedString::from(if notation.mac_symbols {
                mac_label
            } else {
                label
            })
        })
        .chain(std::iter::once(key_label(key, notation.mac_symbols)))
        .collect()
}

/// How a key is printed on its cap.
fn key_label(key: &str, mac: bool) -> SharedString {
    let named = match (key, mac) {
        ("enter", true) => "↩",
        ("enter", false) => "Enter",
        ("escape", true) => "⎋",
        ("escape", false) => "Esc",
        ("backspace", true) => "⌫",
        ("backspace", false) => "Backspace",
        ("delete", true) => "⌦",
        ("delete", false) => "Delete",
        ("tab", true) => "⇥",
        ("tab", false) => "Tab",
        ("space", _) => "Space",
        ("left", _) => "←",
        ("right", _) => "→",
        ("up", _) => "↑",
        ("down", _) => "↓",
        ("pageup", true) => "⇞",
        ("pageup", false) => "PgUp",
        ("pagedown", true) => "⇟",
        ("pagedown", false) => "PgDn",
        ("home", _) => "Home",
        ("end", _) => "End",
        _ => return key.to_uppercase().into(),
    };
    named.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(modifiers: Modifiers, key: &str, mac: bool) -> Vec<String> {
        keycap_labels(
            &modifiers,
            key,
            crate::platform::KeyboardNotation {
                mac_symbols: mac,
                platform_key: if mac { "⌘" } else { "Super" },
                separator: if mac { "" } else { "+" },
            },
        )
        .into_iter()
        .map(|label| label.to_string())
        .collect()
    }

    #[test]
    fn uses_symbols_in_apple_order_on_mac() {
        let modifiers = Modifiers {
            platform: true,
            shift: true,
            alt: true,
            control: true,
            ..Modifiers::default()
        };
        assert_eq!(labels(modifiers, "t", true), ["⌃", "⌥", "⇧", "⌘", "T"]);
    }

    #[test]
    fn spells_modifiers_out_elsewhere() {
        let modifiers = Modifiers {
            control: true,
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(labels(modifiers, "tab", false), ["Ctrl", "Shift", "Tab"]);
    }

    #[test]
    fn names_special_keys() {
        assert_eq!(labels(Modifiers::default(), "enter", true), ["↩"]);
        assert_eq!(labels(Modifiers::default(), "escape", false), ["Esc"]);
        assert_eq!(labels(Modifiers::default(), "left", false), ["←"]);
        assert_eq!(labels(Modifiers::default(), "f5", false), ["F5"]);
        assert_eq!(labels(Modifiers::default(), "[", true), ["["]);
    }
}
