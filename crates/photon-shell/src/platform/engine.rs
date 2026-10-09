//! Engine runtime, its C callbacks, and input forwarding.

use anyhow::Context as _;
use gpui::{AsyncApp, Context, WeakEntity};
use mach2::port::mach_port_t;
use photon_core::{BrowserCommand, BrowserDiagnostics, BrowserState, EngineEvent};
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char, c_void},
    rc::Rc,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicPtr, Ordering},
    },
    time::Instant,
};

use super::presentation::{
    GpuActivity, LeaseLedger, MachPortGuard, PresentationRuntime, RetiredSurface,
};
use super::ui::PhotonWebView;
use super::{ffi::embedder, trace};

#[derive(Clone)]
pub(super) struct UiWake {
    pub(super) app: AsyncApp,
    pub(super) webview: WeakEntity<PhotonWebView>,
}

struct CallbackState {
    pub(super) presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    engine_view: AtomicPtr<c_void>,
    ui_wake: Mutex<Option<UiWake>>,
    diagnostics: Mutex<DiagnosticAccumulator>,
    diagnostics_enabled: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
struct DiagnosticAccumulator {
    snapshot: BrowserDiagnostics,
    last_input_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame_gaps: VecDeque<(Instant, f64)>,
}

struct RuntimeCallbacks {
    views: Mutex<Vec<Weak<CallbackState>>>,
}

pub(super) struct EngineRuntime {
    runtime: *mut c_void,
    reduced_motion_observer: *mut c_void,
    callbacks: Box<RuntimeCallbacks>,
}

impl CallbackState {
    pub(super) fn set_ui_wake(&self, wake: UiWake) {
        *self.ui_wake.lock().unwrap() = Some(wake);
        self.request_redraw();
    }

    fn request_redraw(&self) {
        let diagnostics = self
            .diagnostics_enabled
            .load(Ordering::Acquire)
            .then(|| self.diagnostics.lock().unwrap().snapshot.clone());
        self.update_webview(move |view, cx| {
            view.present_latest(cx);
            if let Some(diagnostics) = diagnostics
                && view.diagnostics != diagnostics
            {
                view.diagnostics = diagnostics;
                cx.notify();
            }
        });
    }

    fn set_page_state(&self, state: BrowserState) {
        self.update_webview(move |view, cx| {
            if view.state != state {
                view.state.apply(EngineEvent::ViewStateChanged(state));
                cx.notify();
            }
        });
    }

    fn set_page_error(&self, message: String) {
        self.update_webview(move |view, cx| {
            view.state.apply(EngineEvent::LoadFailed(message));
            cx.notify();
        });
    }

    fn handle_engine_crash(&self, url: String) {
        self.update_webview(move |view, cx| {
            if let Err(error) = view.handle_engine_crash() {
                eprintln!("Photon Engine: failed to reload crashed page {url}: {error:#}");
            }
            cx.notify();
        });
    }

    fn mark_input(&self) {
        self.diagnostics.lock().unwrap().last_input_at = Some(Instant::now());
    }

    fn record_frame(&self) {
        let now = Instant::now();
        {
            let mut diagnostics = self.diagnostics.lock().unwrap();
            if let Some(previous) = diagnostics.last_frame_at {
                let interval_ms = now.duration_since(previous).as_secs_f64() * 1000.0;
                diagnostics.snapshot.last_frame_interval_ms = Some(interval_ms);
                while diagnostics
                    .frame_gaps
                    .front()
                    .is_some_and(|(at, _)| now.duration_since(*at).as_secs_f64() > 10.0)
                {
                    diagnostics.frame_gaps.pop_front();
                }
                if interval_ms <= 10_000.0 {
                    diagnostics.frame_gaps.push_back((now, interval_ms));
                }
                diagnostics.snapshot.longest_frame_gap_ms = diagnostics
                    .frame_gaps
                    .iter()
                    .map(|(_, gap)| *gap)
                    .max_by(f64::total_cmp);
            }
            diagnostics.last_frame_at = Some(now);
            if let Some(input_at) = diagnostics.last_input_at.take() {
                diagnostics.snapshot.input_to_frame_latency_ms =
                    Some(now.duration_since(input_at).as_secs_f64() * 1000.0);
            }
        }
    }

