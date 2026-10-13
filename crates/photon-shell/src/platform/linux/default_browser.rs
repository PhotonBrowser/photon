//! Linux default-browser integration placeholder.
//!
//! Connect this adapter to the desktop portal or supported desktop services.

use crate::platform::DefaultBrowserStatus;

pub(crate) fn status() -> DefaultBrowserStatus {
    DefaultBrowserStatus::Unavailable
}

pub(crate) fn request() {}
