//! Default key bindings for browser-level actions.

use gpui::{DummyKeyboardMapper, KeyBinding};

use crate::catalog::shortcut_specs;

/// Returns the browser's default keyboard shortcuts.
pub fn browser_shortcuts() -> Vec<KeyBinding> {
    shortcut_specs()
        .iter()
        .flat_map(|shortcut| {
            shortcut.keys.iter().map(|keystrokes| {
                KeyBinding::load(
                    keystrokes,
                    shortcut.action.action(),
                    None,
                    false,
                    None,
                    &DummyKeyboardMapper,
                )
                .expect("shortcut registry contains valid GPUI key sequences")
            })
        })
        .collect()
}
