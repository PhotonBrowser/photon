//! The browser's command catalog, search aliases, and availability rules.

use std::ops::Range;

use crate::omnibox::match_words;

/// A command the browser window can execute.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Command {
    NewTab,
    NewWindow,
    ReopenClosedTab,
    CloseTab,
    ReloadPage,
    StopLoading,
    GoBack,
    GoForward,
    FindInPage,
    CopyAddress,
    FocusAddressField,
    SelectNextTab,
    SelectPreviousTab,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleSidebar,
    ToggleSidebarIcons,
    ToggleZenMode,
    NewSpace,
    NextSpace,
    PreviousSpace,
    OpenSettings,
    OpenHistory,
    OpenDownloads,
    OpenTaskManager,
    ShowKeyboardShortcuts,
}

/// The capability a command needs in the current browser window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRequirement {
    Always,
    HasPage,
    HasMultipleTabs,
    HasClosedTabs,
    CanGoBack,
    CanGoForward,
    IsLoading,
    HasSidebar,
    ShowsSpaces,
    HasNextSpace,
    HasPreviousSpace,
}

/// Shared metadata for a browser command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandDefinition {
    pub command: Command,
    pub name: &'static str,
    /// Additional words users may search for to find this command.
    pub keywords: &'static str,
    pub requirement: CommandRequirement,
}

/// A registered command and the ranges to emphasize in its name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMatch {
    pub command: Command,
    pub name_matches: Vec<Range<usize>>,
}

/// The built-in command registry. Add metadata here when adding a command;
/// the shell's exhaustive dispatcher then requires an implementation.
const BUILT_IN_COMMANDS: &[CommandDefinition] = &[
    CommandDefinition {
        command: Command::NewTab,
        name: "New Tab",
        keywords: "blank page",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::NewWindow,
        name: "New Window",
        keywords: "",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::ReopenClosedTab,
        name: "Reopen Closed Tab",
        keywords: "restore undo",
        requirement: CommandRequirement::HasClosedTabs,
    },
    CommandDefinition {
        command: Command::CloseTab,
        name: "Close Tab",
        keywords: "",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::ReloadPage,
        name: "Reload Page",
        keywords: "refresh restart",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::StopLoading,
        name: "Stop Loading",
        keywords: "cancel load",
        requirement: CommandRequirement::IsLoading,
    },
    CommandDefinition {
        command: Command::GoBack,
        name: "Go Back",
        keywords: "previous history",
        requirement: CommandRequirement::CanGoBack,
    },
    CommandDefinition {
        command: Command::GoForward,
        name: "Go Forward",
        keywords: "next history",
        requirement: CommandRequirement::CanGoForward,
    },
    CommandDefinition {
        command: Command::FindInPage,
        name: "Find in Page",
        keywords: "search text",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::CopyAddress,
        name: "Copy Address",
        keywords: "url link location",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::FocusAddressField,
        name: "Focus Address Field",
        keywords: "address bar omnibox url search",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::SelectNextTab,
        name: "Next Tab",
        keywords: "switch right",
        requirement: CommandRequirement::HasMultipleTabs,
    },
    CommandDefinition {
        command: Command::SelectPreviousTab,
        name: "Previous Tab",
        keywords: "switch left",
        requirement: CommandRequirement::HasMultipleTabs,
    },
    CommandDefinition {
        command: Command::ZoomIn,
        name: "Zoom In",
        keywords: "larger increase",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::ZoomOut,
        name: "Zoom Out",
        keywords: "smaller decrease",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::ResetZoom,
        name: "Reset Zoom",
        keywords: "actual size default",
        requirement: CommandRequirement::HasPage,
    },
    CommandDefinition {
        command: Command::ToggleSidebar,
        name: "Show or Hide Sidebar",
        keywords: "tabs",
        requirement: CommandRequirement::HasSidebar,
    },
    CommandDefinition {
        command: Command::ToggleSidebarIcons,
        name: "Collapse or Expand Sidebar",
        keywords: "icons compact",
        requirement: CommandRequirement::HasSidebar,
    },
    CommandDefinition {
        command: Command::ToggleZenMode,
        name: "Zen Mode",
        keywords: "focus distraction free fullscreen hide chrome",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::NewSpace,
        name: "New Space",
        keywords: "create add workspace",
        requirement: CommandRequirement::ShowsSpaces,
    },
    CommandDefinition {
        command: Command::NextSpace,
        name: "Next Space",
        keywords: "switch right",
        requirement: CommandRequirement::HasNextSpace,
    },
    CommandDefinition {
        command: Command::PreviousSpace,
        name: "Previous Space",
        keywords: "switch left",
        requirement: CommandRequirement::HasPreviousSpace,
    },
    CommandDefinition {
        command: Command::OpenSettings,
        name: "Settings",
        keywords: "preferences options",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::OpenHistory,
        name: "History",
        keywords: "visited pages past browsing",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::OpenDownloads,
        name: "Downloads",
        keywords: "files saved",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::OpenTaskManager,
        name: "Task Manager",
        keywords: "processes cpu memory ram disk network performance",
        requirement: CommandRequirement::Always,
    },
    CommandDefinition {
        command: Command::ShowKeyboardShortcuts,
        name: "Keyboard Shortcuts",
        keywords: "keybindings hotkeys keys help",
        requirement: CommandRequirement::Always,
    },
];

/// Access to the browser's built-in command definitions.
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandRegistry;

