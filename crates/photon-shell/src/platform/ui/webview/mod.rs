//! The page surface, its tab state, and browser input forwarding.

use super::super::engine::{EngineSession, RequestedWebView};
use super::super::presentation::PresentedSurface;
use super::input::WebViewInput;
use super::{
    metrics::WEBVIEW_CORNER_RADIUS,
    theme::{ThemeColors, ThemePreference},
};
use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement, KeyDownEvent, KeyUpEvent, ObjectFit,
    Render, Subscription, SurfaceSource, Window, WindowAppearance, div, prelude::*, px, surface,
};
use photon_core::{BrowserCommand, BrowserState, EngineEvent, PageCrashes, PageDialogs};
use photon_performance::{PerformanceDiagnostics, performance_overlay};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::super::trace;

mod context_menu;
mod crashes;
mod dialogs;
mod favicon;
mod find;
mod history;
mod zoom;

pub(in crate::platform) use context_menu::{PageMenu, PageMenuItem};
pub(in crate::platform) use crashes::CrashNotice;
pub(in crate::platform) use favicon::Favicon;
pub(in crate::platform) use find::FindResult;

static MOUSE_MOVE_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// How long a load runs before its tab shows a spinner, so brief loads do
/// not flash one over the page icon.
const SPINNER_DELAY: Duration = Duration::from_millis(150);

pub(in crate::platform) struct PhotonWebView {
    pub(super) external: Option<PresentedSurface>,
    pub(in crate::platform) state: BrowserState,
    pub(in crate::platform) diagnostics: PerformanceDiagnostics,
    pub(in crate::platform) performance_overlay_enabled: bool,
    /// What the tab's crash chip says, if anything.
    pub(in crate::platform) crash_notice: Option<CrashNotice>,
    crashes: PageCrashes,
    /// The page's zoom level as a factor, 1.0 being 100%.
    zoom_level: f64,
    /// Whether page input has gone unanswered by WebContent.
    pub(in crate::platform) page_unresponsive: bool,
    /// The address last recorded in the session's history.
    recorded_url: Option<String>,
    /// The context menu the page last asked for.
    pub(in crate::platform) context_menu: Option<PageMenu>,
    /// The latest result of a find-in-page search.
    pub(in crate::platform) find_result: Option<FindResult>,
    /// The page's icon, shown in its tab while the page is not loading.
    pub(in crate::platform) favicon: Option<Favicon>,
    /// Whether the page is playing audio and whether its tab is muted.
    pub(in crate::platform) audio_playing: bool,
    pub(in crate::platform) audio_muted: bool,
    /// When the current load started.
    loading_since: Option<Instant>,
    /// The page's JavaScript dialog, which the window shows over the page.
    pub(in crate::platform) dialogs: PageDialogs,
    pub(super) theme: ThemePreference,
    is_blank_tab: bool,
    pub(super) session: EngineSession,
    pub(super) focus_handle: FocusHandle,
    pub(super) last_viewport: Option<(i32, i32, u32)>,
    /// The color scheme last sent to Engine, `true` for dark.
    sent_dark_color_scheme: Option<bool>,
    input: WebViewInput,
    pub(super) _quit_subscription: Option<Subscription>,
    focus_subscriptions: Vec<Subscription>,
    pending_new_webviews: HashMap<u64, RequestedWebView>,
}

/// Page changes the browser chrome shows. Engine frames do not emit it, so
/// the cached chrome is not rebuilt for every page frame.
pub(in crate::platform) enum WebViewEvent {
    StateChanged,
    NewWebViewRequested(u64),
    /// The page asked for its context menu, now in `context_menu`.
    ContextMenuRequested,
    /// A context menu action opened a link in a new tab.
    OpenInNewTab {
        url: String,
        activate: bool,
    },
}

impl EventEmitter<WebViewEvent> for PhotonWebView {}

impl Drop for PhotonWebView {
    fn drop(&mut self) {
        self.prepare_shutdown();
    }
}

impl PhotonWebView {
    pub(in crate::platform) fn present_latest(&mut self, cx: &mut Context<Self>) {
        self.session.drain_releases();
        let Some(presented) = self.session.presentation.take_surface() else {
            // A rejected frame completes its lease with no GPU work to follow.
            self.session.drain_releases();
            return;
        };
        if let Some(replaced) = self.external.take() {
            let pending = std::mem::take(&mut *replaced.retired_until_submitted.lock().unwrap());
            let mut retired = presented.retired_until_submitted.lock().unwrap();
            retired.push(replaced.retire());
            retired.extend(pending);
        }
        self.external = Some(presented);
        cx.notify();
    }

