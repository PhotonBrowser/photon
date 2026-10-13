//! macOS Engine adapter, native controls, and page renderer.

pub(crate) mod app_menus;
pub(crate) mod default_browser;
pub(crate) mod display;
pub(crate) mod engine;
pub(crate) mod ffi;
pub(crate) mod motion_observer;
pub(crate) mod presentation;
pub(crate) mod renderer;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    true
}

pub(crate) fn keyboard_notation() -> super::KeyboardNotation {
    super::KeyboardNotation {
        mac_symbols: true,
        platform_key: "⌘",
        separator: "",
    }
}

pub(crate) fn is_dark_appearance(appearance: gpui::WindowAppearance) -> bool {
    matches!(
        appearance,
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    )
}
