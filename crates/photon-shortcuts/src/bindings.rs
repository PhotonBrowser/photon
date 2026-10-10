//! Default key bindings for browser-level actions.

use gpui::KeyBinding;

use crate::actions::*;

/// Returns the browser's default keyboard shortcuts.
pub fn browser_shortcuts() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("secondary-t", NewTab, None),
        KeyBinding::new("secondary-n", NewWindow, None),
        KeyBinding::new("secondary-w", CloseTab, None),
        KeyBinding::new("secondary-shift-t", ReopenClosedTab, None),
        KeyBinding::new("ctrl-tab", SelectNextTab, None),
        KeyBinding::new("ctrl-shift-tab", SelectPreviousTab, None),
        KeyBinding::new("secondary-shift-]", SelectNextTab, None),
        KeyBinding::new("secondary-shift-[", SelectPreviousTab, None),
        KeyBinding::new("secondary-1", SelectTab1, None),
        KeyBinding::new("secondary-2", SelectTab2, None),
        KeyBinding::new("secondary-3", SelectTab3, None),
        KeyBinding::new("secondary-4", SelectTab4, None),
        KeyBinding::new("secondary-5", SelectTab5, None),
        KeyBinding::new("secondary-6", SelectTab6, None),
        KeyBinding::new("secondary-7", SelectTab7, None),
        KeyBinding::new("secondary-8", SelectTab8, None),
        KeyBinding::new("secondary-9", SelectLastTab, None),
        KeyBinding::new("secondary-r", Reload, None),
        KeyBinding::new("secondary-.", StopLoading, None),
        KeyBinding::new("secondary-[", GoBack, None),
        KeyBinding::new("secondary-]", GoForward, None),
        KeyBinding::new("secondary-l", FocusOmnibox, None),
        KeyBinding::new("secondary-f", FindInPage, None),
        KeyBinding::new("secondary-g", FindNext, None),
        KeyBinding::new("secondary-shift-g", FindPrevious, None),
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary-+", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ResetZoom, None),
        KeyBinding::new("secondary-s", ToggleSidebar, None),
    ]
}
