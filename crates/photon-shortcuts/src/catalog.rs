//! The browser's shortcuts by name and group, for listing them. Each names
//! its action rather than its keys, so a list always shows the bindings in
//! effect.

use gpui::Action;

use crate::actions::*;

/// Shortcuts that belong together, such as those for tabs.
pub struct ShortcutGroup {
    pub name: &'static str,
    pub entries: Vec<ShortcutEntry>,
}

/// What a shortcut does, and the action its keys dispatch.
pub struct ShortcutEntry {
    pub name: &'static str,
    pub action: Box<dyn Action>,
}

fn entry(name: &'static str, action: impl Action) -> ShortcutEntry {
    ShortcutEntry {
        name,
        action: Box::new(action),
    }
}

/// Every browser shortcut worth listing, grouped by what it acts on.
pub fn shortcut_groups() -> Vec<ShortcutGroup> {
    vec![
        ShortcutGroup {
            name: "Tabs",
            entries: vec![
                entry("Open Command Bar", OpenCommandBar),
                entry("Close Tab", CloseTab),
                entry("Reopen Closed Tab", ReopenClosedTab),
                entry("Next Tab", SelectNextTab),
                entry("Previous Tab", SelectPreviousTab),
                entry("First Tab", SelectTab1),
                entry("Last Tab", SelectLastTab),
            ],
        },
        ShortcutGroup {
            name: "Page",
            entries: vec![
                entry("Focus Address Field", FocusOmnibox),
                entry("Reload", Reload),
                entry("Stop Loading", StopLoading),
                entry("Back", GoBack),
                entry("Forward", GoForward),
                entry("Find in Page", FindInPage),
                entry("Find Next", FindNext),
                entry("Find Previous", FindPrevious),
                entry("Zoom In", ZoomIn),
                entry("Zoom Out", ZoomOut),
                entry("Actual Size", ResetZoom),
            ],
        },
        ShortcutGroup {
            name: "Window",
            entries: vec![
                entry("New Window", NewWindow),
                entry("Minimize", MinimizeWindow),
                entry("Show or Hide Sidebar", ToggleSidebar),
                entry("Sidebar as Icons", ToggleSidebarIcons),
                entry("Zen Mode", ToggleZenMode),
                entry("Next Space", NextSpace),
                entry("Previous Space", PreviousSpace),
                entry("Settings", OpenSettings),
                entry("Keyboard Shortcuts", ShowKeyboardShortcuts),
            ],
        },
        ShortcutGroup {
            name: "App",
            entries: vec![
                entry("Hide", HideApp),
                entry("Hide Others", HideOtherApps),
                entry("Quit", Quit),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_shortcuts;

    #[test]
    fn every_listed_shortcut_has_keys() {
        let bindings = browser_shortcuts();
        for group in shortcut_groups() {
            for entry in group.entries {
                assert!(
                    bindings
                        .iter()
                        .any(|binding| binding.action().partial_eq(entry.action.as_ref())),
                    "{} has no key binding",
                    entry.name
                );
            }
        }
    }
}