    fn set_engine_diagnostics(&self, stats: &embedder::PerformanceStats) {
        {
            let mut diagnostics = self.diagnostics.lock().unwrap();
            diagnostics.snapshot.cpu_percent = stats.has_cpu_percent.then_some(stats.cpu_percent);
            diagnostics.snapshot.memory_bytes =
                stats.has_memory_bytes.then_some(stats.memory_bytes);
            diagnostics.snapshot.managed_heap_bytes = stats
                .has_managed_heap_bytes
                .then_some(stats.managed_heap_bytes);
            diagnostics.snapshot.download_bytes_per_second = stats.download_bytes_per_second;
            diagnostics.snapshot.upload_bytes_per_second = stats.upload_bytes_per_second;
            diagnostics.snapshot.frames_per_second = stats
                .has_frames_per_second
                .then_some(stats.frames_per_second);
        }
        if self.diagnostics_enabled.load(Ordering::Acquire) {
            self.publish_diagnostics();
        }
    }

    fn set_diagnostics_enabled(&self, enabled: bool) {
        self.diagnostics_enabled.store(enabled, Ordering::Release);
        if enabled {
            self.publish_diagnostics();
        }
    }

    fn publish_diagnostics(&self) {
        let snapshot = self.diagnostics.lock().unwrap().snapshot.clone();
        self.update_webview(move |view, cx| {
            if view.diagnostics != snapshot {
                view.diagnostics = snapshot;
                cx.notify();
            }
        });
    }

    /// Runs `update` on the UI thread against the live WebView, then refreshes the window.
    fn update_webview(
        &self,
        update: impl FnOnce(&mut PhotonWebView, &mut Context<PhotonWebView>) + 'static,
    ) {
        let Some(wake) = self.ui_wake.lock().unwrap().clone() else {
            return;
        };
        wake.app
            .spawn(async move |cx| {
                let Some(webview) = wake.webview.upgrade() else {
                    return;
                };
                webview.update(cx, update);
                cx.refresh();
            })
            .detach();
    }
}

impl EngineRuntime {
    pub(super) fn create() -> anyhow::Result<Self> {
        let helper = std::env::var("PHOTON_HELPER_DIRECTORY")
            .context("PHOTON_HELPER_DIRECTORY is not set")?;
        let helper = CString::new(helper)?;
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(helper.as_ptr(), error.as_mut_ptr(), error.len())
        };
        anyhow::ensure!(
            !runtime.is_null(),
            "Photon Engine runtime failed: {}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        let mut callbacks = Box::new(RuntimeCallbacks {
            views: Mutex::new(Vec::new()),
        });
        let reduced_motion_observer = unsafe {
            embedder::photon_reduced_motion_observer_create(
                runtime,
                Some(on_system_reduced_motion_changed),
            )
        };
        unsafe {
            embedder::photon_runtime_set_native_release_drain_callback(
                runtime,
                (&mut *callbacks as *mut RuntimeCallbacks).cast(),
                Some(on_native_release_drain),
            );
        }
        Ok(Self {
            runtime,
            reduced_motion_observer,
            callbacks,
        })
    }
}

impl Drop for EngineRuntime {
    fn drop(&mut self) {
        unsafe {
            if !self.runtime.is_null() {
                embedder::photon_runtime_set_native_release_drain_callback(
                    self.runtime,
                    std::ptr::null_mut(),
                    None,
                );
            }
            if !self.reduced_motion_observer.is_null() {
                embedder::photon_reduced_motion_observer_destroy(self.reduced_motion_observer);
                self.reduced_motion_observer = std::ptr::null_mut();
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
                self.runtime = std::ptr::null_mut();
            }
        }
    }
}

fn deliver_pending_releases(callbacks: &CallbackState) {
    let view = callbacks.engine_view.load(Ordering::Acquire);
    if view.is_null() {
        return;
    }
    let pending = callbacks.leases.lock().unwrap().take_pending();
    for release in pending {
        unsafe {
            embedder::photon_view_release_native_frame(
                view,
                release.key.backing,
                release.key.generation,
                release.key.frame,
            );
        }
        trace(format_args!(
            "FrameReleased frame={} gen={} delivered-to-engine",
            release.key.frame, release.key.generation
        ));
        callbacks.leases.lock().unwrap().release(release.key);
    }
}

pub(super) struct EngineSession {
    _runtime: Rc<EngineRuntime>,
    view: *mut c_void,
    startup_url: Option<CString>,
    callbacks: Arc<CallbackState>,
    pub(super) presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    pub(super) gpu_activity: Arc<GpuActivity>,
    final_surface_releases: Option<Arc<Mutex<Vec<RetiredSurface>>>>,
    display: Option<u64>,
    pub(super) shutdown_started: bool,
    finished: bool,
}

