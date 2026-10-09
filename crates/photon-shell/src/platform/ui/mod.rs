//! GPUI views and window composition.

mod crash_alert;
mod icons;
mod input;
mod layout;
mod menu;
pub(super) mod metrics;
mod omnibox;
mod tabs;
pub(super) mod theme;
mod titlebar;
mod toolbar;
mod webview;
mod window;

pub(in crate::platform) use webview::{Favicon, PhotonWebView, WebViewEvent};
pub use window::run;
