//! Opening links Photon cannot load, such as `mailto:`, in the app macOS
//! chooses for them.

use objc2::rc::Retained;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSString, NSURL};

fn ns_url(url: &str) -> Option<Retained<NSURL>> {
    NSURL::URLWithString(&NSString::from_str(url))
}

/// The name of the app that opens `url`, or `None` when no app does.
pub(crate) fn app_for_url(url: &str) -> Option<String> {
    let url = ns_url(url)?;
    let app = NSWorkspace::sharedWorkspace().URLForApplicationToOpenURL(&url)?;
    let name = app.URLByDeletingPathExtension()?.lastPathComponent()?;
    Some(name.to_string())
}

/// Opens `url` in its app, returning whether it could.
pub(crate) fn open_url(url: &str) -> bool {
    ns_url(url).is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url))
}
