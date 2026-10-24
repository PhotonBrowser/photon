//! Browser commands and shortcuts, collected once for menus, bindings and lists.

use gpui::Action;
use photon_core::Command;

use super::actions::*;

const GROUPS: [&str; 4] = ["Tabs", "Page", "Window", "App"];

#[derive(Clone, Copy)]
enum ShortcutLabel {
    Command(Command),
    Text(&'static str),
}

impl ShortcutLabel {
    fn name(self) -> &'static str {
        match self {
            Self::Command(command) => command.name(),
            Self::Text(name) => name,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ShortcutAction {
    Quit,
    HideApp,
    HideOtherApps,
    MinimizeWindow,
    NewTab,
    OpenCommandPalette,
    NewWindow,
    CloseTab,
    ReopenClosedTab,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectLastTab,
    Reload,
    StopLoading,
    GoBack,
    GoForward,
    FocusOmnibox,
    FindInPage,
    FindNext,
    FindPrevious,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    OpenSettings,
    ShowKeyboardShortcuts,
    ToggleSidebar,
    ToggleSidebarIcons,
    ToggleZenMode,
    NextSpace,
    PreviousSpace,
}

impl ShortcutAction {
    pub(super) fn action(self) -> Box<dyn Action> {
        match self {
            Self::Quit => Box::new(Quit),
            Self::HideApp => Box::new(HideApp),
            Self::HideOtherApps => Box::new(HideOtherApps),
            Self::MinimizeWindow => Box::new(MinimizeWindow),
            Self::NewTab => Box::new(NewTab),
            Self::OpenCommandPalette => Box::new(OpenCommandPalette),
            Self::NewWindow => Box::new(NewWindow),
            Self::CloseTab => Box::new(CloseTab),
            Self::ReopenClosedTab => Box::new(ReopenClosedTab),
            Self::SelectNextTab => Box::new(SelectNextTab),
            Self::SelectPreviousTab => Box::new(SelectPreviousTab),
            Self::SelectTab1 => Box::new(SelectTab1),
            Self::SelectTab2 => Box::new(SelectTab2),
            Self::SelectTab3 => Box::new(SelectTab3),
            Self::SelectTab4 => Box::new(SelectTab4),
            Self::SelectTab5 => Box::new(SelectTab5),
            Self::SelectTab6 => Box::new(SelectTab6),
            Self::SelectTab7 => Box::new(SelectTab7),
            Self::SelectTab8 => Box::new(SelectTab8),
            Self::SelectLastTab => Box::new(SelectLastTab),
            Self::Reload => Box::new(Reload),
            Self::StopLoading => Box::new(StopLoading),
            Self::GoBack => Box::new(GoBack),
            Self::GoForward => Box::new(GoForward),
            Self::FocusOmnibox => Box::new(FocusOmnibox),
            Self::FindInPage => Box::new(FindInPage),
            Self::FindNext => Box::new(FindNext),
            Self::FindPrevious => Box::new(FindPrevious),
            Self::ZoomIn => Box::new(ZoomIn),
            Self::ZoomOut => Box::new(ZoomOut),
            Self::ResetZoom => Box::new(ResetZoom),
            Self::OpenSettings => Box::new(OpenSettings),
            Self::ShowKeyboardShortcuts => Box::new(ShowKeyboardShortcuts),
            Self::ToggleSidebar => Box::new(ToggleSidebar),
            Self::ToggleSidebarIcons => Box::new(ToggleSidebarIcons),
            Self::ToggleZenMode => Box::new(ToggleZenMode),
            Self::NextSpace => Box::new(NextSpace),
            Self::PreviousSpace => Box::new(PreviousSpace),
        }
    }
}

pub(super) struct ShortcutSpec {
    pub(super) group: &'static str,
    label: ShortcutLabel,
    pub(super) keys: &'static [&'static str],
    pub(super) action: ShortcutAction,
}

const SHORTCUTS: &[ShortcutSpec] = &[
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Command(Command::NewTab),
        keys: &["secondary-t"],
        action: ShortcutAction::NewTab,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Command Palette"),
        keys: &["secondary-k"],
        action: ShortcutAction::OpenCommandPalette,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Command(Command::CloseTab),
        keys: &["secondary-w"],
        action: ShortcutAction::CloseTab,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Command(Command::ReopenClosedTab),
        keys: &["secondary-shift-t"],
        action: ShortcutAction::ReopenClosedTab,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Command(Command::SelectNextTab),
        keys: &["ctrl-tab", "secondary-shift-]"],
        action: ShortcutAction::SelectNextTab,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Command(Command::SelectPreviousTab),
        keys: &["ctrl-shift-tab", "secondary-shift-["],
        action: ShortcutAction::SelectPreviousTab,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("First Tab"),
        keys: &["secondary-1"],
        action: ShortcutAction::SelectTab1,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 2"),
        keys: &["secondary-2"],
        action: ShortcutAction::SelectTab2,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 3"),
        keys: &["secondary-3"],
        action: ShortcutAction::SelectTab3,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 4"),
        keys: &["secondary-4"],
        action: ShortcutAction::SelectTab4,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 5"),
        keys: &["secondary-5"],
        action: ShortcutAction::SelectTab5,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 6"),
        keys: &["secondary-6"],
        action: ShortcutAction::SelectTab6,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 7"),
        keys: &["secondary-7"],
        action: ShortcutAction::SelectTab7,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Select Tab 8"),
        keys: &["secondary-8"],
        action: ShortcutAction::SelectTab8,
    },
    ShortcutSpec {
        group: "Tabs",
        label: ShortcutLabel::Text("Last Tab"),
        keys: &["secondary-9"],
        action: ShortcutAction::SelectLastTab,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::FocusAddressField),
        keys: &["secondary-l"],
        action: ShortcutAction::FocusOmnibox,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::ReloadPage),
        keys: &["secondary-r"],
        action: ShortcutAction::Reload,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::StopLoading),
        keys: &["secondary-."],
        action: ShortcutAction::StopLoading,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::GoBack),
        keys: &["secondary-["],
        action: ShortcutAction::GoBack,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::GoForward),
        keys: &["secondary-]"],
        action: ShortcutAction::GoForward,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::FindInPage),
        keys: &["secondary-f"],
        action: ShortcutAction::FindInPage,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Text("Find Next"),
        keys: &["secondary-g"],
        action: ShortcutAction::FindNext,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Text("Find Previous"),
        keys: &["secondary-shift-g"],
        action: ShortcutAction::FindPrevious,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::ZoomIn),
        keys: &["secondary-=", "secondary-+"],
        action: ShortcutAction::ZoomIn,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::ZoomOut),
        keys: &["secondary--"],
        action: ShortcutAction::ZoomOut,
    },
    ShortcutSpec {
        group: "Page",
        label: ShortcutLabel::Command(Command::ResetZoom),
        keys: &["secondary-0"],
        action: ShortcutAction::ResetZoom,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::NewWindow),
        keys: &["secondary-n"],
        action: ShortcutAction::NewWindow,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Text("Minimize"),
        keys: &["secondary-m"],
        action: ShortcutAction::MinimizeWindow,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::ToggleSidebar),
        keys: &["secondary-s"],
        action: ShortcutAction::ToggleSidebar,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::ToggleSidebarIcons),
        keys: &["secondary-shift-s"],
        action: ShortcutAction::ToggleSidebarIcons,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::ToggleZenMode),
        keys: &["secondary-alt-z"],
        action: ShortcutAction::ToggleZenMode,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::NextSpace),
        keys: &["secondary-alt-right"],
        action: ShortcutAction::NextSpace,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::PreviousSpace),
        keys: &["secondary-alt-left"],
        action: ShortcutAction::PreviousSpace,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::OpenSettings),
        keys: &["secondary-,"],
        action: ShortcutAction::OpenSettings,
    },
    ShortcutSpec {
        group: "Window",
        label: ShortcutLabel::Command(Command::ShowKeyboardShortcuts),
        keys: &["secondary-/"],
        action: ShortcutAction::ShowKeyboardShortcuts,
    },
    ShortcutSpec {
        group: "App",
        label: ShortcutLabel::Text("Hide"),
        keys: &["secondary-h"],
        action: ShortcutAction::HideApp,
    },
    ShortcutSpec {
        group: "App",
        label: ShortcutLabel::Text("Hide Others"),
        keys: &["secondary-alt-h"],
        action: ShortcutAction::HideOtherApps,
    },
    ShortcutSpec {
        group: "App",
        label: ShortcutLabel::Text("Quit"),
        keys: &["secondary-q"],
        action: ShortcutAction::Quit,
    },
];

