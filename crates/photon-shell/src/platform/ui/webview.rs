//! The page surface and browser input forwarding.

use super::super::engine::EngineSession;
use super::super::presentation::{PresentedSurface, Release};
use super::super::window_settings::WEBVIEW_CORNER_RADIUS;
use super::input::WebViewInput;
use super::theme::colors;
use gpui::{
    Context, FocusHandle, InteractiveElement, KeyDownEvent, KeyUpEvent, ObjectFit, Render,
    Subscription, SurfaceSource, Window, div, prelude::*, px, surface,
};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::super::trace;

static MOUSE_MOVE_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(in crate::platform) struct PhotonWebView {
    pub(super) external: Option<PresentedSurface>,
    pub(in crate::platform) loading: bool,
    pub(super) session: EngineSession,
    pub(super) focus_handle: FocusHandle,
    pub(super) last_viewport: Option<(i32, i32, u32)>,
    input: WebViewInput,
    pub(super) _quit_subscription: Option<Subscription>,
}

impl Drop for PhotonWebView {
    fn drop(&mut self) {
        self.prepare_shutdown();
    }
}

impl PhotonWebView {
    pub(in crate::platform) fn present_latest(&mut self, cx: &mut Context<Self>) {
        self.session.drain_releases();
        let Some(presented) = self.session.presentation.take_surface() else {
            return;
        };
        if let Some(retired) = self.external.take() {
            let pending = std::mem::take(&mut *retired.releases_after_completion.lock().unwrap());
            let mut next_releases = presented.releases_after_completion.lock().unwrap();
            next_releases.push(Release { key: retired.key });
            next_releases.extend(pending);
        }
        self.external = Some(presented);
        cx.notify();
    }

    pub(super) fn prepare_shutdown(&mut self) {
        if self.session.shutdown_started {
            return;
        }
        let final_releases = if let Some(presented) = self.external.as_ref() {
            let releases = presented.releases_after_completion.clone();
            let mut pending = releases.lock().unwrap();
            if !pending.iter().any(|release| release.key == presented.key) {
                pending.push(Release { key: presented.key });
            }
            drop(pending);
            releases
        } else {
            Arc::new(Mutex::new(Vec::new()))
        };
        self.session.begin_shutdown(final_releases);
    }

    fn update_viewport(&mut self, width: f32, height: f32, scale: f32) {
        let width = width.round() as i32;
        let height = height.round() as i32;
        let viewport = (width, height, scale.to_bits());
        if self.last_viewport == Some(viewport) || width <= 0 || height <= 0 {
            return;
        }

        self.session.resize(width, height, f64::from(scale));
        trace(format_args!(
            "viewport logical={width}x{height} dpr={scale:.2} physical={}x{}",
            (width as f32 * scale).round() as i32,
            (height as f32 * scale).round() as i32,
        ));
        self.last_viewport = Some(viewport);
        // The initial native window's backing scale can differ from the scale
        // available before it exists (for example, Retina 2x). Navigate only
        // after the measured WebView bounds and scale have been sent to Engine.
        self.session.navigate_startup();
    }

    pub(super) fn new(cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let width = 1200;
        let height = 760;
        Ok(Self {
            external: None,
            loading: false,
            session: EngineSession::create(width, height, 1.0)?,
            focus_handle: cx.focus_handle(),
            last_viewport: None,
            input: WebViewInput::default(),
            _quit_subscription: None,
        })
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak_this = cx.entity().downgrade();
        let weak_mouse_down = weak_this.clone();
        let weak_mouse_up = weak_this.clone();
        let mut webview = div()
            .size_full()
            .rounded(px(WEBVIEW_CORNER_RADIUS))
            .bg(gpui::rgb(colors::PAGE_BACKGROUND))
            .id("photon-webview-viewport")
            .on_prepaint(
                cx.listener(|this, event: &gpui::InteractivityPrepaint, window, _| {
                    // Resize from the laid-out WebView bounds, not the native window
                    // bounds minus assumed titlebar/padding values. During live resize,
                    // those assumptions can differ by a frame and offset the page.
                    this.input.set_viewport_origin(
                        f32::from(event.bounds.origin.x),
                        f32::from(event.bounds.origin.y),
                    );
                    this.update_viewport(
                        f32::from(event.bounds.size.width),
                        f32::from(event.bounds.size.height),
                        window.scale_factor(),
                    );
                }),
            )
            .track_focus(&self.focus_handle)
            .on_mouse_down_all(move |event, phase, hitbox, window, app| {
                if phase != gpui::DispatchPhase::Bubble || !hitbox.contains(&event.position) {
                    return;
                }
                weak_mouse_down
                    .update(app, |this, cx| {
                        window.focus(&this.focus_handle, cx);
                        this.session.set_focus(true);
                        this.input.mouse_down(&mut this.session, event);
                    })
                    .ok();
            })
            .on_mouse_up_all(move |event, phase, hitbox, _window, app| {
                if phase != gpui::DispatchPhase::Bubble || !hitbox.contains(&event.position) {
                    return;
                }
                weak_mouse_up
                    .update(app, |this, _cx| {
                        this.input.mouse_up(&mut this.session, event)
                    })
                    .ok();
            })
            .on_mouse_move_all(move |event, phase, hitbox, _window, app| {
                // Forward pointer motion during capture so child elements in the
                // external surface cannot stop propagation before the page sees it.
                if phase != gpui::DispatchPhase::Capture {
                    return;
                }
                let move_count = MOUSE_MOVE_TRACE_COUNT.fetch_add(1, Ordering::Relaxed);
                let inside_viewport = hitbox.contains(&event.position);
                if move_count < 5 || move_count.is_multiple_of(60) {
                    trace(format_args!(
                        "mouse-move #{move_count} phase={phase:?} position=({}, {}) inside={inside_viewport} bounds={:?}",
                        f32::from(event.position.x),
                        f32::from(event.position.y),
                        hitbox.bounds,
                    ));
                }
                weak_this
                    .update(app, |this, _cx| {
                        if move_count < 5 || move_count.is_multiple_of(60) {
                            trace(format_args!("mouse-move dispatch #{move_count} inside={inside_viewport}"));
                        }
                        this.input.mouse_move(&mut this.session, event, inside_viewport);
                    })
                    .ok();
            })
            .on_mouse_exit(cx.listener(|this, event, _, _| {
                this.input.mouse_exit(&mut this.session, event);
            }))
            .on_scroll_wheel(cx.listener(|this, event, _, _| {
                this.input.scroll(&mut this.session, event);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let should_consume = this.input.key_down(&mut this.session, event);
                // AppKit routes printable input and non-character commands through its input
                // context. Consume them after forwarding so commands such as Backspace are
                // not replayed.
                if should_consume {
                    cx.stop_propagation();
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                this.input.key_up(&mut this.session, event);
            }));
        if let Some(presented) = self.external.as_ref() {
            webview = webview.child(
                surface(SurfaceSource::ExternalMetal(presented.surface.clone()))
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .rounded(px(WEBVIEW_CORNER_RADIUS)),
            );
        }
        webview
    }
}
