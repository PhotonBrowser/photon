//! GPUI views and window composition.

mod webview;
mod window;

pub(in crate::platform) use webview::PhotonWebView;
pub use window::run;
