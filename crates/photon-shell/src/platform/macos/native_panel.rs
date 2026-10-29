//! Window options for a GPUI panel shown in a native `NSPopover`. GPUI-CE
//! shows an anchored popup window's view in a popover attached to its parent,
//! which draws the system's popover material behind the view, refracting
//! everything behind it, GPUI's drawing included. See `ui/native_panel.rs`.

use gpui::{
    AnyWindowHandle, Bounds, MacosWindowBackground, Pixels, Size, WindowBounds, WindowKind,
    WindowOptions, point,
    popup::{PopupGravity, PopupOptions},
};

/// A `size` popover attached to `anchor`, a rectangle in `parent`'s
/// coordinates, opening toward `gravity`.
pub(crate) fn options(
    parent: AnyWindowHandle,
    anchor: Bounds<Pixels>,
    gravity: PopupGravity,
    size: Size<Pixels>,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(0.0.into(), 0.0.into()),
            size,
        ))),
        titlebar: None,
        kind: WindowKind::AnchoredPopup(PopupOptions::new(parent, anchor).gravity(gravity)),
        focus: true,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The popover draws the material; GPUI draws only the content on it.
        macos_window_background: MacosWindowBackground::Transparent,
        ..WindowOptions::default()
    }
}
