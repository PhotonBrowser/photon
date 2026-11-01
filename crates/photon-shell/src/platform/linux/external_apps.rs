//! Opening links Photon cannot load, such as `mailto:`, in the desktop's
//! handler for their scheme, through the freedesktop `xdg-mime` and
//! `xdg-open` tools.

use std::process::{Command, Stdio};

/// The name of the app that opens `url`, or `None` when no app does.
pub(crate) fn app_for_url(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    let output = Command::new("xdg-mime")
        .args(["query", "default", &format!("x-scheme-handler/{scheme}")])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let desktop_entry = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let name = desktop_entry
        .strip_suffix(".desktop")
        .unwrap_or(&desktop_entry);
    // An entry is named for its app's reverse domain, as in org.gnome.Evolution.
    let name = name.rsplit('.').next().unwrap_or(name);
    (output.status.success() && !name.is_empty()).then(|| name.to_owned())
}

/// Opens `url` in its app, returning whether it could be handed over.
pub(crate) fn open_url(url: &str) -> bool {
    Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}
