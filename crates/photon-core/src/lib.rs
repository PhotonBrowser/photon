//! Framework-independent browser state and command rules.
//!
//! The native C ABI is implemented in the separate `photon-ffi` crate.

mod browsing_data;
mod crashes;
mod dialogs;
mod history;
mod internal_pages;
mod new_tab;
mod settings;
mod state;

pub use browsing_data::ClearBrowsingData;
pub use crashes::{CrashResponse, EngineService, PageCrashes};
pub use dialogs::{DialogKind, DialogReply, DialogRequest, PageDialogs};
pub use history::{History, HistoryEntry, SearchEntry};
pub use internal_pages::{internal_page_name, internal_page_url};
pub use new_tab::{NewTabSettings, SHORTCUT_COUNTS, Shortcut, site_name};
pub use photon_omnibox::{
    OmniboxError, OmniboxTarget, SearchEngine, SearchEngineError, SearchEngines, Suggestion,
    SuggestionKind, Suggestions, UrlKind, resolve as resolve_omnibox_input,
};
pub use settings::{BrowserSettings, PopupPolicy, ThemeMode, Transparency};
pub use state::{BrowserCommand, BrowserState, EngineEvent, normalize_url};
