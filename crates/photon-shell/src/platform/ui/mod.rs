//! GPUI views and window composition.

mod icons;
mod input;
mod omnibox;
pub(super) mod theme;
mod titlebar;
mod webview;
mod window;

pub(in crate::platform) use webview::PhotonWebView;
pub use window::run;