impl CommandRegistry {
    /// Every built-in command, in its stable catalog order.
    pub const fn builtins(self) -> &'static [CommandDefinition] {
        BUILT_IN_COMMANDS
    }

    /// The registered definition for `command`.
    pub fn definition(self, command: Command) -> &'static CommandDefinition {
        BUILT_IN_COMMANDS
            .iter()
            .find(|definition| definition.command == command)
            .expect("every browser command must have a registry definition")
    }

    /// Available commands matching every query word at the start of a name or
    /// keyword. An empty query returns all commands allowed in this context.
    pub fn search(self, query: &str, availability: CommandAvailability) -> Vec<CommandMatch> {
        let query = query.trim();
        self.builtins()
            .iter()
            .filter(|definition| availability.allows(definition.command))
            .filter_map(|definition| {
                let name_matches = match_words(definition.name, query);
                let keyword_matches = match_words(definition.keywords, query);
                if query.is_empty() || name_matches.is_some() || keyword_matches.is_some() {
                    Some(CommandMatch {
                        command: definition.command,
                        name_matches: name_matches.unwrap_or_default(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

/// The process-wide built-in browser command registry.
pub const COMMAND_REGISTRY: CommandRegistry = CommandRegistry;

impl Command {
    /// The display name shared by menus and the command palette.
    pub fn name(self) -> &'static str {
        COMMAND_REGISTRY.definition(self).name
    }
}

/// Browser state that determines which commands can run in a window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandAvailability {
    pub has_page: bool,
    pub has_multiple_tabs: bool,
    pub has_closed_tabs: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub is_loading: bool,
    pub has_sidebar: bool,
    pub shows_spaces: bool,
    pub has_next_space: bool,
    pub has_previous_space: bool,
}

impl CommandAvailability {
    /// Whether `command` can run with these capabilities.
    pub fn allows(self, command: Command) -> bool {
        match COMMAND_REGISTRY.definition(command).requirement {
            CommandRequirement::Always => true,
            CommandRequirement::HasPage => self.has_page,
            CommandRequirement::HasMultipleTabs => self.has_multiple_tabs,
            CommandRequirement::HasClosedTabs => self.has_closed_tabs,
            CommandRequirement::CanGoBack => self.can_go_back,
            CommandRequirement::CanGoForward => self.can_go_forward,
            CommandRequirement::IsLoading => self.is_loading,
            CommandRequirement::HasSidebar => self.has_sidebar,
            CommandRequirement::ShowsSpaces => self.shows_spaces,
            CommandRequirement::HasNextSpace => self.shows_spaces && self.has_next_space,
            CommandRequirement::HasPreviousSpace => self.shows_spaces && self.has_previous_space,
        }
    }

    /// Registered commands that can run with these capabilities.
    pub fn commands(self) -> impl Iterator<Item = Command> {
        COMMAND_REGISTRY
            .builtins()
            .iter()
            .map(|definition| definition.command)
            .filter(move |command| self.allows(*command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_definitions_are_unique_and_cover_every_command() {
        let commands: HashSet<_> = COMMAND_REGISTRY
            .builtins()
            .iter()
            .map(|definition| definition.command)
            .collect();

        assert_eq!(commands.len(), COMMAND_REGISTRY.builtins().len());
        for command in [
            Command::NewTab,
            Command::NewWindow,
            Command::ReopenClosedTab,
            Command::CloseTab,
            Command::ReloadPage,
            Command::StopLoading,
            Command::GoBack,
            Command::GoForward,
            Command::FindInPage,
            Command::CopyAddress,
            Command::FocusAddressField,
            Command::SelectNextTab,
            Command::SelectPreviousTab,
            Command::ZoomIn,
            Command::ZoomOut,
            Command::ResetZoom,
            Command::ToggleSidebar,
            Command::ToggleSidebarIcons,
            Command::ToggleZenMode,
            Command::NewSpace,
            Command::NextSpace,
            Command::PreviousSpace,
            Command::OpenSettings,
            Command::OpenHistory,
            Command::OpenDownloads,
            Command::ShowKeyboardShortcuts,
        ] {
            assert!(commands.contains(&command), "{command:?} is not registered");
        }
    }

    #[test]
    fn availability_uses_registered_requirements() {
        let availability = CommandAvailability {
            has_page: true,
            has_multiple_tabs: true,
            has_closed_tabs: true,
            can_go_back: true,
            can_go_forward: false,
            is_loading: false,
            has_sidebar: true,
            shows_spaces: true,
            has_next_space: true,
            has_previous_space: false,
        };

        assert!(availability.allows(Command::ReloadPage));
        assert!(availability.allows(Command::ReopenClosedTab));
        assert!(availability.allows(Command::NewSpace));
        assert!(availability.allows(Command::GoBack));
        assert!(!availability.allows(Command::GoForward));
        assert!(availability.allows(Command::SelectNextTab));
        assert!(availability.allows(Command::NextSpace));
        assert!(!availability.allows(Command::PreviousSpace));
    }

    #[test]
    fn registry_search_matches_names_and_aliases() {
        let availability = CommandAvailability {
            has_page: true,
            ..CommandAvailability::default()
        };
        let matches = COMMAND_REGISTRY.search("refresh", availability);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].command, Command::ReloadPage);

        let matches = COMMAND_REGISTRY.search("address", availability);
        assert!(
            matches
                .iter()
                .any(|item| item.command == Command::FocusAddressField)
        );
    }
}
