//! Windows default-browser integration placeholder.

use crate::platform::DefaultBrowserStatus;

pub(crate) fn status() -> DefaultBrowserStatus {
    DefaultBrowserStatus::Unavailable
}

pub(crate) fn request() {}
