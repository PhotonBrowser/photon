//! Framework-independent browser state and command rules.
//!
//! The native C ABI is implemented in the separate `photon-ffi` crate.

mod browsing_data;
mod commands;
mod crashes;
mod dialogs;
mod history;
mod history_days;
mod internal_pages;
mod new_tab;
mod omnibox;
mod session;
mod settings;
mod sidebar;
mod spaces;
mod state;
mod tab_sleep;
mod tabs;
mod workspace;

pub use browsing_data::ClearBrowsingData;
pub use commands::{
    COMMAND_REGISTRY, Command, CommandAvailability, CommandDefinition, CommandMatch,
    CommandRegistry, CommandRequirement,
};
pub use crashes::{CrashResponse, EngineService, PageCrashes};
pub use dialogs::{DialogKind, DialogReply, DialogRequest, PageDialogs};
pub use history::{HISTORY_MAX_PAGES, HISTORY_MAX_SEARCHES, History, HistoryEntry, SearchEntry};
pub use history_days::{day_heading, local_day, time_of_day};
pub use internal_pages::{internal_page_name, internal_page_url};
pub use new_tab::{MAX_NEW_TAB_PINS, NewTabSettings, SHORTCUT_COUNTS, Shortcut, site_name};
pub use omnibox::resolve as resolve_omnibox_input;
pub use omnibox::*;
pub use session::{Session, SessionTab, WindowSession};
pub use settings::{
    AnimationSettings, BACKGROUND_NOISE_INTENSITY_MAX, BrowserSettings, NewTabPosition,
    PopupPolicy, TabLayout, ThemeMode, Transparency, WindowBackgroundStyle, WindowColor,
};
pub use sidebar::{
    SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH, SidebarResize,
    SidebarSettings,
};
pub use spaces::{
    MAX_FAVOURITES, MAX_SPACE_COLORS, Space, SpaceColors, SpaceId, SpaceTint, Spaces,
};
pub use state::{BrowserCommand, BrowserState, EngineEvent, normalize_url};
pub use tab_sleep::{
    MemoryPressure, TabSleepMode, active_idle_timeout, idle_discard_delay, idle_timeout,
};
pub use tabs::{ARCHIVE_AFTER, should_archive};
pub use workspace::{
    ADDRESS_FIELD_COMPONENT_ID, ADDRESS_TOOLBAR_COMPONENT_ID, BACK_BUTTON_COMPONENT_ID,
    BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion, FAVOURITES_PANEL_COMPONENT_ID,
    FORWARD_BUTTON_COMPONENT_ID, HORIZONTAL_PRESET_ID, LayoutComponent, RELOAD_BUTTON_COMPONENT_ID,
    SIDEBAR_COMPONENT_ID, SIDEBAR_FOOTER_COMPONENT_ID, SIDEBAR_TOGGLE_COMPONENT_ID, SidebarSide,
    TAB_STRIP_COMPONENT_ID, VERTICAL_PRESET_ID, VERTICAL_TABS_COMPONENT_ID, WorkspaceLayout,
    WorkspacePreset, WorkspaceSettings,
};
