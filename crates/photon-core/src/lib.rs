//! Framework-independent browser state, commands, and the native C API.
//!
//! `state` contains the Rust-facing browser model. `api` adapts that model to
//! the narrow C ABI used by the native shell.

mod api;
mod state;

pub use api::{
    BrowserHandle, photon_browser_cancel_navigation, photon_browser_command, photon_browser_create,
    photon_browser_destroy, photon_browser_error, photon_browser_flag,
    photon_browser_frame_presented, photon_browser_load_failed, photon_browser_navigate,
    photon_browser_navigation_started, photon_browser_string, photon_browser_update,
};
pub use photon_omnibox::{
    OmniboxError, OmniboxTarget, SearchEngine, SearchEngineError, SearchEngines, UrlKind,
    resolve as resolve_omnibox_input,
};
pub use state::{BrowserCommand, BrowserDiagnostics, BrowserState, EngineEvent, normalize_url};
