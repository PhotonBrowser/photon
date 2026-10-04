//! Semantic colors and shared measurements for the native browser shell.

/// Shared shell geometry, in logical pixels.
pub(crate) mod metrics {
    pub const TITLEBAR_HEIGHT: f32 = 30.0;
    pub const PAGE_INSET: f32 = 4.0;
    pub const WEBVIEW_CORNER_RADIUS: f32 = 12.0;
}

pub(crate) mod colors {
    pub const PAGE_BACKGROUND: u32 = 0xffffff;
}
