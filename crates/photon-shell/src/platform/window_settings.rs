//! GPUI-CE window defaults. Edit this file to change native window behavior.

use gpui::{
    App, Bounds, MacosWindowBackground, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, point, px, size,
};
use photon_core::{AnimationSettings, WindowBackgroundStyle};

use crate::platform::ui::{metrics, window_background_style};

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_ENABLED_ON_HOVER: bool = true;

/// Applies Photon-specific interaction styling to native window controls.
pub(super) fn configure_traffic_lights(window: &Window, animations: &AnimationSettings) {
    #[cfg(target_os = "macos")]
    {
        window.set_traffic_light_hover_behavior(TRAFFIC_LIGHTS_ENABLED_ON_HOVER);
        window.set_traffic_light_fade_durations(
            animations.traffic_light_fade_in_ms(),
            animations.traffic_light_fade_out_ms(),
        );
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, animations);
}

/// PHOTON_WINDOW_BACKGROUND can override the saved preference for development.
fn background_for(value: Option<&str>) -> MacosWindowBackground {
    match value {
        Some("opaque") => MacosWindowBackground::Opaque,
        Some("blurred") => MacosWindowBackground::Blurred,
        Some("liquid-glass") => MacosWindowBackground::LiquidGlass,
        _ => MacosWindowBackground::LiquidGlass,
    }
}

fn background_for_style(style: WindowBackgroundStyle) -> MacosWindowBackground {
    match style {
        WindowBackgroundStyle::LiquidGlass => MacosWindowBackground::LiquidGlass,
        WindowBackgroundStyle::Blurred => MacosWindowBackground::Blurred,
    }
}

fn configured_background(style: WindowBackgroundStyle) -> MacosWindowBackground {
    std::env::var("PHOTON_WINDOW_BACKGROUND")
        .ok()
        .map(|value| background_for(Some(&value)))
        .unwrap_or_else(|| background_for_style(style))
}

/// Applies the selected native material to an already open window.
pub(super) fn apply_background_style(window: &Window, style: WindowBackgroundStyle) {
    let appearance = match configured_background(style) {
        MacosWindowBackground::Opaque => WindowBackgroundAppearance::Opaque,
        MacosWindowBackground::Blurred => WindowBackgroundAppearance::Blurred,
        MacosWindowBackground::LiquidGlass => WindowBackgroundAppearance::LiquidGlass,
        _ => WindowBackgroundAppearance::Opaque,
    };
    window.set_background_appearance(appearance);
}

/// Builds a Photon browser window's initial size, titlebar, and native backing.
pub(super) fn options(cx: &App) -> WindowOptions {
    options_for_size(
        cx,
        metrics::INITIAL_WINDOW_WIDTH,
        metrics::INITIAL_WINDOW_HEIGHT,
    )
}

pub(super) fn popup_options(cx: &App, width: Option<i32>, height: Option<i32>) -> WindowOptions {
    let width = width
        .map(|width| width as f32)
        .unwrap_or(metrics::POPUP_WINDOW_WIDTH)
        .clamp(
            metrics::POPUP_MIN_WINDOW_WIDTH,
            metrics::POPUP_MAX_WINDOW_WIDTH,
        );
    let height = height
        .map(|height| height as f32)
        .unwrap_or(metrics::POPUP_WINDOW_HEIGHT)
        .clamp(
            metrics::POPUP_MIN_WINDOW_HEIGHT,
            metrics::POPUP_MAX_WINDOW_HEIGHT,
        );
    options_for_size(cx, width, height)
}

fn options_for_size(cx: &App, width: f32, height: f32) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions::new()
        .window_bounds(Some(WindowBounds::Windowed(bounds)))
        .titlebar(Some(
            TitlebarOptions::default()
                .appears_transparent(true)
                .traffic_light_position(point(
                    px(metrics::WINDOW_CONTROLS_ORIGIN.0),
                    px(metrics::WINDOW_CONTROLS_ORIGIN.1),
                )),
        ))
        .macos_window_background(configured_background(window_background_style(cx)))
        // The titlebar hosts interactive UI, so the shell moves the window itself
        // from empty titlebar space instead of letting AppKit drag from anywhere in it.
        .app_owns_titlebar_drag(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_native_window_background() {
        assert_eq!(
            background_for(Some("opaque")),
            MacosWindowBackground::Opaque
        );
        assert_eq!(
            background_for(Some("blurred")),
            MacosWindowBackground::Blurred
        );
        assert_eq!(
            background_for(Some("liquid-glass")),
            MacosWindowBackground::LiquidGlass
        );
        assert_eq!(background_for(None), MacosWindowBackground::LiquidGlass);
    }
}
