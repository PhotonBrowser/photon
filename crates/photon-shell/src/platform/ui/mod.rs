//! GPUI views and window composition.

mod crash_alert;
mod debug_overlay;
mod icons;
mod input;
mod layout;
mod menu;
mod omnibox;
mod tabs;
pub(super) mod theme;
mod titlebar;
mod toolbar;
mod webview;
mod window;

pub(in crate::platform) use webview::PhotonWebView;
pub use window::run;