    /// Re-renders the view and tells the chrome its page state changed.
    pub(in crate::platform) fn state_changed(&self, cx: &mut Context<Self>) {
        cx.emit(WebViewEvent::StateChanged);
        cx.notify();
    }

    pub(in crate::platform) fn set_state(&mut self, state: BrowserState, cx: &mut Context<Self>) {
        if self.state == state {
            return;
        }
        if state.loading != self.state.loading {
            self.loading_since = state.loading.then(Instant::now);
        }
        self.state.apply(EngineEvent::ViewStateChanged(state));
        self.state_changed(cx);
    }

    pub(in crate::platform) fn set_page_unresponsive(
        &mut self,
        unresponsive: bool,
        cx: &mut Context<Self>,
    ) {
        if self.page_unresponsive == unresponsive {
            return;
        }
        self.page_unresponsive = unresponsive;
        self.state_changed(cx);
    }

    pub(in crate::platform) fn set_audio_state(
        &mut self,
        playing: bool,
        muted: bool,
        cx: &mut Context<Self>,
    ) {
        if self.audio_playing == playing && self.audio_muted == muted {
            return;
        }
        self.audio_playing = playing;
        self.audio_muted = muted;
        self.state_changed(cx);
    }

    pub(super) fn toggle_audio_mute(&mut self, cx: &mut Context<Self>) {
        self.audio_muted = self.session.toggle_audio_mute();
        self.state_changed(cx);
    }

    pub(super) fn restart_unresponsive_page(&mut self) {
        self.session.restart_unresponsive_page();
    }

    /// Whether the tab should show a loading spinner instead of its icon.
    pub(super) fn shows_spinner(&self) -> bool {
        // A failed load clears `loading` without a new state snapshot.
        self.state.loading
            && self
                .loading_since
                .is_some_and(|since| since.elapsed() >= SPINNER_DELAY)
    }

    pub(super) fn prepare_shutdown(&mut self) {
        if self.session.shutdown_started {
            return;
        }
        let final_releases = if let Some(presented) = self.external.as_ref() {
            let releases = presented.retired_until_submitted.clone();
            let mut pending = releases.lock().unwrap();
            if !pending.iter().any(|retired| retired.key == presented.key) {
                pending.push(presented.retire());
            }
            drop(pending);
            releases
        } else {
            Arc::new(Mutex::new(Vec::new()))
        };
        self.session.begin_shutdown(final_releases);
    }

    /// Lets pages follow the shell theme. Runs before the first viewport
    /// update, so the startup page loads with the right scheme.
    pub(super) fn update_color_scheme(&mut self, appearance: WindowAppearance) {
        let dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        if self.sent_dark_color_scheme != Some(dark) {
            self.session.set_dark_color_scheme(dark);
            self.sent_dark_color_scheme = Some(dark);
        }
    }

