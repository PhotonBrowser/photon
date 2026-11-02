//! Windows shell adapter: native window policy and the portable Engine renderer.

pub(crate) mod app_menus;
pub(crate) mod default_browser;
#[cfg(not(feature = "ui-only"))]
pub(crate) mod external_apps;
pub(crate) mod motion_observer;
pub(crate) mod renderer;
pub(crate) mod termination;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    false
}

/// Whether popovers and menus can sharpen out of a blur as they appear. GPUI
/// blurs at half resolution: on the 1x displays common here, content
/// animating through it snaps to a coarse grid and pops sharp at the end.
pub(crate) fn supports_entrance_blur() -> bool {
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
