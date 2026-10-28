//! GPUI-CE window defaults. Edit this file to change native window behavior.

use foreign_types::ForeignTypeRef;
use gpui::{
    App, Axis, Bounds, MacosWindowBackground, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, point, px, size,
};
use metal::{MetalLayerRef, NSUInteger};
use objc2_app_kit::NSView;
use photon_core::{AnimationSettings, WindowBackgroundStyle};
use raw_window_handle::RawWindowHandle;

use crate::platform::{
    ui::{metrics, window_background_style},
    window_config,
};

const TRAFFIC_LIGHTS_ENABLED_ON_HOVER: bool = true;
/// Use double buffering to keep one full-size drawable out of every window's
/// Metal pool. Under GPU load, the background drawable acquisition may wait
/// instead of keeping a third frame ready.
const PHOTON_MAXIMUM_DRAWABLE_COUNT: NSUInteger = 2;
const WINDOW_CONTROLS_HEIGHT: f32 = 14.0;
const WINDOW_CONTROLS_GAP: f32 = 6.0;
const WINDOW_CONTROLS_INSET: f32 = 80.0;
const WINDOW_CONTROLS_ORIGIN: (f32, f32) = (
    12.0,
    (metrics::TITLEBAR_HEIGHT - WINDOW_CONTROLS_HEIGHT) / 2.0,
);
const WINDOW_CONTROLS_COLUMN_ORIGIN: (f32, f32) = (
    (metrics::SIDEBAR_COLLAPSED_WIDTH - WINDOW_CONTROLS_HEIGHT) / 2.0,
    WINDOW_CONTROLS_ORIGIN.1,
);
const WINDOW_CONTROLS_COLUMN_INSET: f32 = WINDOW_CONTROLS_COLUMN_ORIGIN.1 * 2.0
    + WINDOW_CONTROLS_HEIGHT * 3.0
    + WINDOW_CONTROLS_GAP * 2.0;

/// Shared layout asks the selected backend how much room its native controls use.
pub(in crate::platform) fn native_controls_inset() -> f32 {
    WINDOW_CONTROLS_INSET
}

pub(in crate::platform) fn native_controls_column_inset() -> f32 {
    WINDOW_CONTROLS_COLUMN_INSET
}

pub(in crate::platform) fn native_controls_trailing_inset() -> f32 {
    WINDOW_CONTROLS_INSET
}

pub(in crate::platform) fn uses_custom_window_controls() -> bool {
    false
}

/// Reduces the drawable pool created by GPUI for this window's CAMetalLayer.
pub(in crate::platform) fn configure_renderer_memory(window: &Window) {
    if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window)
        && let RawWindowHandle::AppKit(handle) = handle.as_raw()
    {
        // SAFETY: GPUI's handle borrows a live AppKit view, and this runs on GPUI's main thread.
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
        if let Some(layer) = view.layer() {
            // SAFETY: GPUI's macOS window view uses its renderer's CAMetalLayer as its backing layer.
            let layer = unsafe {
                MetalLayerRef::from_ptr(objc2::rc::Retained::as_ptr(&layer).cast_mut().cast())
            };
            layer.set_maximum_drawable_count(PHOTON_MAXIMUM_DRAWABLE_COUNT);
        }
    }

    // GPUI creates the native window before this callback. Keep it hidden until
    // the drawable limit is set, then present it with the usual key-window behavior.
    window.activate();
}

/// Applies Photon-specific interaction styling to macOS window controls.
pub(in crate::platform) fn configure_native_controls(
    window: &Window,
    animations: &AnimationSettings,
) {
    window.set_traffic_light_hover_behavior(TRAFFIC_LIGHTS_ENABLED_ON_HOVER);
    set_native_controls_fade(window, animations);
}

pub(in crate::platform) fn set_native_controls_fade(
    window: &Window,
    animations: &AnimationSettings,
) {
    window.set_traffic_light_fade_durations(
        animations.traffic_light_fade_in_ms(),
        animations.traffic_light_fade_out_ms(),
    );
}

/// Updates the native controls as the sidebar moves, hides, or changes shape.
pub(in crate::platform) fn update_native_controls(
    window: &Window,
    animations: &AnimationSettings,
    visible: bool,
    column: bool,
    revealed: bool,
    offset: f32,
    layout_changed: bool,
) {
    set_native_controls_fade(window, animations);
    window.set_traffic_light_offset(point(px(offset), px(0.0)));
    if !layout_changed {
        return;
    }
    if visible {
        let (axis, (x, y)) = if column {
            (Axis::Vertical, WINDOW_CONTROLS_COLUMN_ORIGIN)
        } else {
            (Axis::Horizontal, WINDOW_CONTROLS_ORIGIN)
        };
        window.set_traffic_light_axis(axis);
        let inset = if revealed {
            metrics::SIDEBAR_REVEAL_INSET
        } else {
            0.0
        };
        window.set_traffic_light_position(point(px(x + inset), px(y + inset)));
        window.set_traffic_lights_visible(true);
    } else {
        // The sidebar offset carries controls out while they fade away.
        window.set_traffic_lights_visible(false);
    }
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
pub(in crate::platform) fn apply_background_style(window: &Window, style: WindowBackgroundStyle) {
    let appearance = match configured_background(style) {
        MacosWindowBackground::Opaque => WindowBackgroundAppearance::Opaque,
        MacosWindowBackground::Blurred => WindowBackgroundAppearance::Blurred,
        MacosWindowBackground::LiquidGlass => WindowBackgroundAppearance::LiquidGlass,
        _ => WindowBackgroundAppearance::Opaque,
    };
    window.set_background_appearance(appearance);
}

/// Builds a Photon browser window's initial size, titlebar, and native backing.
pub(in crate::platform) fn options(cx: &App) -> WindowOptions {
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

pub(in crate::platform) fn popup_options(
    cx: &App,
    width: Option<i32>,
    height: Option<i32>,
) -> WindowOptions {
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
        // Keep visible chrome animations at display cadence even during brief
        // native focus transitions; GPUI's 33 ms default reads as 30 fps.
        .inactive_frame_interval(Some(std::time::Duration::from_micros(16_667)))
        .show(false)
        .titlebar(Some(
            TitlebarOptions::default()
                .appears_transparent(window_config::MACOS_TRANSPARENT_TITLEBAR)
                .traffic_light_position(point(
                    px(WINDOW_CONTROLS_ORIGIN.0),
                    px(WINDOW_CONTROLS_ORIGIN.1),
                )),
        ))
        .macos_window_background(configured_background(window_background_style(cx)))
        // The titlebar hosts interactive UI, so the shell moves the window itself
        // from empty titlebar space instead of letting AppKit drag from anywhere in it.
        .app_owns_titlebar_drag(window_config::MACOS_APP_OWNS_TITLEBAR_DRAG)
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
