//! GPUI-CE window defaults. Edit this file to change native window behavior.

use gpui::{
    App, Bounds, MacosWindowBackground, TitlebarOptions, WindowBounds, WindowOptions, point, px,
    size,
};

pub(super) const WEBVIEW_CORNER_RADIUS: f32 =
    crate::platform::ui::theme::metrics::WEBVIEW_CORNER_RADIUS;

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
        .macos_window_background(background())
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