impl EngineSession {
    pub(super) fn create(
        runtime: Rc<EngineRuntime>,
        width: i32,
        height: i32,
        dpr: f64,
        startup_address: Option<&str>,
    ) -> anyhow::Result<Self> {
        let startup_url = startup_address
            .map(|address| resolve_address(address).context("invalid startup URL"))
            .transpose()?;
        let service = std::env::var("PHOTON_PRESENTATION_XPC_SERVICE")
            .context("presentation broker service is not set")?;
        let channel_id = std::env::var("PHOTON_PRESENTATION_CHANNEL_ID")
            .context("presentation channel is not set")?;
        let leases = Arc::new(Mutex::new(LeaseLedger::default()));
        let gpu_activity = Arc::new(GpuActivity::default());
        let presentation = Arc::new(PresentationRuntime::new(
            &service,
            channel_id,
            leases.clone(),
            gpu_activity.clone(),
        )?);
        presentation
            .release_scheduler
            .store(runtime.runtime, Ordering::Release);
        let callbacks = Arc::new(CallbackState {
            presentation: presentation.clone(),
            leases: leases.clone(),
            engine_view: AtomicPtr::new(std::ptr::null_mut()),
            ui_wake: Mutex::new(None),
            diagnostics: Mutex::new(DiagnosticAccumulator::default()),
            diagnostics_enabled: std::sync::atomic::AtomicBool::new(false),
        });
        runtime
            .callbacks
            .views
            .lock()
            .unwrap()
            .push(Arc::downgrade(&callbacks));
        let view = unsafe {
            embedder::photon_view_create(
                runtime.runtime,
                width,
                height,
                dpr,
                Arc::as_ptr(&callbacks).cast_mut().cast(),
                Some(on_engine_state),
                Some(on_engine_frame),
                Some(on_engine_cursor),
                Some(on_engine_error),
                Some(on_engine_crash),
                Some(on_engine_performance_stats),
                true,
                Some(on_engine_backing),
                Some(on_engine_native_frame),
            )
        };
        if view.is_null() {
            presentation
                .release_scheduler
                .store(std::ptr::null_mut(), Ordering::Release);
            anyhow::bail!("Photon Engine could not create a webpage view");
        }
        callbacks.engine_view.store(view, Ordering::Release);
        let session = Self {
            _runtime: runtime,
            view,
            startup_url,
            callbacks,
            presentation: presentation.clone(),
            leases,
            gpu_activity,
            final_surface_releases: None,
            display: None,
            shutdown_started: false,
            finished: false,
        };
        if let Err(error) = presentation.activate() {
            drop(session);
            return Err(error);
        }
        if !unsafe { embedder::photon_view_set_native_metal_presentation(view, true) } {
            drop(session);
            anyhow::bail!("Photon Engine rejected native Metal presentation");
        }
        trace(format_args!(
            "Engine session ready; waiting for measured WebView viewport"
        ));
        Ok(session)
    }

    pub(super) fn navigate_startup(&mut self) {
        if let Some(url) = self.startup_url.take() {
            self.load(&url);
        }
    }

    /// Opens typed omnibox text as an address or a search.
    pub(super) fn navigate(&mut self, input: &str) -> anyhow::Result<()> {
        let command = BrowserCommand::from_omnibox_input(input)
            .map_err(|error| anyhow::anyhow!("cannot open {input:?}: {error:?}"))?;
        self.execute(command)
    }

    pub(super) fn execute(&mut self, command: BrowserCommand) -> anyhow::Result<()> {
        match command {
            BrowserCommand::Navigate(url) => {
                let url = CString::new(url)?;
                self.load(&url);
            }
            BrowserCommand::Reload => unsafe { embedder::photon_view_reload(self.view) },
            BrowserCommand::StopLoading => unsafe { embedder::photon_view_stop_loading(self.view) },
            BrowserCommand::Back => unsafe { embedder::photon_view_go_back(self.view) },
            BrowserCommand::Forward => unsafe { embedder::photon_view_go_forward(self.view) },
            BrowserCommand::NewTab
            | BrowserCommand::NewWindow
            | BrowserCommand::ToggleDebugInfo => {
                anyhow::bail!("browser-level command sent to a page session")
            }
        }
        Ok(())
    }

    fn load(&mut self, url: &CStr) {
        unsafe { embedder::photon_view_navigate(self.view, url.as_ptr()) };
        trace(format_args!(
            "Engine navigation; URL={}",
            url.to_string_lossy()
        ));
    }

    pub(super) fn set_ui_wake(&self, wake: UiWake) {
        self.callbacks.set_ui_wake(wake);
    }

    pub(super) fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    pub(super) fn set_focus(&mut self, focused: bool) {
        unsafe { embedder::photon_view_set_focus(self.view, focused) }
    }