pub(super) fn shortcut_specs() -> &'static [ShortcutSpec] {
    SHORTCUTS
}

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

/// Returns the GPUI action used to run a command that has a shortcut.
pub fn action_for_command(command: Command) -> Option<Box<dyn Action>> {
    shortcut_specs()
        .iter()
        .find(
            |shortcut| matches!(shortcut.label, ShortcutLabel::Command(value) if value == command),
        )
        .map(|shortcut| shortcut.action.action())
}

/// Returns the registry's label for an action, for native menus and other
/// surfaces that display the same command.
#[cfg(target_os = "macos")]
pub fn shortcut_name_for_action(action: &dyn Action) -> Option<&'static str> {
    shortcut_specs()
        .iter()
        .find(|shortcut| shortcut.action.action().partial_eq(action))
        .map(|shortcut| shortcut.label.name())
}

/// Every browser shortcut worth listing, grouped by what it acts on.
pub fn shortcut_groups() -> Vec<ShortcutGroup> {
    GROUPS
        .into_iter()
        .map(|name| ShortcutGroup {
            name,
            entries: shortcut_specs()
                .iter()
                .filter(|shortcut| shortcut.group == name)
                .map(|shortcut| ShortcutEntry {
                    name: shortcut.label.name(),
                    action: shortcut.action.action(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::shortcuts::browser_shortcuts;

    #[test]
    fn every_listed_shortcut_has_a_binding() {
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

    #[test]
    fn registered_command_shortcuts_use_their_action() {
        let bindings = browser_shortcuts();
        for definition in photon_core::COMMAND_REGISTRY.builtins() {
            let command = definition.command;
            if let Some(action) = action_for_command(command) {
                assert!(
                    bindings
                        .iter()
                        .any(|binding| binding.action().partial_eq(action.as_ref())),
                    "{} is missing its registered keybinding",
                    command.name()
                );
            }
        }
    }
}
