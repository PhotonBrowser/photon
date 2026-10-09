//! C-compatible browser API for native Photon adapters.
//!
//! Browser rules and state live in `photon-core`; this crate owns pointer
//! validation, string conversion, and the exported C ABI.

mod api;

pub use api::{
    BrowserHandle, photon_browser_cancel_navigation, photon_browser_command, photon_browser_create,
    photon_browser_destroy, photon_browser_error, photon_browser_flag,
    photon_browser_frame_presented, photon_browser_load_failed, photon_browser_navigate,
    photon_browser_navigation_started, photon_browser_string, photon_browser_update,
};