    pub(super) fn set_visible(&mut self, visible: bool) {
        unsafe { embedder::photon_view_set_visible(self.view, visible) }
    }

    /// Re-reads the display's refresh rate the next time the view is laid out.
    pub(super) fn forget_display(&mut self) {
        self.display = None;
    }

    /// Paces Engine rendering to the display that shows this view.
    pub(super) fn set_display(&mut self, display: u64) {
        if self.display == Some(display) {
            return;
        }
        self.display = Some(display);
        let refresh_rate = super::display::refresh_rate(display);
        trace(format_args!(
            "display={display} refresh-rate={refresh_rate:.2}Hz"
        ));
        unsafe { embedder::photon_view_set_display_metadata(self.view, display, refresh_rate) }
    }

    pub(super) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.callbacks.set_diagnostics_enabled(enabled);
        unsafe { embedder::photon_view_set_performance_monitor_enabled(self.view, enabled) }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn send_pointer(
        &mut self,
        kind: i32,
        x: f64,
        y: f64,
        button: i32,
        buttons: u8,
        shift: bool,
        control: bool,
        alt: bool,
        meta: bool,
        wheel_x: f64,
        wheel_y: f64,
        precise: bool,
        phase: i32,
        clicks: i32,
    ) {
        if kind == 2 || kind == 4 {
            self.callbacks.mark_input();
        }
        unsafe {
            embedder::photon_view_pointer(
                self.view, kind, x, y, button, buttons, shift, control, alt, meta, wheel_x,
                wheel_y, precise, phase, clicks,
            )
        }
    }

    pub(super) fn send_key(
        &mut self,
        key: u16,
        pressed: bool,
        code_point: u32,
        modifiers: gpui::Modifiers,
        repeat: bool,
        insert_text: bool,
    ) {
        if pressed {
            self.callbacks.mark_input();
        }
        unsafe {
            embedder::photon_view_key(
                self.view,
                key,
                pressed,
                code_point,
                modifiers.shift,
                modifiers.control,
                modifiers.alt,
                modifiers.platform,
                repeat,
                insert_text,
            )
        }
    }

    pub(super) fn drain_releases(&mut self) {
        deliver_pending_releases(&self.callbacks);
    }

    pub(super) fn begin_shutdown(
        &mut self,
        final_surface_releases: Arc<Mutex<Vec<RetiredSurface>>>,
    ) {
        if self.shutdown_started {
            return;
        }
        if !self.view.is_null() {
            unsafe {
                embedder::photon_view_set_native_metal_presentation(self.view, false);
            }
        }
        self.presentation.stop_accepting_frames();
        self.final_surface_releases = Some(final_surface_releases);
        self.shutdown_started = true;
    }

    pub(super) fn finish_shutdown(&mut self) {
        if self.finished {
            return;
        }
        self.gpu_activity.wait_until_idle();
        if let Some(final_releases) = self.final_surface_releases.take() {
            for release in std::mem::take(&mut *final_releases.lock().unwrap()) {
                self.leases.lock().unwrap().complete(release.key);
            }
        }
        self.presentation.stop_accepting_frames();
        self.drain_releases();
        let leases = self.leases.lock().unwrap().counts();
        let (gpu_submitted, gpu_completed, gpu_in_flight) = self.gpu_activity.counts();
        trace(format_args!(
            "shutdown accounting received={} accepted={} presented={} lease-completed={} released={} outstanding={} gpu-submitted={gpu_submitted} gpu-completed={gpu_completed} gpu-in-flight={gpu_in_flight}",
            leases.received,
            leases.accepted,
            leases.presented,
            leases.completed,
            leases.released,
            leases.outstanding,
        ));
        assert_eq!(
            leases.outstanding, 0,
            "native presentation leases remain at shutdown"
        );
        assert_eq!(
            leases.received, leases.completed,
            "native presentation leases did not complete"
        );
        assert_eq!(
            leases.completed, leases.released,
            "completed native presentation leases were not released"
        );
        assert_eq!(
            gpu_submitted, gpu_completed,
            "Metal command buffers did not all complete"
        );
        assert_eq!(gpu_in_flight, 0);
        self.presentation
            .release_scheduler
            .store(std::ptr::null_mut(), Ordering::Release);
        unsafe {
            if !self.view.is_null() {
                self.callbacks
                    .engine_view
                    .store(std::ptr::null_mut(), Ordering::Release);
                embedder::photon_view_shutdown(self.view);
                embedder::photon_view_destroy(self.view);
                self.view = std::ptr::null_mut();
            }
        }
        self.finished = true;
    }
}

