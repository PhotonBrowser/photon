//! Engine runtime, its C callbacks, and input forwarding.

use anyhow::Context as _;
use gpui::{AsyncApp, WeakEntity};
use mach2::port::mach_port_t;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::{
        Arc, Mutex,
        atomic::{AtomicPtr, Ordering},
    },
};

use super::presentation::{GpuActivity, LeaseLedger, MachPortGuard, PresentationRuntime, Release};
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
}

impl CallbackState {
    pub(super) fn set_ui_wake(&self, wake: UiWake) {
        *self.ui_wake.lock().unwrap() = Some(wake);
        self.request_redraw();
    }

    fn request_redraw(&self) {
        let Some(wake) = self.ui_wake.lock().unwrap().clone() else {
            return;
        };
        wake.app
            .spawn(async move |cx| {
                let Some(webview) = wake.webview.upgrade() else {
                    return;
                };
                webview.update(cx, |view, cx| view.present_latest(cx));
                cx.refresh();
            })
            .detach();
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
    runtime: *mut c_void,
    view: *mut c_void,
    startup_url: Option<CString>,
    callbacks: Box<CallbackState>,
    pub(super) presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    pub(super) gpu_activity: Arc<GpuActivity>,
    final_surface_releases: Option<Arc<Mutex<Vec<Release>>>>,
    pub(super) shutdown_started: bool,
    finished: bool,
}

impl EngineSession {
    pub(super) fn create(width: i32, height: i32, dpr: f64) -> anyhow::Result<Self> {
        let url = std::env::var("PHOTON_URL").unwrap_or_else(|_| "https://example.com/".into());
        let url = photon_omnibox::resolve(&url)
            .map_err(|error| anyhow::anyhow!("invalid startup URL: {error:?}"))?
            .url()
            .to_owned();
        let startup_url = CString::new(url)?;
        let helper = std::env::var("PHOTON_HELPER_DIRECTORY")
            .context("PHOTON_HELPER_DIRECTORY is not set")?;
        let helper = CString::new(helper)?;
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
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(helper.as_ptr(), error.as_mut_ptr(), error.len())
        };
        anyhow::ensure!(
            !runtime.is_null(),
            "Photon Engine runtime failed: {}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        presentation
            .release_scheduler
            .store(runtime, Ordering::Release);
        let mut callbacks = Box::new(CallbackState {
            presentation: presentation.clone(),
            leases: leases.clone(),
            engine_view: AtomicPtr::new(std::ptr::null_mut()),
            ui_wake: Mutex::new(None),
        });
        let view = unsafe {
            embedder::photon_view_create(
                runtime,
                width,
                height,
                dpr,
                (&mut *callbacks as *mut CallbackState).cast(),
                Some(on_engine_state),
                Some(on_engine_frame),
                Some(on_engine_cursor),
                Some(on_engine_error),
                true,
                Some(on_engine_backing),
                Some(on_engine_native_frame),
            )
        };
        if view.is_null() {
            presentation
                .release_scheduler
                .store(std::ptr::null_mut(), Ordering::Release);
            unsafe { embedder::photon_runtime_destroy(runtime) };
            anyhow::bail!("Photon Engine could not create a webpage view");
        }
        callbacks.engine_view.store(view, Ordering::Release);
        unsafe {
            embedder::photon_runtime_set_native_release_drain_callback(
                runtime,
                (&mut *callbacks as *mut CallbackState).cast(),
                Some(on_native_release_drain),
            );
        }
        let session = Self {
            runtime,
            view,
            startup_url: Some(startup_url),
            callbacks,
            presentation: presentation.clone(),
            leases,
            gpu_activity,
            final_surface_releases: None,
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
        let Some(url) = self.startup_url.take() else {
            return;
        };
        unsafe { embedder::photon_view_navigate(self.view, url.as_ptr()) };
        trace(format_args!(
            "Engine session startup navigation; URL={}",
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

    pub(super) fn begin_shutdown(&mut self, final_surface_releases: Arc<Mutex<Vec<Release>>>) {
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
            if !self.runtime.is_null() {
                embedder::photon_runtime_set_native_release_drain_callback(
                    self.runtime,
                    std::ptr::null_mut(),
                    None,
                );
            }
            if !self.view.is_null() {
                self.callbacks
                    .engine_view
                    .store(std::ptr::null_mut(), Ordering::Release);
                embedder::photon_view_shutdown(self.view);
                embedder::photon_view_destroy(self.view);
                self.view = std::ptr::null_mut();
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
                self.runtime = std::ptr::null_mut();
            }
        }
        self.finished = true;
    }
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
    _: *mut c_void,
    url: *const c_char,
    title: *const c_char,
    loading: bool,
    _: bool,
    _: bool,
) {
    if !loading && !url.is_null() {
        let title = if title.is_null() {
            "<null>".to_owned()
        } else {
            unsafe { CStr::from_ptr(title) }
                .to_string_lossy()
                .into_owned()
        };
        trace(format_args!(
            "page-state url={} title={title}",
            unsafe { CStr::from_ptr(url) }.to_string_lossy(),
        ));
    }
}
unsafe extern "C" fn on_engine_frame(
    _: *mut c_void,
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
}
unsafe extern "C" fn on_engine_cursor(_: *mut c_void, _: i32) {}
unsafe extern "C" fn on_engine_error(_: *mut c_void, message: *const c_char) {
    if !message.is_null() {
        eprintln!(
            "Photon Engine: {}",
            unsafe { CStr::from_ptr(message) }.to_string_lossy()
        );
    }
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
    unsafe { &*(context.cast::<CallbackState>()) }
        .presentation
        .receive_frame(backing, generation, frame, signal, width, height);
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    callbacks.presentation.schedule_release_drain();
    callbacks.request_redraw();
}

unsafe extern "C" fn on_native_release_drain(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    deliver_pending_releases(callbacks);
}
