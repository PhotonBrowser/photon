//! Framework-independent browser state and command rules.
//!
//! The native C ABI is implemented in the separate `photon-ffi` crate.

mod browsing_data;
mod command_bar;
mod crashes;
mod dialogs;
mod history;
mod internal_pages;
mod new_tab;
mod settings;
mod sidebar;
mod state;
mod tabs;

pub use browsing_data::ClearBrowsingData;
pub use command_bar::{Command, CommandBarResult, OpenTab, command_bar_results};
pub use crashes::{CrashResponse, EngineService, PageCrashes};
pub use dialogs::{DialogKind, DialogReply, DialogRequest, PageDialogs};
pub use history::{History, HistoryEntry, SearchEntry};
pub use internal_pages::{internal_page_name, internal_page_url};
pub use new_tab::{NewTabSettings, SHORTCUT_COUNTS, Shortcut, site_name};
pub use photon_omnibox::{
    OmniboxError, OmniboxTarget, SearchEngine, SearchEngineError, SearchEngines, Suggestion,
    SuggestionKind, Suggestions, UrlKind, resolve as resolve_omnibox_input,
};
pub use settings::{BrowserSettings, PopupPolicy, TabLayout, ThemeMode, Transparency, WindowColor};
pub use sidebar::{
    MAX_FAVOURITES, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH, SidebarResize, SidebarSettings,
};
pub use state::{BrowserCommand, BrowserState, EngineEvent, normalize_url};
pub use tabs::{ARCHIVE_AFTER, pinned_after_drop, should_archive};
