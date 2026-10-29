//! A borderless Liquid Glass window that floats a GPUI panel over the
//! browser window. As its own window it is composited by the system, so its
//! glass refracts everything behind it, GPUI's drawing included, which glass
//! inside GPUI's window cannot. See `ui/native_panel.rs`.

use std::time::Duration;

use gpui::{
    Bounds, MacosWindowBackground, Pixels, Window, WindowBounds, WindowKind, WindowOptions,
};
use objc2::{
    rc::Retained,
    runtime::{AnyClass, NSObjectProtocol},
};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSGlassEffectView, NSView, NSWindow,
    NSWindowOrderingMode,
};

use super::appkit::appkit_host_view;

/// A borderless, non-activating panel on Liquid Glass at `bounds`, in screen
/// coordinates. It is shown by [`attach`] once its parent is known.
pub(crate) fn options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        macos_window_background: MacosWindowBackground::LiquidGlass,
        ..WindowOptions::default()
    }
}

/// Ties the panel `panel` to `parent`: it moves with it and sits at its
/// level rather than above every app, with its glass rounded to `radius` and
/// faded in over `fade`.
pub(crate) fn attach(panel: &Window, parent: &Window, radius: f32, fade: Duration) {
    let (Some(panel_window), Some(parent_window)) = (ns_window(panel), ns_window(parent)) else {
        return;
    };
    panel_window.setLevel(parent_window.level());
    panel_window.setHasShadow(true);
    // SAFETY: both windows are live AppKit windows on the main thread.
    unsafe { parent_window.addChildWindow_ordered(&panel_window, NSWindowOrderingMode::Above) };
    if let Some(glass) = appkit_host_view(panel).and_then(|view| {
        // SAFETY: GPUI's live view, accessed on the main thread.
        glass_above(unsafe { view.as_ref() })
    }) {
        glass.setCornerRadius(f64::from(radius));
    }
    panel_window.setAlphaValue(0.0);
    animate_alpha(&panel_window, 1.0, fade);
}

/// Fades the panel out over `fade`, before GPUI removes it.
pub(crate) fn fade_out(panel: &Window, fade: Duration) {
    if let Some(window) = ns_window(panel) {
        animate_alpha(&window, 0.0, fade);
    }
}

fn animate_alpha(window: &NSWindow, alpha: f64, duration: Duration) {
    NSAnimationContext::beginGrouping();
    NSAnimationContext::currentContext().setDuration(duration.as_secs_f64());
    window.animator().setAlphaValue(alpha);
    NSAnimationContext::endGrouping();
}

fn ns_window(window: &Window) -> Option<Retained<NSWindow>> {
    // SAFETY: GPUI's live view, accessed on the main thread.
    appkit_host_view(window).and_then(|view| unsafe { view.as_ref() }.window())
}

/// The glass view GPUI's Liquid Glass background places around its view.
fn glass_above(view: &NSView) -> Option<Retained<NSGlassEffectView>> {
    let glass_class = AnyClass::get(c"NSGlassEffectView")?;
    // SAFETY: AppKit view hierarchy access runs on the main thread.
    let mut ancestor = unsafe { view.superview() };
    while let Some(view) = ancestor {
        if view.isKindOfClass(glass_class) {
            // SAFETY: the view was just checked to be an NSGlassEffectView.
            return Some(unsafe { Retained::cast_unchecked(view) });
        }
        // SAFETY: as above.
        ancestor = unsafe { view.superview() };
    }
    None
}
