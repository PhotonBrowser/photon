//! Linux shell adapter: desktop integration and CPU-backed page renderer.
//!
//! The browser chrome, Engine session and WebView component are shared from
//! `platform/ui` and `platform/engine`. This adapter owns Linux-specific window
//! settings and system services.

pub(crate) mod app_menus;
pub(crate) mod default_browser;
pub(crate) mod motion_observer;
pub(crate) mod renderer;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    false
}

pub(crate) fn keyboard_notation() -> super::KeyboardNotation {
    super::KeyboardNotation {
        mac_symbols: false,
        platform_key: "Super",
        separator: "+",
    }
}

pub(crate) fn is_dark_appearance(appearance: gpui::WindowAppearance) -> bool {
    matches!(appearance, gpui::WindowAppearance::Dark)
}
