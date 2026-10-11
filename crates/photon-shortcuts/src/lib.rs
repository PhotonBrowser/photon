//! Browser keyboard actions and their default key bindings.

mod actions;
mod bindings;
mod catalog;

pub use actions::*;
pub use bindings::browser_shortcuts;
pub use catalog::{ShortcutEntry, ShortcutGroup, shortcut_groups};
