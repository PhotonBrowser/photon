//! The Engine's C callbacks and the shared state they update.

use gpui::Context;
use mach2::port::mach_port_t;
use photon_core::{BrowserState, DialogKind, DialogRequest, EngineEvent, EngineService};
use photon_performance::{EnginePerformanceStats, PerformanceMonitor};
use std::{
    ffi::{CStr, c_char, c_void},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicPtr, AtomicU64, Ordering},
    },
};

use super::super::presentation::{LeaseLedger, MachPortGuard, PresentationRuntime};
use super::super::ui::{Favicon, PhotonWebView};
use super::super::{ffi::embedder, trace};
use super::{EngineSession, PopupPolicy, RequestedWebView, UiWake};

pub(super) struct CallbackState {
    pub(super) presentation: Arc<PresentationRuntime>,
    pub(super) leases: Arc<Mutex<LeaseLedger>>,
    pub(super) engine_view: AtomicPtr<c_void>,
    pub(super) ui_wake: Mutex<Option<UiWake>>,
    pub(super) performance: PerformanceMonitor,
}

pub(super) struct RuntimeCallbacks {
    pub(super) views: Mutex<Vec<Weak<CallbackState>>>,
}

static NEXT_WINDOW_HANDLE: AtomicU64 = AtomicU64::new(1);

impl CallbackState {
    pub(super) fn set_ui_wake(&self, wake: UiWake) {
        *self.ui_wake.lock().unwrap() = Some(wake);
        self.request_redraw();
    }

    pub(super) fn request_redraw(&self) {
        let diagnostics = self.performance.snapshot_if_enabled();
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

    pub(super) fn set_page_state(&self, state: BrowserState) {
        self.update_webview(move |view, cx| view.set_state(state, cx));
    }

    pub(super) fn set_page_error(&self, message: String) {
        self.update_webview(move |view, cx| {
            view.state.apply(EngineEvent::LoadFailed(message));
            view.state_changed(cx);
        });
    }

    pub(super) fn set_page_favicon(&self, favicon: Option<Favicon>) {
        self.update_webview(move |view, cx| view.set_favicon(favicon, cx));
    }

    pub(super) fn navigation_committed(&self) {
        self.update_webview(|view, _| view.dialogs.navigation_committed());
    }

    pub(super) fn request_page_dialog(&self, request: DialogRequest) {
        self.update_webview(move |view, cx| view.request_dialog(request, cx));
    }

    pub(super) fn handle_engine_crash(&self) {
        self.update_webview(|view, cx| view.handle_engine_crash(cx));
    }

    pub(super) fn crash_recovered(&self) {
        self.update_webview(|view, cx| view.crash_recovered(cx));
    }

    pub(super) fn mark_input(&self) {
        self.performance.mark_input();
    }

    /// Tracks frame gaps and input latency, which only the diagnostics
    /// overlay shows, so frames skip this while it is hidden.
    pub(super) fn record_frame(&self) {
        self.performance.record_frame();
    }

    pub(super) fn set_engine_diagnostics(&self, stats: &embedder::PerformanceStats) {
        self.performance.set_engine_stats(EnginePerformanceStats {
            cpu_percent: stats.has_cpu_percent.then_some(stats.cpu_percent),
            memory_bytes: stats.has_memory_bytes.then_some(stats.memory_bytes),
            managed_heap_bytes: stats
                .has_managed_heap_bytes
                .then_some(stats.managed_heap_bytes),
            download_bytes_per_second: stats.download_bytes_per_second,
            upload_bytes_per_second: stats.upload_bytes_per_second,
            frames_per_second: stats
                .has_frames_per_second
                .then_some(stats.frames_per_second),
        });
        if self.performance.is_enabled() {
            self.publish_diagnostics();
        }
    }

    pub(super) fn set_diagnostics_enabled(&self, enabled: bool) {
        // Frames were not tracked while hidden; starting enabled resets the
        // interval so it does not span the hidden period.
        self.performance.set_enabled(enabled);
        if enabled {
            self.publish_diagnostics();
        }
    }

    pub(super) fn publish_diagnostics(&self) {
        let snapshot = self.performance.snapshot();
        self.update_webview(move |view, cx| {
            if view.diagnostics != snapshot {
                view.diagnostics = snapshot;
                cx.notify();
            }
        });
    }

    /// Runs `update` on the UI thread against the live WebView.
    pub(super) fn update_webview(
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
                // Notifying the view is enough to schedule a frame; a window
                // refresh would bypass every cached view.
                webview.update(cx, update);
            })
            .detach();
    }
}

