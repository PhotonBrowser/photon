//! Linux shell adapter landing point.
#![allow(dead_code)] // These adapter contracts are pending Linux shell wiring.
//!
//! Linux window integration, system services, Engine bindings, and page
//! presentation belong here. The browser chrome and WebView component are
//! shared from `platform/ui`; this adapter supplies client-side window
//! decorations and a placeholder page renderer, which stays blank and shows a
//! notice until a Linux Engine surface is connected.

pub(crate) mod app_menus;
pub(crate) mod default_browser;
pub(crate) mod motion_observer;
pub(crate) mod renderer;
pub(crate) mod window_observer;
pub(crate) mod window_settings;

pub(crate) fn supports_elastic_scroll() -> bool {
    false
}

pub(crate) fn run() {
    super::trace(format_args!("Linux shell adapter is not implemented"));
    eprintln!(
        "the Linux Photon shell adapter is not implemented yet; its code belongs in crates/photon-shell/src/platform/linux"
    );
    std::process::exit(1);
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
