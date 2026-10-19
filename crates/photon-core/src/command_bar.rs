//! What the command bar offers for the typed text: the address or search
//! the omnibox would open, open tabs to switch to, and browser commands.

use std::ops::Range;

use photon_omnibox::{Suggestion, match_words};

/// How many open tabs the command bar offers at most.
const MAX_TABS: usize = 4;
/// How many commands the command bar offers at most.
const MAX_COMMANDS: usize = 3;
/// How many rows the command bar shows at most.
const MAX_RESULTS: usize = 10;

/// A command the command bar can run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    NewTab,
    NewWindow,
    ReopenClosedTab,
    CloseTab,
    ReloadPage,
    FindInPage,
    CopyAddress,
    ToggleSidebar,
    ToggleSidebarIcons,
    ToggleZenMode,
    NewSpace,
    NextSpace,
    PreviousSpace,
    OpenSettings,
    ShowKeyboardShortcuts,
}

impl Command {
    /// Every command, in the order offered when several match.
    pub const ALL: [Self; 15] = [
        Self::NewTab,
        Self::NewWindow,
        Self::ReopenClosedTab,
        Self::CloseTab,
        Self::ReloadPage,
        Self::FindInPage,
        Self::CopyAddress,
        Self::ToggleSidebar,
        Self::ToggleSidebarIcons,
        Self::ToggleZenMode,
        Self::NewSpace,
        Self::NextSpace,
        Self::PreviousSpace,
        Self::OpenSettings,
        Self::ShowKeyboardShortcuts,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NewTab => "New Tab",
            Self::NewWindow => "New Window",
            Self::ReopenClosedTab => "Reopen Closed Tab",
            Self::CloseTab => "Close Tab",
            Self::ReloadPage => "Reload Page",
            Self::FindInPage => "Find in Page",
            Self::CopyAddress => "Copy Address",
            Self::ToggleSidebar => "Show or Hide Sidebar",
            Self::ToggleSidebarIcons => "Collapse or Expand Sidebar",
            Self::ToggleZenMode => "Zen Mode",
            Self::NewSpace => "New Space",
            Self::NextSpace => "Next Space",
            Self::PreviousSpace => "Previous Space",
            Self::OpenSettings => "Settings",
            Self::ShowKeyboardShortcuts => "Keyboard Shortcuts",
        }
    }

    /// Other words that find the command.
    fn keywords(self) -> &'static str {
        match self {
            Self::NewTab => "blank",
            Self::NewWindow => "",
            Self::ReopenClosedTab => "undo restore",
            Self::CloseTab => "",
            Self::ReloadPage => "refresh",
            Self::FindInPage => "search text",
            Self::CopyAddress => "url link",
            Self::ToggleSidebar => "tabs",
            Self::ToggleSidebarIcons => "icons compact tabs",
            Self::ToggleZenMode => "focus distraction free fullscreen hide chrome",
            Self::NewSpace => "create add",
            Self::NextSpace => "switch right",
            Self::PreviousSpace => "switch left back",
            Self::OpenSettings => "preferences options",
            Self::ShowKeyboardShortcuts => "keybindings hotkeys keys help",
        }
    }
}

/// Browser state that determines which commands are useful right now.
///
/// The shell supplies these capabilities from its current window and layout;
/// the command bar and its owner use the same rules to filter and revalidate
/// command selections.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommandAvailability {
    pub has_page: bool,
    pub has_closed_tabs: bool,
    pub has_sidebar: bool,
    pub shows_spaces: bool,
    pub has_next_space: bool,
    pub has_previous_space: bool,
}

impl CommandAvailability {
    /// Whether `command` can run with these capabilities.
    pub fn allows(self, command: Command) -> bool {
        match command {
            Command::ReloadPage | Command::FindInPage | Command::CopyAddress => self.has_page,
            Command::ReopenClosedTab => self.has_closed_tabs,
            Command::ToggleSidebar | Command::ToggleSidebarIcons => self.has_sidebar,
            Command::NewSpace => self.shows_spaces,
            Command::NextSpace => self.shows_spaces && self.has_next_space,
            Command::PreviousSpace => self.shows_spaces && self.has_previous_space,
            _ => true,
        }
    }

    /// Available commands in their normal command bar order.
    pub fn commands(self) -> impl Iterator<Item = Command> {
        Command::ALL
            .into_iter()
            .filter(move |command| self.allows(*command))
    }
}

/// An open tab the command bar can switch to.
#[derive(Clone, Copy, Debug)]
pub struct OpenTab<'a> {
    pub index: usize,
    pub title: &'a str,
    pub url: &'a str,
}

/// One row of the command bar.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandBarResult {
    /// An address, search or visited page, as the omnibox suggests it.
    Open(Suggestion),
    /// Switch to the open tab at `tab`.
    SwitchToTab {
        tab: usize,
        title: String,
        url: String,
        title_matches: Vec<Range<usize>>,
    },
    Run {
        command: Command,
        name_matches: Vec<Range<usize>>,
    },
}

