//! macOS default-browser system integration.

use crate::platform::DefaultBrowserStatus;
use crate::platform::engine_ffi::embedder;

pub(crate) fn status() -> DefaultBrowserStatus {
    if !unsafe { embedder::photon_default_browser_available() } {
        DefaultBrowserStatus::Unavailable
    } else if unsafe { embedder::photon_default_browser_is_current() } {
        DefaultBrowserStatus::Default
    } else {
        DefaultBrowserStatus::NotDefault
    }
}

/// Asks macOS to open web links with Photon. The system confirms the change.
pub(crate) fn request() {
    unsafe { embedder::photon_default_browser_request() };
}
