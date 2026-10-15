//! Shared GPUI browser UI and target-specific shell adapters.

#[cfg_attr(target_os = "linux", allow(dead_code))]
mod app_update;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[path = "ui/metrics.rs"]
pub(crate) mod metrics;
#[cfg(target_os = "macos")]
mod ui;
pub(crate) mod window_config;

#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub(crate) use linux::{
    app_menus, default_browser, is_dark_appearance, keyboard_notation, motion_observer, renderer,
    supports_elastic_scroll, window_observer, window_settings,
};
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    app_menus, default_browser, engine, is_dark_appearance, keyboard_notation, motion_observer,
    renderer, supports_elastic_scroll, window_observer, window_settings,
};

#[cfg(target_os = "linux")]
pub(crate) use linux::run;
#[cfg(target_os = "macos")]
pub(crate) use ui::run;

/// State the shared browser UI sends to its native menu adapter.
#[derive(Clone, Copy, Default, PartialEq)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) struct MenuBarState {
    pub(crate) has_sidebar: bool,
    pub(crate) sidebar_icons: bool,
    pub(crate) zen: bool,
    pub(crate) loading: bool,
}

/// Key labels vary with the desktop's shortcut notation.
#[derive(Clone, Copy)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) struct KeyboardNotation {
    pub(crate) mac_symbols: bool,
    pub(crate) platform_key: &'static str,
    pub(crate) separator: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) enum DefaultBrowserStatus {
    Unavailable,
    NotDefault,
    Default,
}

#[derive(Clone, Copy)]
pub(crate) struct PageSurfacePalette {
    pub(crate) background: u32,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) notice_surface: u32,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) notice_border: u32,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) notice_text: u32,
}

fn trace(args: std::fmt::Arguments<'_>) {
    // Read once: traces run on every frame, including in GPU completion handlers.
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("PHOTON_VERBOSE").is_some()) {
        eprintln!("[PhotonWebView/GPUI-CE] {args}");
    }
}

pub(crate) fn automation_lifecycle_trace(args: std::fmt::Arguments<'_>) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static EMITTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if *ENABLED.get_or_init(|| {
        std::env::var_os("PHOTON_AUTOMATION_LIFECYCLE_TRACE").is_some()
            && std::env::args_os().any(|argument| argument == "--automation-socket")
    }) && EMITTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 64
    {
        eprintln!("[PhotonAutomationLifecycle] {args}");
    }
}
