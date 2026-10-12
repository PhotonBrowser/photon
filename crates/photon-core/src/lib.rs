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
mod session;
mod settings;
mod sidebar;
mod spaces;
mod state;
mod tabs;
mod workspace;

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
pub use session::{Session, SessionTab, WindowSession};
pub use settings::{
    AnimationSettings, BACKGROUND_NOISE_INTENSITY_MAX, BrowserSettings, PopupPolicy, TabLayout,
    ThemeMode, Transparency, WindowBackgroundStyle, WindowColor,
};
pub use sidebar::{
    SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH, SidebarResize,
    SidebarSettings,
};
pub use spaces::{MAX_FAVOURITES, Space, SpaceId, Spaces};
pub use state::{BrowserCommand, BrowserState, EngineEvent, normalize_url};
pub use tabs::{ARCHIVE_AFTER, should_archive};
pub use workspace::{
    ADDRESS_FIELD_COMPONENT_ID, ADDRESS_TOOLBAR_COMPONENT_ID, BACK_BUTTON_COMPONENT_ID,
    BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion, FAVOURITES_PANEL_COMPONENT_ID,
    FORWARD_BUTTON_COMPONENT_ID, HORIZONTAL_PRESET_ID, LayoutComponent, RELOAD_BUTTON_COMPONENT_ID,
    SIDEBAR_COMPONENT_ID, SIDEBAR_FOOTER_COMPONENT_ID, SIDEBAR_TOGGLE_COMPONENT_ID, SidebarSide,
    TAB_STRIP_COMPONENT_ID, VERTICAL_PRESET_ID, VERTICAL_TABS_COMPONENT_ID, WorkspaceLayout,
    WorkspacePreset, WorkspaceSettings,
};
