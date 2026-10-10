//! The Engine runtime and per-tab sessions: navigation, input and shutdown.

mod callbacks;

use anyhow::Context as _;
use gpui::{AsyncApp, WeakEntity};
use photon_core::{BrowserCommand, ClearBrowsingData, DialogReply};
use photon_performance::PerformanceMonitor;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicPtr, AtomicU8, Ordering},
    },
};

use super::presentation::{GpuActivity, LeaseLedger, PresentationRuntime, RetiredSurface};
use super::ui::PhotonWebView;
use super::{ffi::embedder, trace};
use callbacks::*;
use photon_core::EngineService;
use std::cell::RefCell;

#[derive(Clone)]
pub(super) struct UiWake {
    pub(super) app: AsyncApp,
    pub(super) webview: WeakEntity<PhotonWebView>,
    pub(super) runtime: Rc<EngineRuntime>,
}

pub(super) struct EngineRuntime {
    runtime: *mut c_void,
    reduced_motion_observer: *mut c_void,
    callbacks: Box<RuntimeCallbacks>,
    service_callback: RefCell<Option<Box<ServiceCallback>>>,
    popup_policy: AtomicU8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum PopupPolicy {
    Ask = 0,
    Allow = 1,
    Block = 2,
}

impl PopupPolicy {
    fn from_byte(value: u8) -> Self {
        match value {
            1 => Self::Allow,
            2 => Self::Block,
            _ => Self::Ask,
        }
    }
}

impl EngineRuntime {
    /// Starts the Engine, keeping website data in `profile_dir`, or in a
    /// temporary profile when there is none.
    pub(super) fn create(profile_dir: Option<&Path>) -> anyhow::Result<Self> {
        let helper = std::env::var("PHOTON_HELPER_DIRECTORY")
            .context("PHOTON_HELPER_DIRECTORY is not set")?;
        let helper = CString::new(helper)?;
        let profile = profile_dir
            .map(|dir| CString::new(dir.to_string_lossy().into_owned()))
            .transpose()?;
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(
                helper.as_ptr(),
                profile
                    .as_ref()
                    .map_or(std::ptr::null(), |profile| profile.as_ptr()),
                error.as_mut_ptr(),
                error.len(),
            )
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
        #[cfg(target_os = "macos")]
        unsafe {
            embedder::photon_runtime_use_system_clipboard(runtime);
        }
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
            service_callback: RefCell::default(),
            popup_policy: AtomicU8::new(PopupPolicy::Ask as u8),
        })
    }

    pub(super) fn popup_policy(&self) -> PopupPolicy {
        PopupPolicy::from_byte(self.popup_policy.load(Ordering::Relaxed))
    }

    pub(super) fn set_popup_policy(&self, policy: PopupPolicy) {
        self.popup_policy.store(policy as u8, Ordering::Relaxed);
    }
}

impl EngineRuntime {
    /// Calls `on_change(service, restarted)` on the main thread when an Engine
    /// service stops (`false`) and once it is running again (`true`). It must
    /// not update GPUI entities synchronously.
    pub(in crate::platform) fn on_service_change(
        &self,
        on_change: impl Fn(EngineService, bool) + 'static,
    ) {
        let mut callback: Box<ServiceCallback> = Box::new(Box::new(on_change));
        unsafe {
            embedder::photon_runtime_set_service_callback(
                self.runtime,
                (&mut *callback as *mut ServiceCallback).cast(),
                Some(on_engine_service),
            );
        }
        self.service_callback.replace(Some(callback));
    }
}

