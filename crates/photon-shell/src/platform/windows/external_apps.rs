//! Opening links Photon cannot load, such as `mailto:`, in the app Windows
//! registers for their scheme.

use std::process::{Command, Stdio};

/// The app that opens `url`. Windows names it only through the registry, so
/// the prompt names it generically; opening reports when there is none.
pub(crate) fn app_for_url(_url: &str) -> Option<String> {
    Some("another app".to_owned())
}

/// Opens `url` in its app, returning whether it could be handed over. The URL
/// handler is called directly rather than through `cmd`, which would read
/// characters such as `&` in the URL as commands.
pub(crate) fn open_url(url: &str) -> bool {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}
