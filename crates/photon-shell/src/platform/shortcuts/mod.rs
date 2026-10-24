//! Browser keyboard actions, their bindings, and shortcut catalog.

mod actions;
mod bindings;
mod catalog;

pub use actions::*;
pub use bindings::browser_shortcuts;
#[cfg(target_os = "macos")]
pub use catalog::shortcut_name_for_action;
pub use catalog::{ShortcutGroup, action_for_command, shortcut_groups};