/// The command bar's rows for `query`. With nothing typed, the open tabs.
/// Otherwise what Enter would open in the omnibox first, then matching
/// tabs and those of `commands` that match, then the rest of the omnibox's
/// suggestions.
pub fn command_bar_results(
    query: &str,
    tabs: &[OpenTab],
    commands: &[Command],
    suggestions: Vec<Suggestion>,
) -> Vec<CommandBarResult> {
    let query = query.trim();
    let tabs = tabs.iter().filter_map(|tab| {
        let title_matches = match_words(tab.title, query);
        let url_matches = match_words(tab.url, query);
        (title_matches.is_some() || url_matches.is_some()).then(|| CommandBarResult::SwitchToTab {
            tab: tab.index,
            title: tab.title.to_owned(),
            url: tab.url.to_owned(),
            title_matches: title_matches.unwrap_or_default(),
        })
    });
    if query.is_empty() {
        return tabs.take(MAX_RESULTS).collect();
    }
    let commands = commands.iter().copied().filter_map(|command| {
        let name_matches = match_words(command.name(), query);
        let keyword_matches = match_words(command.keywords(), query);
        (name_matches.is_some() || keyword_matches.is_some()).then(|| CommandBarResult::Run {
            command,
            name_matches: name_matches.unwrap_or_default(),
        })
    });
    let mut suggestions = suggestions.into_iter().map(CommandBarResult::Open);
    suggestions
        .next()
        .into_iter()
        .chain(tabs.take(MAX_TABS))
        .chain(commands.take(MAX_COMMANDS))
        .chain(suggestions)
        .take(MAX_RESULTS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use photon_omnibox::SuggestionKind;

    const TABS: [OpenTab; 2] = [
        OpenTab {
            index: 0,
            title: "Rust Programming Language",
            url: "https://www.rust-lang.org/",
        },
        OpenTab {
            index: 1,
            title: "Example Domain",
            url: "https://example.com/",
        },
    ];

    fn search(text: &str) -> Suggestion {
        Suggestion {
            kind: SuggestionKind::Search {
                engine: "Google".to_owned(),
            },
            text: text.to_owned(),
            text_matches: Vec::new(),
            url: format!("https://www.google.com/search?q={text}"),
        }
    }

    #[test]
    fn offers_open_tabs_before_typing() {
        let results = command_bar_results("", &TABS, &Command::ALL, Vec::new());
        assert_eq!(results.len(), 2);
        assert!(matches!(
            results[0],
            CommandBarResult::SwitchToTab { tab: 0, .. }
        ));
    }

    #[test]
    fn keeps_what_enter_opens_first() {
        let results = command_bar_results("rust", &TABS, &Command::ALL, vec![search("rust")]);
        assert!(matches!(&results[0], CommandBarResult::Open(_)));
        assert!(matches!(
            results[1],
            CommandBarResult::SwitchToTab { tab: 0, .. }
        ));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn finds_commands_by_name_and_keyword() {
        let results = command_bar_results("refresh", &TABS, &Command::ALL, vec![search("refresh")]);
        assert!(results.contains(&CommandBarResult::Run {
            command: Command::ReloadPage,
            name_matches: Vec::new(),
        }));
        let results = command_bar_results("find", &TABS, &Command::ALL, vec![search("find")]);
        assert!(results.contains(&CommandBarResult::Run {
            command: Command::FindInPage,
            name_matches: std::iter::once(0..4).collect(),
        }));
    }

    #[test]
    fn offers_only_the_given_commands() {
        let results = command_bar_results("sidebar", &TABS, &[Command::NewTab], Vec::new());
        assert!(results.is_empty());
    }

    #[test]
    fn command_availability_updates_rows_while_the_bar_is_open() {
        let mut availability = CommandAvailability {
            has_page: true,
            has_closed_tabs: true,
            has_sidebar: true,
            shows_spaces: true,
            has_next_space: true,
            has_previous_space: true,
        };
        let query = "reload";

        let results = command_bar_results(
            query,
            &[],
            &availability.commands().collect::<Vec<_>>(),
            Vec::new(),
        );
        assert!(results.iter().any(|result| matches!(
            result,
            CommandBarResult::Run {
                command: Command::ReloadPage,
                ..
            }
        )));

        availability.has_page = false;
        let results = command_bar_results(
            query,
            &[],
            &availability.commands().collect::<Vec<_>>(),
            Vec::new(),
        );
        assert!(!results.iter().any(|result| matches!(
            result,
            CommandBarResult::Run {
                command: Command::ReloadPage,
                ..
            }
        )));
    }

    #[test]
    fn matches_tabs_by_address() {
        let results = command_bar_results("example", &TABS, &Command::ALL, Vec::new());
        assert!(matches!(
            results[0],
            CommandBarResult::SwitchToTab { tab: 1, .. }
        ));
    }
}
