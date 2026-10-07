//! Semantic colors and shared measurements for the native browser shell.

use gpui::WindowAppearance;

/// Shared shell geometry, in logical pixels.
pub(crate) mod metrics {
    pub const TITLEBAR_HEIGHT: f32 = 36.0;
    /// Height of the native window control buttons.
    const WINDOW_CONTROLS_HEIGHT: f32 = 14.0;
    /// Top-left origin of the native window controls, vertically centered in the titlebar.
    pub const WINDOW_CONTROLS_ORIGIN: (f32, f32) =
        (12.0, (TITLEBAR_HEIGHT - WINDOW_CONTROLS_HEIGHT) / 2.0);
    /// Titlebar space reserved on each side so content stays clear of the
    /// native window controls and remains centered.
    pub const WINDOW_CONTROLS_INSET: f32 = 80.0;
    pub const OMNIBOX_HEIGHT: f32 = 28.0;
    pub const OMNIBOX_MAX_WIDTH: f32 = 640.0;
    pub const PAGE_INSET: f32 = 4.0;
    pub const WEBVIEW_CORNER_RADIUS: f32 = 12.0;
}

pub(crate) mod colors {
    pub const PAGE_BACKGROUND: u32 = 0xffffff;
}

/// Shell chrome colors for the window's light or dark appearance. Fills are
/// RGBA so they tint the native window material rather than hide it.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub text: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub field: u32,
    pub field_focused: u32,
    pub selection: u32,
}

impl Palette {
    const LIGHT: Self = Self {
        text: 0x1f2933,
        text_muted: 0x5f6b76,
        accent: 0x477d99,
        field: 0x0000000f,
        field_focused: 0x00000017,
        selection: 0x477d9940,
    };

    const DARK: Self = Self {
        text: 0xe8eaed,
        text_muted: 0x9aa0a6,
        accent: 0x8ab4f8,
        field: 0xffffff17,
        field_focused: 0xffffff24,
        selection: 0x8ab4f866,
    };

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::DARK,
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::LIGHT,
        }
    }
}
