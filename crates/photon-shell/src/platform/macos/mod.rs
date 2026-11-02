//! macOS Engine adapter, native controls, and page renderer.

pub(crate) mod app_menus;
pub(crate) mod appkit;
pub(crate) mod default_browser;
pub(crate) mod display;
#[cfg(not(feature = "ui-only"))]
pub(crate) mod external_apps;
pub(crate) mod glass_backdrop;
pub(crate) mod idle;
pub(crate) mod memory_pressure;
pub(crate) mod motion_observer;
pub(crate) mod native_panel;
mod page_layer;
pub(crate) mod presentation;
pub(crate) mod renderer;
pub(crate) mod site_info_popover;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    true
}

/// Whether popovers and menus can sharpen out of a blur as they appear.
/// GPUI blurs at half resolution, which Retina displays hide.
pub(crate) fn supports_entrance_blur() -> bool {
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
