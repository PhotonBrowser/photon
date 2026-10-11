//! Whether the browser is the one macOS opens web links with, and asking
//! macOS to make it so.

use super::ffi::embedder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefaultBrowser {
    /// Running outside an app bundle, which macOS cannot make the default.
    Unavailable,
    NotDefault,
    Default,
}

impl DefaultBrowser {
    pub(crate) fn current() -> Self {
        if !unsafe { embedder::photon_default_browser_available() } {
            Self::Unavailable
        } else if unsafe { embedder::photon_default_browser_is_current() } {
            Self::Default
        } else {
            Self::NotDefault
        }
    }

    /// Asks macOS to open web links with this browser. macOS asks the person
    /// to confirm, so the change, if any, comes later.
    pub(crate) fn request() {
        unsafe { embedder::photon_default_browser_request() };
    }
}
