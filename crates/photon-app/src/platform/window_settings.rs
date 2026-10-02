//! GPUI-CE window defaults. Edit this file to change native window behavior.

use gpui::{
    App, Bounds, MacosWindowBackground, TitlebarOptions, WindowBounds, WindowOptions, point, px,
    size,
};

pub(super) const TITLEBAR_HEIGHT: f32 = 30.0;
pub(super) const WEBVIEW_INSET: f32 = 4.0;
pub(super) const WEBVIEW_CORNER_RADIUS: f32 = 12.0;

const INITIAL_WIDTH: f32 = 1200.0;
const INITIAL_HEIGHT: f32 = 760.0;

/// Builds the one Photon browser window's initial size, titlebar, and native backing.
pub(super) fn options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(INITIAL_WIDTH), px(INITIAL_HEIGHT)), cx);
    WindowOptions::new()
        .window_bounds(Some(WindowBounds::Windowed(bounds)))
        .titlebar(Some(
            TitlebarOptions::default()
                .appears_transparent(true)
                .traffic_light_position(point(px(12.0), px(10.0))),
        ))
        .macos_window_background(MacosWindowBackground::Blurred)
}