impl EngineRuntime {
    /// Deletes the website data `request` asks for, then calls `done` on the
    /// main thread. `done` must not update GPUI entities synchronously.
    pub(in crate::platform) fn clear_browsing_data(
        &self,
        request: &ClearBrowsingData,
        done: impl FnOnce() + 'static,
    ) {
        if !request.touches_engine() {
            done();
            return;
        }
        let done: Box<BrowsingDataCleared> = Box::new(Box::new(done));
        unsafe {
            embedder::photon_runtime_clear_browsing_data(
                self.runtime,
                i64::try_from(request.since).unwrap_or(i64::MAX),
                request.cache,
                request.site_data,
                Box::into_raw(done).cast(),
                Some(on_engine_browsing_data_cleared),
            );
        }
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

/// A change to a page's zoom level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) enum ZoomStep {
    In,
    Out,
    Reset,
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

pub(super) struct RequestedWebView {
    pub(super) session: EngineSession,
    pub(super) popup: bool,
    pub(super) activate: bool,
    pub(super) needs_confirmation: bool,
    pub(super) width: Option<i32>,
    pub(super) height: Option<i32>,
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
        Self::create_with_native_view(runtime, startup_url, |runtime, callbacks| unsafe {
            embedder::photon_view_create(runtime, width, height, dpr, callbacks)
        })
    }

    pub(super) fn create_for_traversable(
        runtime: Rc<EngineRuntime>,
        parent_view: *mut c_void,
        traversable: *mut c_void,
        width: i32,
        height: i32,
        dpr: f64,
    ) -> anyhow::Result<Self> {
        Self::create_with_native_view(runtime, None, move |runtime, callbacks| unsafe {
            embedder::photon_view_create_for_traversable(
                runtime,
                parent_view,
                traversable,
                width,
                height,
                dpr,
                callbacks,
            )
        })
    }

    fn create_with_native_view(
        runtime: Rc<EngineRuntime>,
        startup_url: Option<CString>,
        create_native_view: impl FnOnce(*mut c_void, *const embedder::ViewCallbacks) -> *mut c_void,
    ) -> anyhow::Result<Self> {
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
        let native_callbacks = embedder::ViewCallbacks {
            callback_data: Arc::as_ptr(&callbacks).cast_mut().cast(),
            state_callback: Some(on_engine_state),
            frame_callback: Some(on_engine_frame),
            cursor_callback: Some(on_engine_cursor),
            error_callback: Some(on_engine_error),
            crash_callback: Some(on_engine_crash),
            performance_callback: Some(on_engine_performance_stats),
            favicon_callback: Some(on_engine_favicon),
            audio_state_callback: Some(on_engine_audio_state),
            dialog_callback: Some(on_engine_dialog),
            navigation_committed_callback: Some(on_engine_navigation_committed),
            crash_recovered_callback: Some(on_engine_crash_recovered),
            new_web_view_callback: Some(on_engine_new_web_view),
            find_result_callback: Some(on_engine_find_result),
            zoom_callback: Some(on_engine_zoom),
            page_unresponsive_callback: Some(on_engine_page_unresponsive),
            context_menu_callback: Some(on_engine_context_menu),
            open_in_new_tab_callback: Some(on_engine_open_in_new_tab),
            #[cfg(target_os = "macos")]
            native_metal_presentation: true,
            #[cfg(target_os = "macos")]
            native_backing_callback: Some(on_engine_backing),
            #[cfg(target_os = "macos")]
            native_frame_callback: Some(on_engine_native_frame),
        };
        let view = create_native_view(runtime.runtime, &native_callbacks);
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
        unsafe { embedder::photon_view_notify_state(self.view) };
    }

    pub(super) fn copy_window_handle(&self, buffer: *mut c_char, capacity: usize) {
        unsafe { embedder::photon_view_copy_window_handle(self.view, buffer, capacity) };
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

    /// Finds text in the page, highlighting every match; an empty query
    /// clears the search.
    pub(super) fn find_in_page(&mut self, query: &str) {
        // Interior NULs cannot cross the C ABI; drop them.
        let Ok(query) = CString::new(query.replace('\0', "")) else {
            return;
        };
        unsafe { embedder::photon_view_find_in_page(self.view, query.as_ptr(), false, true) }
    }

    /// Moves to the next match, or the previous one when `forward` is false.
    pub(super) fn find_in_page_step(&mut self, forward: bool) {
        unsafe { embedder::photon_view_find_in_page_step(self.view, forward) }
    }

    /// Zooms in a step, out a step, or back to 100%.
    pub(super) fn zoom(&mut self, step: ZoomStep) {
        let step = match step {
            ZoomStep::In => 1,
            ZoomStep::Out => -1,
            ZoomStep::Reset => 0,
        };
        unsafe { embedder::photon_view_zoom(self.view, step) }
    }

    pub(super) fn restart_unresponsive_page(&mut self) {
        unsafe { embedder::photon_view_restart_unresponsive_page(self.view) }
    }

    /// Runs an item of the context menu the page last asked for.
    pub(super) fn activate_context_menu_item(&mut self, index: usize) {
        unsafe { embedder::photon_view_activate_context_menu_item(self.view, index) }
    }

    /// Ends the search and removes its highlights.
    pub(super) fn find_in_page_end(&mut self) {
        unsafe { embedder::photon_view_find_in_page_end(self.view) }
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

    pub(super) fn toggle_audio_mute(&mut self) -> bool {
        unsafe { embedder::photon_view_toggle_audio_mute(self.view) }
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
