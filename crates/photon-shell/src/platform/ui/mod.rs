//! GPUI views and window composition.

mod animated_icons;
mod appearance_preview;
mod background_noise;
mod button;
mod color_picker;
mod command_palette;
mod controls;
mod downloads;
mod dropdown;
mod find_bar;
mod glass;
mod history;
mod icons;
mod input;
mod js_dialog;
mod kbd;
mod layout;
mod menu;
mod theme_gradient;
mod theme_picker;
pub(super) use crate::platform::metrics;
mod modal;
mod motion;
#[cfg(target_os = "macos")]
mod native_panel;
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
mod tooltip;
mod webview;
mod window;
mod window_controls;
mod window_space;
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

pub(in crate::platform) fn interaction_sounds_enabled(cx: &gpui::App) -> bool {
    settings::Settings::get(cx).interaction_sounds
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(in crate::platform) fn window_background_style(
    cx: &gpui::App,
) -> photon_core::WindowBackgroundStyle {
    settings::Settings::get(cx).window_background_style
}

pub(in crate::platform) use webview::{
    CrashNotice, Favicon, FindResult, PageMenuItem, PhotonWebView, SelectDropdown, SelectItem,
    WebViewEvent,
};
pub use window::run;
pub use window::show_alert;
