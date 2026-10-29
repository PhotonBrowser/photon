//! A floating panel in its own native Liquid Glass window, with GPUI content.
//!
//! Glass inside GPUI's window can only refract what lies beneath GPUI, so it
//! cuts through any GPUI interface behind it. A panel in its own window is
//! composited by the system over the browser window instead, so its glass
//! refracts everything behind it. macOS only; other platforms draw panels in
//! the window with `glass`. See `Documentation/Glass.md`.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Context, FocusHandle, KeyDownEvent, Pixels, Render, Subscription,
    Task, Window, WindowHandle, div, prelude::*,
};

use super::motion::{Entrance, Transition};
use super::settings::Settings;

/// Draws the panel's content, entering or leaving.
pub(super) type PanelContent = Rc<dyn Fn(Transition, &mut Window, &mut App) -> AnyElement>;

/// One panel at a time, opened by its owner and closed by it or by the person
/// pressing Escape or clicking elsewhere, which `on_dismiss` reports.
#[derive(Default)]
pub(super) struct NativePanel {
    window: Option<WindowHandle<PanelRoot>>,
    _closing: Option<Task<()>>,
}

pub(super) struct PanelOptions {
    /// Where the panel opens, in the parent window's coordinates.
    pub(super) bounds: Bounds<Pixels>,
    pub(super) corner_radius: f32,
    /// The content's entrance, whose duration the window's fade follows.
    pub(super) entrance: Entrance,
    pub(super) content: PanelContent,
    pub(super) on_dismiss: Rc<dyn Fn(&mut App)>,
}

impl NativePanel {
    pub(super) fn is_open(&self) -> bool {
        self.window.is_some()
    }

    /// Opens the panel over `parent`, replacing any already open.
    pub(super) fn open(&mut self, parent: &Window, options: PanelOptions, cx: &mut App) {
        self.close(cx);
        let PanelOptions {
            bounds,
            corner_radius,
            entrance,
            content,
            on_dismiss,
        } = options;
        let origin = parent.bounds().origin;
        let screen_bounds = Bounds::new(origin + bounds.origin, bounds.size);
        let fade = entrance.total_duration(&Settings::motion(cx));
        let parent_handle = parent.window_handle();
        let window = cx.open_window(
            crate::platform::macos::native_panel::options(screen_bounds),
            |window, cx| {
                let root = cx.new(|cx| PanelRoot::new(content, entrance, on_dismiss, window, cx));
                // The parent is borrowed by its own update, so attach once
                // this one returns.
                let panel = window.window_handle();
                cx.defer(move |cx| {
                    let _ = parent_handle.update(cx, |_, parent, cx| {
                        let _ = panel.update(cx, |_, panel, _| {
                            crate::platform::macos::native_panel::attach(
                                panel,
                                parent,
                                corner_radius,
                                fade,
                            );
                        });
                    });
                });
                root
            },
        );
        self.window = window.ok();
        self._closing = None;
    }

    /// Plays the content's exit and fades the window, then removes it.
    pub(super) fn close(&mut self, cx: &mut App) {
        let Some(window) = self.window.take() else {
            return;
        };
        let mut exit = Duration::ZERO;
        let _ = window.update(cx, |root, panel, cx| {
            exit = root.leave(panel, cx);
        });
        self._closing = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(exit).await;
            let _ = window.update(cx, |_, panel, _| panel.remove_window());
        }));
    }
}

struct PanelRoot {
    content: PanelContent,
    transition: Transition,
    entrance: Entrance,
    on_dismiss: Rc<dyn Fn(&mut App)>,
    focus: FocusHandle,
    _activation: Subscription,
}

impl PanelRoot {
    fn new(
        content: PanelContent,
        entrance: Entrance,
        on_dismiss: Rc<dyn Fn(&mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // A click anywhere else takes focus from the panel, which closes it.
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.transition == Transition::Enter {
                (this.on_dismiss)(cx);
            }
        });
        Self {
            content,
            transition: Transition::Enter,
            entrance,
            on_dismiss,
            focus,
            _activation: activation,
        }
    }

    /// Starts the exit, returning how long it takes.
    fn leave(&mut self, window: &Window, cx: &mut Context<Self>) -> Duration {
        self.transition = Transition::Exit;
        cx.notify();
        let exit = self.entrance.exit_duration(&Settings::motion(cx));
        crate::platform::macos::native_panel::fade_out(window, exit);
        exit
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
            .child((self.content)(self.transition, window, cx))
    }
}
