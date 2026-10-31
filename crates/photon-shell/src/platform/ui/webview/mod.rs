//! The page surface, its tab state, and browser input forwarding.

use super::super::engine::{EngineSession, RequestedWebView};
use super::input::{EditCommand, WebViewInput};
use super::{
    metrics,
    settings::Settings,
    theme::{page_palette, palette},
};
use gpui::{
    AnyWindowHandle, Context, CursorStyle, EventEmitter, FocusHandle, InteractiveElement,
    KeyDownEvent, KeyUpEvent, MouseButton, NavigationDirection, Render, Subscription, Task, Window,
    WindowAppearance, div, prelude::*, px,
};
use gpui_elements::editable_text::actions::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use photon_core::{BrowserCommand, BrowserState, EngineEvent, PageCrashes, PageDialogs};
use photon_performance::PerformanceDiagnostics;
use photon_performance_ui::performance_overlay;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::platform::PagePlacement;
use crate::platform::renderer::{PagePresenter, PresentedSurface};
use std::time::{Duration, Instant};

use super::super::trace;

/// How long the viewport must stay still before Engine lays it out again.
const RESIZE_SETTLE_DELAY: Duration = Duration::from_millis(120);

#[cfg(unix)]
mod automation;
mod context_menu;
mod crashes;
mod dialogs;
mod favicon;
mod find;
mod form_controls;
mod fullscreen;
mod history;
mod history_swipe;
mod link_status;
mod permissions;
mod text_input;
mod zoom;

pub(in crate::platform) use context_menu::{PageMenu, PageMenuItem};
pub(in crate::platform) use crashes::CrashNotice;
pub(in crate::platform) use favicon::Favicon;
pub(in crate::platform) use find::FindResult;
pub(in crate::platform) use form_controls::{SelectDropdown, SelectItem};
pub(in crate::platform) use text_input::InputMethodState;

