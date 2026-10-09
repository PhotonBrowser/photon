//! Framework-independent browser state and command rules.
//!
//! The native C ABI is implemented in the separate `photon-ffi` crate.

mod state;

pub use photon_omnibox::{
    OmniboxError, OmniboxTarget, SearchEngine, SearchEngineError, SearchEngines, UrlKind,
    resolve as resolve_omnibox_input,
};
pub use state::{BrowserCommand, BrowserState, EngineEvent, normalize_url};
