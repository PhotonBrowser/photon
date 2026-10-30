//! Windows shell adapter: native window policy and the portable Engine renderer.

pub(crate) mod app_menus;
pub(crate) mod default_browser;
pub(crate) mod motion_observer;
pub(crate) mod renderer;
pub(crate) mod termination;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    false
}

pub(crate) fn keyboard_notation() -> super::KeyboardNotation {
    super::KeyboardNotation {
        mac_symbols: false,
        platform_key: "Ctrl",
        separator: "+",
    }
}

pub(crate) fn is_dark_appearance(appearance: gpui::WindowAppearance) -> bool {
    matches!(appearance, gpui::WindowAppearance::Dark)
}