static MOUSE_MOVE_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(in crate::platform) struct PhotonWebView {
    pub(super) external: Option<PresentedSurface>,
    presenter: PagePresenter,
    navigation_surface_pending_render: Option<u64>,
    pub(in crate::platform) state: BrowserState,
    pub(in crate::platform) diagnostics: PerformanceDiagnostics,
    pub(in crate::platform) performance_overlay_enabled: bool,
    #[cfg(unix)]
    performance_diagnostics_session_active: bool,
    /// How this tab presents its last crash and recovery.
    pub(in crate::platform) crash_notice: Option<CrashNotice>,
    crashes: PageCrashes,
    /// The page's zoom level as a factor, 1.0 being 100%.
    zoom_level: f64,
    /// Whether page input has gone unanswered by WebContent.
    pub(in crate::platform) page_unresponsive: bool,
    /// The pointer style currently requested by the page.
    page_cursor: CursorStyle,
    /// What the tab last recorded in the browsing history.
    recorded: history::Recorded,
    /// The context menu the page last asked for.
    pub(in crate::platform) context_menu: Option<PageMenu>,
    /// The `<select>` menu waiting for the browser window to present it.
    select_dropdown_request: Option<SelectDropdown>,
    /// The latest result of a find-in-page search.
    pub(in crate::platform) find_result: Option<FindResult>,
    /// Whether a search is under way, so results arriving after it ended are dropped.
    find_active: bool,
    /// Find requests sent whose result has not arrived. The page answers each
    /// in order, so only the answer to the latest, at zero, is current.
    find_pending: usize,
    /// The page's icon, shown in its tab while the page is not loading.
    pub(in crate::platform) favicon: Option<Favicon>,
    /// Whether the page is playing audio and whether its tab is muted.
    pub(in crate::platform) audio_playing: bool,
    pub(in crate::platform) audio_muted: bool,
    /// The sites this tab has allowed to use the microphone.
    microphone_allowed: std::collections::HashSet<String>,
    /// When the current load started.
    loading_since: Option<Instant>,
    /// Redraws the tab once the spinner delay passes, as the spinner's own
    /// animation timer does not run when animations are off.
    spinner_reveal: Option<Task<()>>,
    /// The page's JavaScript dialog, which the window shows over the page.
    pub(in crate::platform) dialogs: PageDialogs,
    pub(super) session: EngineSession,
    pub(super) focus_handle: FocusHandle,
    pub(super) last_viewport: Option<(i32, i32, u32)>,
    /// The latest size waiting for a continuous resize to settle.
    deferred_viewport: Option<(f32, f32, f32)>,
    /// Sends the latest viewport once a continuous resize settles.
    resize_settle_task: Option<Task<()>>,
    /// The page's own background colour as 0xRRGGBB, once Engine reports it.
    page_background: Option<u32>,
    /// The window the page was last drawn in, where its menus open.
    window: Option<AnyWindowHandle>,
    /// The color scheme last sent to Engine, `true` for dark.
    sent_dark_color_scheme: Option<bool>,
    /// Whether force dark mode has been sent to Engine.
    sent_force_dark_mode: Option<bool>,
    /// The link under the pointer, shown over the page.
    hovered_link: Option<String>,
    /// The page has an element in fullscreen.
    page_fullscreen: bool,
    /// The size the page is laid out at, when it is scaled to its bounds
    /// rather than laid out at them.
    layout_size: Option<gpui::Size<gpui::Pixels>>,
    /// How opaque the page is drawn, as while a preview fades.
    page_opacity: f32,
    /// Opacity of the complete tab surface during a tab switch.
    tab_switch_opacity: f32,
    /// Whether this page is the incoming layer in a tab cross-fade.
    tab_switch_on_top: bool,
    /// How much larger than its bounds the page stays laid out while the
    /// chrome around it moves, so Engine lays it out once, not every frame.
    layout_growth: gpui::Size<f32>,
    input: WebViewInput,
    /// Text from the platform's input methods, such as accents and CJK input.
    text_input: text_input::TextInput,
    history_swipe: history_swipe::HistorySwipe,
    pub(super) _quit_subscription: Option<Subscription>,
    focus_subscriptions: Vec<Subscription>,
    pending_new_webviews: HashMap<u64, RequestedWebView>,
    /// The page the tab was opened to show, by address and title, which
    /// stands for the page until it loads, as for a tab reopened in the
    /// background that has not been shown yet.
    pub(in crate::platform) requested: Option<photon_core::Shortcut>,
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
    /// A link was shift-clicked, to show in a preview over the page.
    Peek(String),
    /// The page asked the shell to show a styled `<select>` popup.
    SelectDropdownRequested,
    /// The page entered or left element fullscreen.
    FullscreenChanged(bool),
}

impl EventEmitter<WebViewEvent> for PhotonWebView {}

impl Drop for PhotonWebView {
    fn drop(&mut self) {
        self.prepare_shutdown();
    }
}

impl PhotonWebView {
    #[cfg(unix)]
    pub(in crate::platform) fn registered_engine_processes(
        &self,
    ) -> Vec<super::super::engine::RegisteredEngineProcess> {
        self.session.registered_engine_processes()
    }

    /// Runs an editing command in the page.
    fn edit(&mut self, command: EditCommand) {
        self.input.edit(&mut self.session, command);
    }
}

