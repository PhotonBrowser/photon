//! GPUI-CE window defaults. Edit this file to change native window behavior.

use gpui::{
    App, Bounds, MacosWindowBackground, TitlebarOptions, Window, WindowBounds, WindowOptions,
    point, px, size,
};

use crate::platform::ui::metrics;

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_ENABLED_ON_HOVER: bool = true;

/// Applies Photon-specific interaction styling to native window controls.
pub(super) fn configure_traffic_lights(window: &Window) {
    #[cfg(target_os = "macos")]
    window.set_traffic_light_hover_behavior(TRAFFIC_LIGHTS_ENABLED_ON_HOVER);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Select with PHOTON_WINDOW_BACKGROUND=opaque|blurred|liquid-glass.
/// The system controls the strength of both native materials.
fn background_for(value: Option<&str>) -> MacosWindowBackground {
    match value {
        Some("opaque") => MacosWindowBackground::Opaque,
        Some("liquid-glass") => MacosWindowBackground::LiquidGlass,
        _ => MacosWindowBackground::Blurred,
    }
}

fn background() -> MacosWindowBackground {
    background_for(std::env::var("PHOTON_WINDOW_BACKGROUND").ok().as_deref())
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
        .macos_window_background(background())
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
        assert_eq!(background_for(None), MacosWindowBackground::Blurred);
    }
}
