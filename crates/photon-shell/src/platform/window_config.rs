//! Source controlled defaults for every Photon window created by the shell.
//!
//! Window dimensions are logical pixels. Normal windows use INITIAL_* for the
//! first size and MIN_* as the native resize limit. Popup requests are clamped
//! between POPUP_MIN_* and POPUP_MAX_*; POPUP_* is used when a request omits a
//! dimension.
//!
//! Platform-specific creation switches live here too. The platform adapter
//! translates them to GPUI's native options. Additional GPUI options that can
//! be made configurable here if needed include whether windows are resizable,
//! movable, minimizable, focused or shown on creation, and the macOS titlebar
//! title/appearance. Bounds are centered by default; options_at uses the origin
//! supplied by the caller for windows opened from a detached tab.
//!
//! Other native window details that can be centralized here later include the
//! macOS traffic-light size, gap, position and insets (currently constants in
//! macos/window_settings.rs), and Linux control-group spacing (in
//! ui/metrics.rs). The macOS material is currently a user preference, with
//! PHOTON_WINDOW_BACKGROUND as a development override, so it is intentionally
//! selected at runtime rather than fixed by these defaults. Linux currently
//! uses opaque windows.

/// First size for a normal browser window, in logical pixels.
pub(crate) const INITIAL_WIDTH: f32 = 1200.0;
/// First size for a normal browser window, in logical pixels.
pub(crate) const INITIAL_HEIGHT: f32 = 760.0;
/// Smallest width allowed when resizing a normal browser window.
pub(crate) const MIN_WIDTH: f32 = 640.0;
/// Smallest height allowed when resizing a normal browser window.
pub(crate) const MIN_HEIGHT: f32 = 420.0;

/// Popup fallback width when its request does not specify one.
pub(crate) const POPUP_WIDTH: f32 = 720.0;
/// Popup fallback height when its request does not specify one.
pub(crate) const POPUP_HEIGHT: f32 = 620.0;
/// Smallest requested popup width after clamping.
pub(crate) const POPUP_MIN_WIDTH: f32 = 360.0;
/// Largest requested popup width after clamping.
pub(crate) const POPUP_MAX_WIDTH: f32 = 1280.0;
/// Smallest requested popup height after clamping.
pub(crate) const POPUP_MIN_HEIGHT: f32 = 320.0;
/// Largest requested popup height after clamping.
pub(crate) const POPUP_MAX_HEIGHT: f32 = 1000.0;

/// Whether macOS should draw the titlebar as transparent.
#[cfg(target_os = "macos")]
pub(crate) const MACOS_TRANSPARENT_TITLEBAR: bool = true;
/// Whether Photon handles dragging from the macOS titlebar area.
#[cfg(target_os = "macos")]
pub(crate) const MACOS_APP_OWNS_TITLEBAR_DRAG: bool = true;
/// Whether Linux windows use client-side decorations, which host Photon controls.
#[cfg(target_os = "linux")]
pub(crate) const LINUX_CLIENT_DECORATIONS: bool = true;
