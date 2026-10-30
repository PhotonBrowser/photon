//! Linux default-browser integration through `xdg-settings`.
//!
//! Photon can only become the default once a desktop entry named
//! [`photon_brand::LINUX_DESKTOP_ID`] is installed, so development builds
//! without one report the integration as unavailable.

use std::{env, path::PathBuf, process::Command};

use photon_brand::LINUX_DESKTOP_ID;

use crate::platform::DefaultBrowserStatus;

const DEFAULT_BROWSER_SETTING: &str = "default-web-browser";

pub(crate) fn status() -> DefaultBrowserStatus {
    if !desktop_entry_installed() {
        return DefaultBrowserStatus::Unavailable;
    }
    let Ok(output) = Command::new("xdg-settings")
        .args(["get", DEFAULT_BROWSER_SETTING])
        .output()
    else {
        return DefaultBrowserStatus::Unavailable;
    };
    if !output.status.success() {
        DefaultBrowserStatus::Unavailable
    } else if String::from_utf8_lossy(&output.stdout).trim() == LINUX_DESKTOP_ID {
        DefaultBrowserStatus::Default
    } else {
        DefaultBrowserStatus::NotDefault
    }
}

/// Asks the desktop to open web links with Photon.
pub(crate) fn request() {
    let _ = Command::new("xdg-settings")
        .args(["set", DEFAULT_BROWSER_SETTING, LINUX_DESKTOP_ID])
        .status();
}

/// Whether Photon's desktop entry is in an XDG application directory.
fn desktop_entry_installed() -> bool {
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs = env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(data_dirs.split(':').map(PathBuf::from))
        .any(|dir| dir.join("applications").join(LINUX_DESKTOP_ID).is_file())
}
