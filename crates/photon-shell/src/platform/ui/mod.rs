//! GPUI views and window composition.

mod button;
mod crash_alert;
mod icons;
mod input;
mod js_dialog;
mod layout;
mod menu;
pub(super) mod metrics;
mod modal;
mod omnibox;
mod tabs;
pub(super) mod theme;
mod titlebar;
mod toolbar;
mod webview;
mod window;

/// A click callback for shell controls.
pub(super) type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>;

pub(in crate::platform) use webview::{Favicon, PhotonWebView, WebViewEvent};
pub use window::run;
