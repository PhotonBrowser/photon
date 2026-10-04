//! GPUI views and window composition.

pub(super) mod theme;
mod webview;
mod window;

pub(in crate::platform) use webview::PhotonWebView;
pub use window::run;
