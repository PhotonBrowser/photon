//! Shared GPUI browser UI and target-specific shell adapters.

use photon_core::CommandAvailability;

#[cfg_attr(target_os = "linux", allow(dead_code))]
mod app_update;
#[allow(dead_code)]
mod cpu_presentation;
#[allow(dead_code)]
mod cpu_renderer;
pub(crate) mod display;
pub(crate) mod engine;
mod engine_ffi;
pub(crate) mod haptics;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg_attr(target_os = "linux", allow(dead_code))]
#[path = "ui/metrics.rs"]
pub(crate) mod metrics;
mod permission_coordinator;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod portable_display;
pub(crate) mod presentation;
pub(crate) mod shortcuts;
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) mod theme_config;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod ui;
pub(crate) mod window_config;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub(crate) fn system_idle_duration() -> Option<std::time::Duration> {
    macos::idle::duration()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn system_idle_duration() -> Option<std::time::Duration> {
    None
}

#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub(crate) use linux::{
    app_menus, default_browser, is_dark_appearance, keyboard_notation, motion_observer, renderer,
    supports_elastic_scroll, window_observer, window_settings,
};
#[cfg(target_os = "macos")]
pub use macos::appkit::{
    AppKitButton, AppKitButtonStyle, AppKitGlassBackdrop, AppKitTabBar, AppKitView,
    AppKitViewElement, GlassWindowTint,
};
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    app_menus, default_browser, is_dark_appearance, keyboard_notation, motion_observer, renderer,
    supports_elastic_scroll, window_observer, window_settings,
};
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    app_menus, default_browser, is_dark_appearance, keyboard_notation, motion_observer, renderer,
    supports_elastic_scroll, window_observer, window_settings,
};

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub(crate) use ui::run;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use ui::show_alert;

/// State the shared browser UI sends to its native menu adapter.
#[derive(Clone, Copy, Default, PartialEq)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) struct MenuBarState {
    pub(crate) commands: CommandAvailability,
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
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    NotDefault,
    Default,
}

#[derive(Clone, Copy)]
pub(crate) struct PageSurfacePalette {
    pub(crate) background: u32,
}

/// Where a page appears in its window, from the web view's GPUI layout. A
/// platform that shows pages in native layers places them from this.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PagePlacement {
    pub(crate) bounds: gpui::Bounds<gpui::Pixels>,
    pub(crate) corner_radius: f32,
    pub(crate) scale_factor: f32,
    /// Shown until the page's first frame, as `0xRRGGBB`.
    pub(crate) background: u32,
}

pub(crate) fn verbose_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("PHOTON_VERBOSE").is_some())
}

fn trace(args: std::fmt::Arguments<'_>) {
    if verbose_trace_enabled() {
        eprintln!("[PhotonWebView/GPUI-CE] {args}");
    }
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
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