    /// Takes another tab's page size, so a tab opened in the background
    /// lays out and starts loading before it is first shown.
    pub(super) fn adopt_viewport(&mut self, viewport: Option<(i32, i32, u32)>) {
        if let Some((width, height, scale)) = viewport {
            self.update_viewport(width as f32, height as f32, f32::from_bits(scale));
        }
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

    pub(super) fn from_session(
        cx: &mut Context<Self>,
        theme: ThemePreference,
        session: EngineSession,
        is_blank_tab: bool,
    ) -> Self {
        Self {
            external: None,
            state: BrowserState::default(),
            diagnostics: PerformanceDiagnostics::default(),
            performance_overlay_enabled: false,
            crash_notice: None,
            crashes: PageCrashes::default(),
            recorded_url: None,
            context_menu: None,
            find_result: None,
            zoom_level: 1.0,
            page_unresponsive: false,
            favicon: None,
            audio_playing: false,
            audio_muted: false,
            loading_since: None,
            dialogs: PageDialogs::default(),
            theme,
            is_blank_tab,
            session,
            focus_handle: cx.focus_handle(),
            last_viewport: None,
            sent_dark_color_scheme: None,
            input: WebViewInput::default(),
            _quit_subscription: None,
            focus_subscriptions: Vec::new(),
            pending_new_webviews: HashMap::new(),
        }
    }

    pub(in crate::platform) fn queue_new_web_view(
        &mut self,
        id: u64,
        request: RequestedWebView,
        cx: &mut Context<Self>,
    ) {
        self.pending_new_webviews.insert(id, request);
        cx.emit(WebViewEvent::NewWebViewRequested(id));
        cx.notify();
    }

    pub(super) fn take_new_web_view(&mut self, id: u64) -> Option<RequestedWebView> {
        self.pending_new_webviews.remove(&id)
    }

    /// Keeps Engine page focus in step with GPUI focus and window activation.
    pub(super) fn track_engine_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_subscriptions = vec![
            cx.on_focus(&self.focus_handle, window, |this, window, _| {
                this.sync_engine_focus(window)
            }),
            cx.on_blur(&self.focus_handle, window, |this, window, _| {
                this.sync_engine_focus(window)
            }),
            cx.observe_window_activation(window, |this, window, _| this.sync_engine_focus(window)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
        ];
        self.sync_engine_focus(window);
    }

    fn sync_engine_focus(&mut self, window: &Window) {
        self.session
            .set_focus(window.is_window_active() && self.focus_handle.is_focused(window));
    }

    pub(super) fn set_performance_overlay_enabled(&mut self, enabled: bool) {
        self.performance_overlay_enabled = enabled;
        self.session.set_diagnostics_enabled(enabled);
    }

    /// Runs a page command, closing the page's dialog first when the command
    /// navigates away from it.
    pub(super) fn execute(
        &mut self,
        command: BrowserCommand,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if matches!(
            command,
            BrowserCommand::Navigate(_)
                | BrowserCommand::Reload
                | BrowserCommand::Back
                | BrowserCommand::Forward
        ) {
            let reply = self.dialogs.navigation_started();
            self.send_dialog_reply(reply, cx);
        }
        if self.crash_notice == Some(CrashNotice::KeepsCrashing) {
            if command == BrowserCommand::Reload {
                self.crashes.reload_requested();
                self.crash_notice = Some(CrashNotice::Reloading);
            } else {
                self.crash_notice = None;
            }
            self.state_changed(cx);
        }
        self.session.execute(command)
    }

    pub(super) fn is_blank_tab(&self) -> bool {
        self.is_blank_tab
    }

    /// Whether the tab shows a page. A new tab may load one behind its blank
    /// surface, whose state the chrome ignores until someone navigates.
    pub(super) fn has_page(&self) -> bool {
        !self.is_blank_tab && !self.state.url.is_empty() && self.state.url != "about:blank"
    }

    pub(super) fn omnibox_url(&self) -> String {
        if self.is_blank_tab {
            String::new()
        } else {
            self.state.url.clone()
        }
    }

    pub(super) fn navigate(&mut self, input: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let command = BrowserCommand::from_omnibox_input(input)
            .map_err(|error| anyhow::anyhow!("cannot open {input:?}: {error:?}"))?;
        self.execute(command, cx)?;
        if self.is_blank_tab {
            self.is_blank_tab = false;
            self.state_changed(cx);
        }
        Ok(())
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.theme.appearance(window.appearance());
        self.update_color_scheme(appearance);
        let palette = ThemeColors::for_appearance(appearance);
        let weak_this = cx.entity().downgrade();
        let weak_mouse_down = weak_this.clone();
        let weak_mouse_up = weak_this.clone();
        let mut webview = div()
            .size_full()
            .relative()
            .id("photon-webview-viewport")
            .on_prepaint(
                cx.listener(|this, event: &gpui::InteractivityPrepaint, window, cx| {
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
                    // Follow the window across displays so Engine paces to the
                    // one it is on rather than a 60 Hz default.
                    if let Some(display) = window.display(cx) {
                        this.session.set_display(display.id().into());
                    }
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
        if !self.is_blank_tab {
            webview = webview
                .rounded(px(WEBVIEW_CORNER_RADIUS))
                .bg(gpui::rgb(palette.page_background));
        }
        if !self.is_blank_tab
            && let Some(presented) = self.external.as_ref()
        {
            webview = webview.child(
                surface(SurfaceSource::ExternalMetal(presented.surface.clone()))
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .rounded(px(WEBVIEW_CORNER_RADIUS)),
            );
        }
        if self.performance_overlay_enabled {
            webview = webview.child(performance_overlay(
                &self.diagnostics,
                palette.performance_palette,
            ));
        }
        webview
    }
}
