//! Browser keyboard actions and their default key bindings.

mod actions;
mod bindings;
mod catalog;

pub use actions::*;
pub use bindings::browser_shortcuts;
pub use catalog::{
    ShortcutEntry, ShortcutGroup, action_for_command, shortcut_groups, shortcut_name_for_action,
};
