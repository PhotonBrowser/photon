//! The Engine runtime and per-tab sessions: navigation, input and shutdown.

mod callbacks;

use anyhow::Context as _;
use gpui::{AsyncApp, WeakEntity};
use photon_core::{BrowserCommand, DialogReply};
use photon_performance::PerformanceMonitor;
use std::{
    ffi::{CStr, CString, c_void},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicPtr, Ordering},
    },
};

use super::presentation::{GpuActivity, LeaseLedger, PresentationRuntime, RetiredSurface};
use super::ui::PhotonWebView;
use super::{ffi::embedder, trace};
use callbacks::*;

#[derive(Clone)]
pub(super) struct UiWake {
    pub(super) app: AsyncApp,
    pub(super) webview: WeakEntity<PhotonWebView>,
}

pub(super) struct EngineRuntime {
    runtime: *mut c_void,
    reduced_motion_observer: *mut c_void,
    callbacks: Box<RuntimeCallbacks>,
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
            performance: PerformanceMonitor::default(),
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
                Some(on_engine_favicon),
                Some(on_engine_dialog),
                Some(on_engine_navigation_committed),
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
            BrowserCommand::NewTab | BrowserCommand::NewWindow => {
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

    /// Answers the page's open dialog.
    pub(super) fn reply_dialog(&mut self, reply: DialogReply) {
        const ALERT: i32 = 0;
        const CONFIRM: i32 = 1;
        const PROMPT: i32 = 2;
        let (dialog_type, accepted, text) = match reply {
            DialogReply::Alert => (ALERT, true, None),
            DialogReply::Confirm(accepted) => (CONFIRM, accepted, None),
            DialogReply::Prompt(text) => (
                PROMPT,
                text.is_some(),
                // Interior NULs cannot cross the C ABI; drop them.
                text.map(|text| CString::new(text.replace('\0', "")).unwrap_or_default()),
            ),
        };
        let text_ptr = text.as_ref().map_or(std::ptr::null(), |text| text.as_ptr());
        unsafe { embedder::photon_view_close_dialog(self.view, dialog_type, accepted, text_ptr) }
    }

    /// Sets the color scheme pages see through `prefers-color-scheme`.
    pub(super) fn set_dark_color_scheme(&mut self, dark: bool) {
        const DARK: i32 = 1;
        const LIGHT: i32 = 2;
        let scheme = if dark { DARK } else { LIGHT };
        unsafe { embedder::photon_view_set_preferred_color_scheme(self.view, scheme) }
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