pub(super) fn deliver_pending_releases(callbacks: &CallbackState) {
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

pub(super) unsafe extern "C" fn on_engine_state(
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
pub(super) unsafe extern "C" fn on_engine_frame(
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
pub(super) unsafe extern "C" fn on_engine_cursor(_: *mut c_void, _: i32) {}
pub(super) unsafe extern "C" fn on_engine_error(context: *mut c_void, message: *const c_char) {
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

pub(super) unsafe extern "C" fn on_engine_crash(context: *mut c_void, url: *const c_char) {
    let url = (!url.is_null())
        .then(|| {
            unsafe { CStr::from_ptr(url) }
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "unknown page".into());
    eprintln!("Photon Engine: WebContent process crashed while displaying {url}");
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.handle_engine_crash();
    }
}

pub(super) unsafe extern "C" fn on_engine_crash_recovered(context: *mut c_void) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.crash_recovered();
    }
}

pub(super) unsafe extern "C" fn on_engine_new_web_view(
    context: *mut c_void,
    _runtime: *mut c_void,
    parent_view: *mut c_void,
    request: *const embedder::NewWebViewRequest,
    window_handle: *mut c_char,
    window_handle_capacity: usize,
) {
    if context.is_null() || request.is_null() {
        return;
    }
    let callbacks = unsafe { &*context.cast::<CallbackState>() };
    let Some(wake) = callbacks.ui_wake.lock().unwrap().clone() else {
        return;
    };
    let request = unsafe { &*request };
    let policy = wake.runtime.popup_policy();
    if request.popup && policy == PopupPolicy::Block {
        trace(format_args!("blocked site popup"));
        return;
    }

    let id = NEXT_WINDOW_HANDLE.fetch_add(1, Ordering::Relaxed);
    let width = request.has_width.then_some(request.width);
    let height = request.has_height.then_some(request.height);
    let viewport_width = width.unwrap_or(1200).max(1);
    let viewport_height = height.unwrap_or(760).max(1);
    let session = match EngineSession::create_for_traversable(
        wake.runtime.clone(),
        parent_view,
        request.traversable,
        viewport_width,
        viewport_height,
        1.0,
    ) {
        Ok(session) => session,
        Err(error) => {
            trace(format_args!("could not create requested page: {error:#}"));
            return;
        }
    };
    session.copy_window_handle(window_handle, window_handle_capacity);
    let pending = RequestedWebView {
        session,
        popup: request.popup,
        activate: request.activate,
        needs_confirmation: request.popup && policy == PopupPolicy::Ask,
        width,
        height,
    };
    wake.app
        .spawn(async move |cx| {
            let Some(webview) = wake.webview.upgrade() else {
                return;
            };
            webview.update(cx, |view, cx| view.queue_new_web_view(id, pending, cx));
        })
        .detach();
}

/// Called on the main thread with an Engine service's stop or restart.
pub(super) type ServiceCallback = Box<dyn Fn(EngineService, bool)>;

pub(super) unsafe extern "C" fn on_engine_service(
    context: *mut c_void,
    service: i32,
    restarted: bool,
) {
    if context.is_null() {
        return;
    }
    let service = match service {
        0 => EngineService::Compositor,
        _ => EngineService::Network,
    };
    eprintln!(
        "Photon Engine: {service:?} service {}",
        if restarted { "restarted" } else { "stopped" }
    );
    let on_change = unsafe { &*context.cast::<ServiceCallback>() };
    on_change(service, restarted);
}

pub(super) unsafe extern "C" fn on_engine_performance_stats(
    context: *mut c_void,
    stats: *const embedder::PerformanceStats,
) {
    if context.is_null() || stats.is_null() {
        return;
    }
    unsafe { &*(context.cast::<CallbackState>()) }.set_engine_diagnostics(unsafe { &*stats });
}
pub(super) unsafe extern "C" fn on_engine_favicon(
    context: *mut c_void,
    pixels: *const u8,
    length: usize,
    width: i32,
    height: i32,
) {
    if context.is_null() {
        return;
    }
    let favicon = (!pixels.is_null() && width > 0 && height > 0)
        .then(|| unsafe { std::slice::from_raw_parts(pixels, length) })
        .and_then(|pixels| Favicon::from_bgra(pixels, width as u32, height as u32));
    unsafe { &*(context.cast::<CallbackState>()) }.set_page_favicon(favicon);
}

pub(super) unsafe extern "C" fn on_engine_navigation_committed(context: *mut c_void) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.navigation_committed();
    }
}

pub(super) unsafe extern "C" fn on_engine_dialog(
    context: *mut c_void,
    dialog_type: i32,
    title: *const c_char,
    message: *const c_char,
    default_text: *const c_char,
) {
    if context.is_null() {
        return;
    }
    let text = |value: *const c_char| {
        (!value.is_null())
            .then(|| {
                unsafe { CStr::from_ptr(value) }
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default()
    };
    let kind = match dialog_type {
        0 => DialogKind::Alert,
        1 => DialogKind::Confirm,
        _ => DialogKind::Prompt {
            default_text: text(default_text),
        },
    };
    unsafe { &*(context.cast::<CallbackState>()) }.request_page_dialog(DialogRequest {
        kind,
        title: text(title),
        message: text(message),
    });
}

pub(super) unsafe extern "C" fn on_engine_backing(
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
pub(super) unsafe extern "C" fn on_engine_native_frame(
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
    // Receiving a frame queues a release only when it drops one; GPU
    // completion schedules the drain for the rest.
    if callbacks.leases.lock().unwrap().has_pending() {
        callbacks.presentation.schedule_release_drain();
    }
    callbacks.request_redraw();
}

pub(super) unsafe extern "C" fn on_native_release_drain(context: *mut c_void) {
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
