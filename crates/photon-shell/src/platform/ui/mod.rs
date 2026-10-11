//! GPUI views and window composition.

mod button;
mod color_picker;
mod command_bar;
mod controls;
mod dropdown;
mod find_bar;
mod history;
mod icons;
mod input;
mod js_dialog;
mod kbd;
mod layout;
mod menu;
pub(super) mod metrics;
mod modal;
mod motion;
mod omnibox;
mod pages;
mod settings;
mod shortcuts_dialog;
mod sidebar;
mod space_editor;
mod status_chip;
mod tabs;
pub(super) mod theme;
mod titlebar;
mod toolbar;
mod webview;
mod window;
mod workspace_layout;
mod workspace_preset_fields;

/// A click callback for shell controls.
pub(super) type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>;
/// A callback for a mouse button pressed on an element, such as a
/// right-click that opens its menu.
pub(super) type MouseDownHandler =
    Box<dyn Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App)>;
/// A callback for a mouse button released on an element.
pub(super) type MouseUpHandler =
    Box<dyn Fn(&gpui::MouseUpEvent, &mut gpui::Window, &mut gpui::App)>;
/// A callback for a key pressed while an element has focus.
pub(super) type KeyDownHandler =
    Box<dyn Fn(&gpui::KeyDownEvent, &mut gpui::Window, &mut gpui::App)>;

pub(in crate::platform) use webview::{
    CrashNotice, Favicon, FindResult, PageMenuItem, PhotonWebView, WebViewEvent,
};
pub use window::run;