impl PhotonWebView {
    pub(in crate::platform) fn present_latest(&mut self, cx: &mut Context<Self>) {
        let Some(presented) = self.session.take_presented_surface() else {
            return;
        };
        // Keep the laid-out frame while the viewport is still changing. The
        // settle timer sends only the final dimensions after the resize ends.
        let navigation_id = presented.navigation_id;
        let (generation, frame_id) = presented.timing_identity();
        self.session.record_surface_install(generation, frame_id);
        self.external = Some(crate::platform::renderer::replace_surface(
            self.external.take(),
            presented,
        ));
        if let Some(navigation_id) = navigation_id {
            self.session
                .record_navigation_surface_installed(navigation_id);
            self.navigation_surface_pending_render = Some(navigation_id);
        }
        let Some(presented) = self.external.as_mut() else {
            return;
        };
        if self.presenter.show(presented, &self.session.presentation) {
            if let Some(navigation_id) = self.navigation_surface_pending_render.take() {
                self.session
                    .record_navigation_surface_rendered(navigation_id);
            }
        } else {
            cx.notify();
        }
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
        if state.url != self.state.url {
            // The new page reports its own hover once the pointer moves.
            self.hovered_link = None;
            if self.history_swipe.phase == history_swipe::HistorySwipePhase::Committing {
                self.history_swipe_navigation_committed();
            } else if self.history_swipe.phase == history_swipe::HistorySwipePhase::Tracking {
                self.reset_history_swipe(true);
            }
        }
        if state.loading != self.state.loading {
            self.loading_since = state.loading.then(Instant::now);
            self.spinner_reveal = state.loading.then(|| self.reveal_spinner_later(cx));
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

    pub(in crate::platform) fn set_page_cursor(
        &mut self,
        cursor: CursorStyle,
        cx: &mut Context<Self>,
    ) {
        if self.page_cursor == cursor {
            return;
        }
        self.page_cursor = cursor;
        cx.notify();
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
    pub(super) fn shows_spinner(&self, animations: &photon_core::AnimationSettings) -> bool {
        // A failed load clears `loading` without a new state snapshot.
        self.state.loading
            && self.loading_since.is_some_and(|since| {
                since.elapsed()
                    >= Duration::from_millis(u64::from(animations.loading_spinner_delay_ms()))
            })
    }

    /// Redraws once the spinner delay has passed, if the page is still loading.
    fn reveal_spinner_later(&self, cx: &mut Context<Self>) -> Task<()> {
        let delay = Duration::from_millis(u64::from(
            Settings::get(cx).animations.loading_spinner_delay_ms(),
        ));
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(delay).await;
            let _ = view.update(cx, |view, cx| {
                view.spinner_reveal = None;
                if view.state.loading {
                    view.state_changed(cx);
                }
            });
        })
    }

    pub(super) fn prepare_shutdown(&mut self) {
        if self.session.shutdown_started {
            return;
        }
        self.presenter.detach();
        self.session
            .begin_shutdown_with_surface(self.external.take());
    }

    /// Lets pages follow the shell theme. Runs before the first viewport
    /// update, so the startup page loads with the right scheme.
    pub(super) fn update_color_scheme(&mut self, appearance: WindowAppearance) {
        let dark = crate::platform::is_dark_appearance(appearance);
        if self.sent_dark_color_scheme != Some(dark) {
            self.session.set_dark_color_scheme(dark);
            self.sent_dark_color_scheme = Some(dark);
        }
    }

    /// Applies the profile's force dark mode preference to the page.
    pub(super) fn update_force_dark_mode(&mut self, enabled: bool) {
        if self.sent_force_dark_mode != Some(enabled) {
            self.session.set_force_dark_mode_enabled(enabled);
            self.sent_force_dark_mode = Some(enabled);
        }
    }

    /// Uses the page's own background where it has not painted.
    pub(in crate::platform) fn set_page_background(&mut self, rgb: u32, cx: &mut Context<Self>) {
        if self.page_background != Some(rgb) {
            self.page_background = Some(rgb);
            cx.notify();
        }
    }

    /// Takes another tab's page size, so a tab opened in the background
    /// lays out and starts loading before it is first shown.
    pub(super) fn adopt_viewport(
        &mut self,
        viewport: Option<(i32, i32, u32)>,
        cx: &mut Context<Self>,
    ) {
        if let Some((width, height, scale)) = viewport {
            self.update_viewport(width as f32, height as f32, f32::from_bits(scale), cx);
        }
    }

    fn update_viewport(&mut self, width: f32, height: f32, scale: f32, cx: &mut Context<Self>) {
        let width = width.round() as i32;
        let height = height.round() as i32;
        let viewport = (width, height, scale.to_bits());
        if self.last_viewport == Some(viewport) || width <= 0 || height <= 0 {
            if self.last_viewport == Some(viewport) {
                self.deferred_viewport = None;
                self.resize_settle_task = None;
            }
            return;
        }

        let is_initial_viewport = self.last_viewport.is_none();
        let scale_changed = self
            .last_viewport
            .is_some_and(|(_, _, last_scale)| last_scale != viewport.2);
        if !is_initial_viewport && !scale_changed {
            self.deferred_viewport = Some((width as f32, height as f32, scale));
            self.resize_settle_task = Some(cx.spawn(async move |view, cx| {
                cx.background_executor().timer(RESIZE_SETTLE_DELAY).await;
                let _ = view.update(cx, |view, _| {
                    view.resize_settle_task = None;
                    if let Some((width, height, scale)) = view.deferred_viewport.take() {
                        view.send_viewport(width, height, scale);
                    }
                });
            }));
            return;
        }
        self.deferred_viewport = None;
        self.resize_settle_task = None;
        self.send_viewport(width as f32, height as f32, scale);
    }

    fn send_viewport(&mut self, width: f32, height: f32, scale: f32) {
        let width = width.round() as i32;
        let height = height.round() as i32;
        let viewport = (width, height, scale.to_bits());
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

    pub(super) fn from_session(cx: &mut Context<Self>, session: EngineSession) -> Self {
        Self {
            external: None,
            presenter: PagePresenter::new(),
            navigation_surface_pending_render: None,
            state: BrowserState::default(),
            diagnostics: PerformanceDiagnostics::default(),
            performance_overlay_enabled: false,
            #[cfg(unix)]
            performance_diagnostics_session_active: false,
            crash_notice: None,
            crashes: PageCrashes::default(),
            recorded: history::Recorded::default(),
            context_menu: None,
            select_dropdown_request: None,
            find_result: None,
            find_active: false,
            find_pending: 0,
            zoom_level: 1.0,
            page_unresponsive: false,
            page_cursor: CursorStyle::Arrow,
            favicon: None,
            audio_playing: false,
            audio_muted: false,
            microphone_allowed: std::collections::HashSet::new(),
            loading_since: None,
            spinner_reveal: None,
            dialogs: PageDialogs::default(),
            window: None,
            deferred_viewport: None,
            resize_settle_task: None,
            page_background: None,
            session,
            focus_handle: cx.focus_handle(),
            last_viewport: None,
            requested: None,
            sent_dark_color_scheme: None,
            sent_force_dark_mode: None,
            hovered_link: None,
            page_fullscreen: false,
            layout_size: None,
            page_opacity: 1.0,
            tab_switch_opacity: 1.0,
            tab_switch_on_top: false,
            layout_growth: gpui::Size::default(),
            input: WebViewInput::default(),
            text_input: text_input::TextInput::default(),
            history_swipe: history_swipe::HistorySwipe::default(),
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
    pub(in crate::platform) fn track_engine_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_subscriptions = vec![
            cx.on_focus(&self.focus_handle, window, |this, window, _| {
                this.sync_engine_focus(window)
            }),
            cx.on_blur(&self.focus_handle, window, |this, window, cx| {
                this.end_composition();
                this.sync_engine_focus(window);
                this.reset_history_swipe(true);
                cx.notify();
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                this.sync_engine_focus(window);
                if !window.is_window_active() {
                    this.reset_history_swipe(true);
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                let appearance = Settings::appearance(window.appearance(), cx);
                this.update_color_scheme(appearance);
                cx.notify();
            }),
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

    #[cfg(unix)]
    pub(super) fn start_task_queue_diagnostics(&mut self) -> Result<(), String> {
        if self.performance_diagnostics_session_active {
            return Err("task queue diagnostics are already active for this tab".to_owned());
        }
        if !self.session.set_task_queue_diagnostics_enabled(true) {
            return Err(
                "a WebContent process hosting this tab already has another diagnostics session, or the tab has no active content process".to_owned(),
            );
        }
        self.session.start_frame_timing_diagnostics();
        self.performance_diagnostics_session_active = true;
        Ok(())
    }

    #[cfg(unix)]
    pub(super) fn task_queue_diagnostics_json(&self) -> Result<String, String> {
        if !self.performance_diagnostics_session_active {
            return Err("task queue diagnostics are not active for this tab".to_owned());
        }
        self.session
            .task_queue_diagnostics_json()
            .map_err(|error| format!("cannot read Engine task queue diagnostics: {error:#}"))
    }

    #[cfg(unix)]
    pub(super) fn stop_task_queue_diagnostics(&mut self) -> Result<String, String> {
        self.session.stop_frame_timing_diagnostics();
        let snapshot = self.task_queue_diagnostics_json();
        let _ = self.session.set_task_queue_diagnostics_enabled(false);
        self.performance_diagnostics_session_active = false;
        snapshot
    }

    #[cfg(unix)]
    pub(super) fn frame_timing_diagnostics_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.session.frame_timing_diagnostics_json())
            .map_err(|error| format!("cannot serialize Photon frame diagnostics: {error}"))
    }

    /// Runs a page command, closing the page's dialog first when the command
    /// navigates away from it.
    pub(super) fn execute(
        &mut self,
        command: BrowserCommand,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if self.history_swipe.phase == history_swipe::HistorySwipePhase::Tracking {
            self.reset_history_swipe(true);
        }
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
        if matches!(
            self.crash_notice.as_ref(),
            Some(CrashNotice::KeepsCrashing { .. })
        ) {
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

    /// Whether the tab shows a page. A new tab may load one behind its blank
    /// surface, whose state the chrome ignores until someone navigates.
    pub(super) fn has_page(&self) -> bool {
        !self.state.url.is_empty() && self.state.url != "about:blank"
    }

    /// Records that the user allowed `origin` the microphone when it asked.
    pub(in crate::platform) fn microphone_allowed(&mut self, origin: &str) {
        if let Some(origin) = photon_core::site_origin(origin) {
            self.microphone_allowed.insert(origin);
        }
    }

    /// Whether the site showing now may use the microphone; `None` for a page
    /// with no site of its own.
    pub(super) fn microphone_allowed_here(&self) -> Option<bool> {
        photon_core::site_origin(&self.state.url)
            .map(|origin| self.microphone_allowed.contains(&origin))
    }

    /// Allows or blocks the microphone for the site showing now. Blocking ends
    /// capture it has running, and it is not asked again until allowed.
    pub(super) fn set_microphone_allowed_here(&mut self, allowed: bool, cx: &mut Context<Self>) {
        let Some(origin) = photon_core::site_origin(&self.state.url) else {
            return;
        };
        self.session.set_microphone_permission(&origin, allowed);
        if allowed {
            self.microphone_allowed.insert(origin);
        } else {
            self.microphone_allowed.remove(&origin);
        }
        self.state_changed(cx);
    }

    pub(super) fn omnibox_url(&self) -> String {
        self.state.url.clone()
    }

    pub(in crate::platform) fn viewport_origin(&self) -> (f32, f32) {
        self.input.viewport_origin()
    }

    pub(in crate::platform) fn take_select_dropdown_request(&mut self) -> Option<SelectDropdown> {
        self.select_dropdown_request.take()
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = Settings::appearance(window.appearance(), cx);
        self.update_color_scheme(appearance);
        let palette = palette(window, cx);
        // Shown where the page has not painted, as while it lays out at a new size.
        let page_background = self.page_background.unwrap_or(palette.page_background);
        // A fullscreen page meets the screen's edges.
        let radius = if self.page_fullscreen {
            0.0
        } else {
            metrics::page_radius()
        };
        let weak_this = cx.entity().downgrade();
        let weak_mouse_down = weak_this.clone();
        let weak_mouse_up = weak_this.clone();
        let mut webview = div()
            .size_full()
            .relative()
            .id("photon-webview-viewport")
            .cursor(self.page_cursor)
            .on_prepaint(
                cx.listener(move |this, event: &gpui::InteractivityPrepaint, window, cx| {
                    // Resize from the laid-out WebView bounds, not the native window
                    // bounds minus assumed titlebar/padding values. During live resize,
                    // those assumptions can differ by a frame and offset the page.
                    this.window = Some(window.window_handle());
                    this.input.set_viewport_origin(
                        f32::from(event.bounds.origin.x),
                        f32::from(event.bounds.origin.y),
                    );
                    let scale_factor = window.scale_factor();
                    let layout_size = this.layout_size.unwrap_or_else(|| gpui::Size {
                        width: event.bounds.size.width + px(this.layout_growth.width),
                        height: event.bounds.size.height + px(this.layout_growth.height),
                    });
                    this.update_viewport(
                        f32::from(layout_size.width),
                        f32::from(layout_size.height),
                        scale_factor,
                        cx,
                    );
                    this.presenter.place(
                        window,
                        PagePlacement {
                            bounds: event.bounds,
                            corner_radius: radius,
                            corner_smoothing: metrics::page_smoothing(),
                            scale_factor,
                            background: page_background,
                            scaled: this.layout_size.is_some(),
                            opacity: this.page_opacity,
                            layer_opacity: this.tab_switch_opacity,
                            layer_on_top: this.tab_switch_on_top,
                        },
                    );
                    // Follow the window across displays so Engine paces to the
                    // one it is on rather than a 60 Hz default.
                    if let Some(display) = window.display(cx) {
                        this.session.set_display(display.id().into());
                    }
                }),
            )
            .track_focus(&self.focus_handle)
            // The Edit menu's commands, chosen while the page has focus.
            .on_action(cx.listener(|this, _: &Undo, _, _| this.edit(EditCommand::Undo)))
            .on_action(cx.listener(|this, _: &Redo, _, _| this.edit(EditCommand::Redo)))
            .on_action(cx.listener(|this, _: &Cut, _, _| this.edit(EditCommand::Cut)))
            .on_action(cx.listener(|this, _: &Copy, _, _| this.edit(EditCommand::Copy)))
            .on_action(cx.listener(|this, _: &Paste, _, _| this.edit(EditCommand::Paste)))
            .on_action(cx.listener(|this, _: &SelectAll, _, _| this.edit(EditCommand::SelectAll)))
            .on_mouse_down_all(move |event, phase, hitbox, window, app| {
                // Hovered rather than contained: a preview or menu over the page
                // occludes it, and what happens there is not the page's.
                if phase != gpui::DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                    return;
                }
                weak_mouse_down
                    .update(app, |this, cx| {
                        window.focus(&this.focus_handle, cx);
                        match event.button {
                            MouseButton::Navigate(NavigationDirection::Back)
                                if this.state.can_go_back =>
                            {
                                let _ = this.execute(BrowserCommand::Back, cx);
                            }
                            MouseButton::Navigate(NavigationDirection::Forward)
                                if this.state.can_go_forward =>
                            {
                                let _ = this.execute(BrowserCommand::Forward, cx);
                            }
                            _ => this.input.mouse_down(&mut this.session, event),
                        }
                    })
                    .ok();
            })
            .on_mouse_up_all(move |event, phase, hitbox, window, app| {
                if phase != gpui::DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                    return;
                }
                weak_mouse_up
                    .update(app, |this, _cx| {
                        this.input.mouse_up(&mut this.session, event)
                    })
                    .ok();
            })
            .on_mouse_move_all(move |event, phase, hitbox, window, app| {
                // Forward pointer motion during capture so child elements in the
                // external surface cannot stop propagation before the page sees it.
                if phase != gpui::DispatchPhase::Capture {
                    return;
                }
                let move_count = MOUSE_MOVE_TRACE_COUNT.fetch_add(1, Ordering::Relaxed);
                let inside_viewport = hitbox.is_hovered(window);
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
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                let gesture_id = this.input.history_swipe_gesture_id(event);
                let captured = this.handle_history_swipe_scroll(event, gesture_id, cx);
                this.input.scroll(
                    &mut this.session,
                    event,
                    this.state.can_go_back,
                    this.state.can_go_forward,
                    gesture_id,
                    captured,
                );
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.fullscreen_key_down(event) {
                    cx.stop_propagation();
                    return;
                }
                if let Some(text) = text_input::TextInput::text_for_key(event) {
                    this.session.commit_text(text, 0, 0);
                    cx.stop_propagation();
                    return;
                }
                let should_consume = this.input.key_down(&mut this.session, event);
                this.text_input.key_pressed(event);
                // Consume commands after forwarding so editing keys such as Backspace
                // are not replayed by the native text input system.
                if should_consume {
                    cx.stop_propagation();
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                this.text_input.key_released();
                this.input.key_up(&mut this.session, event);
            }));
        webview = webview
            .rounded(px(radius))
            .rounded_smoothing(metrics::page_smoothing())
            .overflow_hidden()
            .bg(gpui::rgb(page_background));
        {
            webview = webview.child(crate::platform::renderer::page_surface(
                self.external.as_ref(),
                crate::platform::PageSurfacePalette {
                    background: page_background,
                },
            ));
            if let Some(navigation_id) = self.navigation_surface_pending_render.take() {
                self.session
                    .record_navigation_surface_rendered(navigation_id);
            }
        }
        webview = webview.child(text_input::input_handler_element(
            self.focus_handle.clone(),
            cx.entity(),
        ));
        if let Some(status) = self.link_status(palette) {
            webview = webview.child(status);
        }
        if let Some(indicator) = self.history_swipe_indicator(palette) {
            webview = webview.child(indicator);
        }
        if self.performance_overlay_enabled {
            webview = webview.child(performance_overlay(
                &self.diagnostics,
                palette.performance_palette,
            ));
        }
        if let Some(overlay) = self.crash_overlay(page_palette(window, cx), radius, cx) {
            webview = webview.child(overlay);
        }
        #[cfg(not(target_os = "macos"))]
        {
            webview = webview.opacity(self.tab_switch_opacity * self.page_opacity);
        }
        webview
    }
}

impl PhotonWebView {
    /// Lays the page out at `size` and scales it to wherever it is drawn, so
    /// moving between sizes, as a preview growing into place, never makes the
    /// page lay out again. `None` lays it out at its drawn size again.
    pub(in crate::platform) fn set_layout_size(&mut self, size: Option<gpui::Size<gpui::Pixels>>) {
        self.layout_size = size;
    }

    /// Keeps the page laid out `growth` larger than where it is drawn, and
    /// clipped there, while the chrome moves its edges.
    pub(in crate::platform) fn set_layout_growth(&mut self, growth: gpui::Size<f32>) {
        self.layout_growth = growth;
    }

    /// Fades the page's native layer, which GPUI's opacity cannot reach.
    pub(in crate::platform) fn set_page_opacity(&mut self, opacity: f32) {
        self.page_opacity = opacity;
    }

    /// Cross-fades this tab's complete surface over the page beneath it.
    pub(in crate::platform) fn set_tab_switch_transition(&mut self, opacity: f32, incoming: bool) {
        self.tab_switch_opacity = opacity.clamp(0.0, 1.0);
        self.tab_switch_on_top = incoming;
        self.presenter
            .set_layer_opacity(self.tab_switch_opacity * self.page_opacity);
    }

    /// Pauses Engine rendering while keeping the last page layer visible for
    /// the short tab cross-fade.
    pub(in crate::platform) fn pause_page_for_transition(&mut self) {
        self.session.set_visible(false);
    }

    /// Shows or hides the page: Engine pauses rendering while hidden, and a
    /// native page layer hides so it cannot show through what replaces it.
    pub(in crate::platform) fn set_page_visible(&mut self, visible: bool) {
        self.session.set_visible(visible);
        if !visible {
            self.presenter.hide();
        }
    }
}
