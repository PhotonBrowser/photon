//! Default key bindings for browser-level actions.

use gpui::KeyBinding;

use crate::{CloseTab, NewTab};

/// Returns the browser's default keyboard shortcuts.
pub fn browser_shortcuts() -> [KeyBinding; 2] {
    [
        KeyBinding::new("secondary-t", NewTab, None),
        KeyBinding::new("secondary-w", CloseTab, None),
    ]
}
