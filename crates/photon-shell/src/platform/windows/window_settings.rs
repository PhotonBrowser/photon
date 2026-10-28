//! Windows window defaults for the shared Photon browser window.

use gpui::{
    App, Bounds, Window, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowOptions, WindowsWindowBackground, px, size,
};
use photon_core::{AnimationSettings, WindowBackgroundStyle};

use crate::platform::{metrics, ui::window_background_style, window_config};

pub(crate) fn native_controls_inset() -> f32 {
    0.0
}

pub(crate) fn native_controls_column_inset() -> f32 {
    0.0
}

pub(crate) fn native_controls_trailing_inset() -> f32 {
    metrics::WINDOW_CONTROL_GROUP_WIDTH + metrics::WINDOW_CONTROL_EDGE_INSET * 2.0
}

pub(crate) fn uses_custom_window_controls() -> bool {
    true
}

pub(crate) fn configure_native_controls(_: &Window, _: &AnimationSettings) {}

pub(in crate::platform) fn configure_renderer_memory(_: &Window) {}

pub(crate) fn update_native_controls(
    _: &Window,
    _: &AnimationSettings,
    _: bool,
    _: bool,
    _: bool,
    _: f32,
    _: bool,
) {
}

pub(crate) fn apply_background_style(window: &Window, style: WindowBackgroundStyle) {
    let appearance = match style {
        WindowBackgroundStyle::LiquidGlass => WindowBackgroundAppearance::MicaBackdrop,
        WindowBackgroundStyle::Blurred => WindowBackgroundAppearance::Blurred,
    };
    window.set_background_appearance(appearance);
}

pub(crate) fn options(cx: &App) -> WindowOptions {
    let mut options = options_for_size(
        cx,
        window_config::INITIAL_WIDTH,
        window_config::INITIAL_HEIGHT,
    );
    options.window_min_size = Some(size(
        px(window_config::MIN_WIDTH),
        px(window_config::MIN_HEIGHT),
    ));
    options
}

pub(crate) fn popup_options(cx: &App, width: Option<i32>, height: Option<i32>) -> WindowOptions {
    let width = width
        .map(|width| width as f32)
        .unwrap_or(window_config::POPUP_WIDTH)
        .clamp(
            window_config::POPUP_MIN_WIDTH,
            window_config::POPUP_MAX_WIDTH,
        );
    let height = height
        .map(|height| height as f32)
        .unwrap_or(window_config::POPUP_HEIGHT)
        .clamp(
            window_config::POPUP_MIN_HEIGHT,
            window_config::POPUP_MAX_HEIGHT,
        );
    options_for_size(cx, width, height)
}

fn options_for_size(cx: &App, width: f32, height: f32) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions::new()
        .window_bounds(Some(WindowBounds::Windowed(bounds)))
        .inactive_frame_interval(Some(std::time::Duration::from_micros(16_667)))
        .window_decorations(
            window_config::WINDOWS_CLIENT_DECORATIONS.then_some(WindowDecorations::Client),
        )
        .windows_window_background(match window_background_style(cx) {
            WindowBackgroundStyle::LiquidGlass => WindowsWindowBackground::MicaBackdrop,
            WindowBackgroundStyle::Blurred => WindowsWindowBackground::Blurred,
        })
}
