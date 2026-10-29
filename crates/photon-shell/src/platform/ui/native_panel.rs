//! A panel with GPUI content in a native `NSPopover`, on macOS.
//!
//! Glass inside GPUI's window can only refract what lies beneath GPUI, so it
//! cuts through any GPUI interface behind it. A popover is its own window,
//! composited by the system, so its material refracts everything behind it.
//! AppKit animates it in and out and closes it on a click elsewhere, as Zen's
//! native popovers do. Other platforms draw panels in the window with
//! `glass`. See `Documentation/Glass.md`.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, KeyDownEvent, Pixels, Render, Size, Window,
    WindowHandle, div, popup::PopupGravity, prelude::*,
};

use super::motion::Edge;

/// Draws the panel's content.
pub(super) type PanelContent = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One popover at a time, opened by its owner and closed by it, by Escape or
/// by a click elsewhere, which `on_dismiss` reports.
#[derive(Default)]
pub(super) struct NativePanel {
    window: Option<WindowHandle<PanelRoot>>,
    /// Whether the open popover's closing is the person's doing, to report.
    reports_close: Rc<Cell<bool>>,
}

pub(super) struct PanelOptions {
    /// What the popover points at, in the parent window's coordinates.
    pub(super) anchor: Bounds<Pixels>,
    /// The side of the anchor the popover opens on.
    pub(super) side: Edge,
    pub(super) size: Size<Pixels>,
    pub(super) content: PanelContent,
    pub(super) on_dismiss: Rc<dyn Fn(&mut App)>,
}

impl NativePanel {
    pub(super) fn is_open(&self) -> bool {
        self.window.is_some()
    }

    /// Opens the popover from `parent`, replacing any already open.
    pub(super) fn open(&mut self, parent: &Window, options: PanelOptions, cx: &mut App) {
        self.close(cx);
        let PanelOptions {
            anchor,
            side,
            size,
            content,
            on_dismiss,
        } = options;
        let gravity = match side {
            Edge::Left => PopupGravity::Left,
            Edge::Right => PopupGravity::Right,
            Edge::Top => PopupGravity::Top,
            Edge::Bottom => PopupGravity::Bottom,
        };
        let reports_close = Rc::new(Cell::new(true));
        self.reports_close = reports_close.clone();
        let window = cx.open_window(
            crate::platform::macos::native_panel::options(
                parent.window_handle(),
                anchor,
                gravity,
                size,
            ),
            move |window, cx| {
                let root = cx.new(|cx| PanelRoot::new(content, on_dismiss.clone(), window, cx));
                // AppKit closes the popover itself on a click elsewhere.
                cx.observe_release(&root, move |_, cx| {
                    if reports_close.get() {
                        on_dismiss(cx);
                    }
                })
                .detach();
                root
            },
        );
        self.window = window.ok();
    }

    /// Closes the popover, which AppKit animates away.
    pub(super) fn close(&mut self, cx: &mut App) {
        if let Some(window) = self.window.take() {
            // The owner closes it, so it already knows.
            self.reports_close.set(false);
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }
}

struct PanelRoot {
    content: PanelContent,
    on_dismiss: Rc<dyn Fn(&mut App)>,
    focus: FocusHandle,
}

impl PanelRoot {
    fn new(
        content: PanelContent,
        on_dismiss: Rc<dyn Fn(&mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            content,
            on_dismiss,
            focus,
        }
    }
}

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    (this.on_dismiss)(cx);
                }
            }))
            .child((self.content)(window, cx))
    }
}