unsafe extern "C" fn on_system_reduced_motion_changed(runtime: *mut c_void, reduce_motion: bool) {
    if !runtime.is_null() {
        unsafe {
            embedder::photon_runtime_set_system_reduced_motion_preference(runtime, reduce_motion)
        };
    }
}

fn resolve_address(input: &str) -> anyhow::Result<CString> {
    let command = BrowserCommand::from_omnibox_input(input)
        .map_err(|error| anyhow::anyhow!("cannot open {input:?}: {error:?}"))?;
    let BrowserCommand::Navigate(url) = command else {
        unreachable!("omnibox input always resolves to navigation")
    };
    Ok(CString::new(url)?)
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        if !self.finished {
            self.begin_shutdown(Arc::new(Mutex::new(Vec::new())));
            self.finish_shutdown();
        }
    }
}

unsafe extern "C" fn on_engine_state(
    context: *mut c_void,
    url: *const c_char,
    title: *const c_char,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
) {
    let url = (!url.is_null()).then(|| {
        unsafe { CStr::from_ptr(url) }
            .to_string_lossy()
            .into_owned()
    });
    let title = (!title.is_null())
        .then(|| {
            unsafe { CStr::from_ptr(title) }
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_default();
    if !loading && let Some(url) = url.as_deref() {
        trace(format_args!("page-state url={url} title={title}"));
    }
    if !context.is_null() {
        let callbacks = unsafe { &*context.cast::<CallbackState>() };
        callbacks.set_page_state(BrowserState {
            url: url.unwrap_or_default(),
            title,
            loading,
            can_go_back,
            can_go_forward,
            error: None,
        });
    }
}
unsafe extern "C" fn on_engine_frame(
    context: *mut c_void,
    _: i32,
    _: i32,
    _: usize,
    _: f64,
    _: *const u8,
    _: usize,
    _: u64,
    _: u64,
    _: u64,
    _: u64,
) {
    if !context.is_null() {
        let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
        callbacks.record_frame();
        callbacks.request_redraw();
    }
}
unsafe extern "C" fn on_engine_cursor(_: *mut c_void, _: i32) {}
unsafe extern "C" fn on_engine_error(context: *mut c_void, message: *const c_char) {
    if !message.is_null() {
        let message = unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned();
        eprintln!("Photon Engine: {message}");
        if !context.is_null() {
            unsafe { &*(context.cast::<CallbackState>()) }.set_page_error(message);
        }
    }
}

unsafe extern "C" fn on_engine_crash(context: *mut c_void, url: *const c_char) {
    let url = (!url.is_null())
        .then(|| {
            unsafe { CStr::from_ptr(url) }
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "unknown page".into());
    eprintln!("Photon Engine: WebContent process crashed while displaying {url}");
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.handle_engine_crash(url);
    }
}

unsafe extern "C" fn on_engine_performance_stats(
    context: *mut c_void,
    stats: *const embedder::PerformanceStats,
) {
    if context.is_null() || stats.is_null() {
        return;
    }
    unsafe { &*(context.cast::<CallbackState>()) }.set_engine_diagnostics(unsafe { &*stats });
}
unsafe extern "C" fn on_engine_backing(
    context: *mut c_void,
    backing: u64,
    generation: u64,
    width: u32,
    height: u32,
    format: u32,
    port: u32,
) -> bool {
    if context.is_null() {
        return false;
    }
    let state = unsafe { &*(context.cast::<CallbackState>()) };
    let _owned_port = MachPortGuard(port as mach_port_t);
    match state.presentation.register_backing(
        backing,
        generation,
        width,
        height,
        format,
        port as mach_port_t,
    ) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Photon presentation rejected backing: {error:#}");
            false
        }
    }
}
unsafe extern "C" fn on_engine_native_frame(
    context: *mut c_void,
    backing: u64,
    generation: u64,
    frame: u64,
    signal: u64,
    width: i32,
    height: i32,
    _: f64,
) {
    if context.is_null() {
        return;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    callbacks.record_frame();
    callbacks
        .presentation
        .receive_frame(backing, generation, frame, signal, width, height);
    callbacks.presentation.schedule_release_drain();
    callbacks.request_redraw();
}

unsafe extern "C" fn on_native_release_drain(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    let runtime_callbacks = unsafe { &*(context.cast::<RuntimeCallbacks>()) };
    let callbacks = {
        let mut views = runtime_callbacks.views.lock().unwrap();
        let mut callbacks = Vec::with_capacity(views.len());
        views.retain(|view| {
            if let Some(view) = view.upgrade() {
                callbacks.push(view);
                true
            } else {
                false
            }
        });
        callbacks
    };
    for callback in callbacks {
        deliver_pending_releases(&callback);
    }
}
