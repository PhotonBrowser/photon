//! A tab's Engine session: its page view, navigation, input, presentation
//! and shutdown.

use anyhow::Context as _;
use photon_core::{BrowserCommand, DialogReply};
use photon_performance::PerformanceMonitor;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicPtr, AtomicU64, Ordering},
    },
};

use super::super::presentation::{FrameKey, LeaseLedger, PresentationRuntime, PresentedSurface};
use super::callbacks::*;
use super::navigation_timing::{NavigationTimingProfiler, navigation_profile_enabled};
use super::{EngineRuntime, UiWake, ZoomStep};
use crate::platform::engine_ffi::embedder;
use crate::platform::trace;

static NEXT_PERFORMANCE_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(unix)]
fn path_to_cstring(path: &std::path::Path) -> Option<CString> {
    use std::os::unix::ffi::OsStrExt;

    CString::new(path.as_os_str().as_bytes()).ok()
}

#[cfg(windows)]
fn path_to_cstring(path: &std::path::Path) -> Option<CString> {
    CString::new(path.to_str()?.as_bytes()).ok()
}

pub(in crate::platform) struct EngineSession {
    _runtime: Rc<EngineRuntime>,
    view: *mut c_void,
    startup_url: Option<CString>,
    callbacks: Arc<CallbackState>,
    pub(in crate::platform) presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    /// The frame leases the page still held when it began to close.
    final_surface_releases: Option<Vec<FrameKey>>,
    display: Option<u64>,
    pub(in crate::platform) shutdown_started: bool,
    finished: bool,
}

pub(in crate::platform) struct RequestedWebView {
    pub(in crate::platform) session: EngineSession,
    pub(in crate::platform) popup: bool,
    pub(in crate::platform) activate: bool,
    pub(in crate::platform) width: Option<i32>,
    pub(in crate::platform) height: Option<i32>,
}

impl EngineSession {
    pub(in crate::platform) fn release_hidden_memory(&self) {
        if !self.view.is_null() {
            unsafe { embedder::photon_view_release_hidden_memory(self.view) };
        }
    }

    pub(in crate::platform) fn needs_beforeunload_check(&self) -> bool {
        !self.view.is_null() && unsafe { embedder::photon_view_needs_beforeunload_check(self.view) }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn registered_engine_processes(
        &self,
    ) -> Vec<super::RegisteredEngineProcess> {
        self._runtime.registered_processes()
    }

    #[cfg(unix)]
    pub(in crate::platform) fn set_task_queue_diagnostics_enabled(&self, enabled: bool) -> bool {
        !self.view.is_null()
            && unsafe {
                embedder::photon_view_set_task_queue_diagnostics_enabled(self.view, enabled)
            }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn task_queue_diagnostics_json(&self) -> anyhow::Result<String> {
        if self.view.is_null() {
            anyhow::bail!("Engine view is unavailable");
        }
        const CAPACITY: usize = 64 * 1024;
        let mut output = vec![0_u8; CAPACITY];
        let length = unsafe {
            embedder::photon_view_copy_task_queue_diagnostics(
                self.view,
                output.as_mut_ptr().cast(),
                output.len(),
            )
        };
        if length >= output.len() {
            anyhow::bail!("Engine task queue diagnostics exceeded the 64 KiB response limit");
        }
        let json = unsafe { CStr::from_ptr(output.as_ptr().cast()) };
        Ok(json.to_str()?.to_owned())
    }

    #[cfg(unix)]
    pub(in crate::platform) fn start_frame_timing_diagnostics(&self) {
        self.callbacks.performance.start_frame_timing(
            self.display
                .and_then(super::super::display::known_refresh_rate),
        );
    }

    #[cfg(unix)]
    pub(in crate::platform) fn stop_frame_timing_diagnostics(&self) {
        self.callbacks.performance.stop_frame_timing();
    }

    pub(in crate::platform) fn record_surface_install(&self, generation: u64, frame_id: u64) {
        self.callbacks
            .performance
            .record_surface_install(generation, frame_id);
    }

    #[cfg(unix)]
    pub(in crate::platform) fn frame_timing_diagnostics_json(&self) -> serde_json::Value {
        let diagnostics = self.callbacks.performance.frame_timing_snapshot();
        let frames = diagnostics
            .frames
            .into_iter()
            .map(|frame| {
                serde_json::json!([
                    frame.generation,
                    frame.frame_id,
                    frame.handoff_elapsed_ns,
                    frame.surface_install_elapsed_ns,
                    frame.gpu_submission_elapsed_ns,
                    frame.gpu_completion_callback_elapsed_ns,
                    frame.drawable_id,
                    frame.presented_time_seconds,
                    frame.presentation_feedback_received,
                    frame.presentation_dropped,
                    frame.presentation_feedback_count,
                    frame.presentation_dropped_count,
                    frame.coalesced_before_install,
                ])
            })
            .collect::<Vec<_>>();
        let presentation_events = diagnostics
            .presentation_events
            .into_iter()
            .map(|event| {
                serde_json::json!([
                    event.generation,
                    event.frame_id,
                    event.drawable_id,
                    event.presented_time_seconds,
                ])
            })
            .collect::<Vec<_>>();
        let gpu_sampling_events = diagnostics
            .gpu_sampling_events
            .into_iter()
            .map(|event| {
                serde_json::json!([
                    event.generation,
                    event.frame_id,
                    event.drawable_id,
                    event.submitted_elapsed_ns,
                    event.completion_callback_elapsed_ns,
                ])
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "schema_version": 1,
            "scope": "photon_tab_engine_surface_frames",
            "surface_identity_semantics": "generation is the target renderer resource epoch; engine_frame_id is the monotonic publication sequence within this WebView; the pair identifies a surface update, not a content generation or proof of changed pixels",
            "duration_clock_domain": "photon_process_monotonic_elapsed_ns",
            "presentation_clock_domain": "host_backend_defined",
            "presentation_timestamp_source": "available only when the target renderer reports native drawable feedback",
            "measurement_window_ms": diagnostics.measurement_window_ms,
            "display_refresh_rate_hz": diagnostics.display_refresh_rate_hz,
            "display_refresh_rate_change_count": diagnostics.display_refresh_rate_change_count,
            "coalesced_before_install_count": diagnostics.coalesced_before_install_count,
            "evicted_frame_sample_count": diagnostics.evicted_frame_sample_count,
            "evicted_presentation_event_count": diagnostics.evicted_presentation_event_count,
            "evicted_gpu_sampling_event_count": diagnostics.evicted_gpu_sampling_event_count,
            "frame_sample_count": frames.len(),
            "frame_sample_fields": [
                "generation",
                "engine_frame_id",
                "handoff_elapsed_ns",
                "surface_install_elapsed_ns",
                "gpu_submission_elapsed_ns",
                "gpu_completion_callback_elapsed_ns",
                "drawable_id",
                "presented_time_seconds",
                "presentation_feedback_received",
                "presentation_dropped",
                "presentation_feedback_count",
                "presentation_dropped_count",
                "coalesced_before_install",
            ],
            "presentation_event_count": presentation_events.len(),
            "presentation_event_fields": [
                "generation",
                "engine_frame_id",
                "drawable_id",
                "presented_time_seconds",
            ],
            "gpu_sampling_event_count": gpu_sampling_events.len(),
            "gpu_sampling_event_fields": [
                "generation",
                "engine_frame_id",
                "drawable_id",
                "submitted_elapsed_ns",
                "completion_callback_elapsed_ns",
            ],
            "frames": frames,
            "presentation_events": presentation_events,
            "gpu_sampling_events": gpu_sampling_events,
        })
    }

    pub(in crate::platform) fn create(
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

    pub(in crate::platform) fn create_for_traversable(
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
        anyhow::ensure!(
            runtime.is_available(),
            "web pages are unavailable in the UI-only build"
        );
        #[cfg(target_os = "macos")]
        let leases = Arc::new(Mutex::new(LeaseLedger::default()));
        #[cfg(not(target_os = "macos"))]
        let leases = Arc::new(Mutex::new(LeaseLedger));
        let performance = Arc::new(PerformanceMonitor::default());
        let navigation_timing =
            navigation_profile_enabled().then(|| Arc::new(NavigationTimingProfiler::default()));
        // Engine threads reach both through raw callback contexts and touch
        // only their mutex- and atomic-guarded state; the main-thread-only
        // fields (pixel buffers, the UI wake) are used on the main thread.
        #[cfg_attr(
            target_os = "macos",
            expect(
                clippy::arc_with_non_send_sync,
                reason = "shared with Engine callbacks through a raw context pointer"
            )
        )]
        let presentation = Arc::new(PresentationRuntime::new(
            leases.clone(),
            navigation_timing.clone(),
            performance.clone(),
        )?);
        presentation
            .release_scheduler
            .store(runtime.runtime, Ordering::Release);
        // Shared with Engine callbacks through a raw context pointer; the weak
        // self-reference lets a frame's GPU-completion wakeup outlive no view.
        let callbacks = Arc::new_cyclic(|self_ref| CallbackState {
            self_ref: self_ref.clone(),
            task_id: NEXT_PERFORMANCE_TASK_ID.fetch_add(1, Ordering::Relaxed),
            presentation: presentation.clone(),
            #[cfg(target_os = "macos")]
            leases: leases.clone(),
            engine_view: AtomicPtr::new(std::ptr::null_mut()),
            ui_wake: Mutex::new(None),
            redraw_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            performance,
            page_details: Mutex::default(),
            navigation_timing: navigation_timing.clone(),
        });
        runtime
            .callbacks
            .views
            .lock()
            .unwrap()
            .push(Arc::downgrade(&callbacks));
        let native_callbacks = embedder::ViewCallbacks {
            callback_data: Arc::as_ptr(&callbacks).cast_mut().cast(),
            navigation_timing_callback: navigation_timing
                .is_some()
                .then_some(on_engine_navigation_timing),
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
            history_swipe_candidate_callback: Some(on_engine_history_swipe_candidate),
            crash_recovered_callback: Some(on_engine_crash_recovered),
            new_web_view_callback: Some(on_engine_new_web_view),
            find_result_callback: Some(on_engine_find_result),
            zoom_callback: Some(on_engine_zoom),
            page_unresponsive_callback: Some(on_engine_page_unresponsive),
            context_menu_callback: Some(on_engine_context_menu),
            open_in_new_tab_callback: Some(on_engine_open_in_new_tab),
            peek_callback: Some(on_engine_peek),
            select_dropdown_callback: Some(on_engine_select_dropdown),
            file_picker_callback: Some(on_engine_file_picker),
            permission_callback: Some(on_engine_permission_request),
            permission_cancelled_callback: Some(on_engine_permission_cancelled),
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
        callbacks.set_residency_monitor_enabled(true);
        if runtime.performance_dashboard_active() {
            callbacks.set_dashboard_enabled(true);
        }
        let session = Self {
            _runtime: runtime,
            view,
            startup_url,
            callbacks,
            presentation: presentation.clone(),
            leases,
            final_surface_releases: None,
            display: None,
            shutdown_started: false,
            finished: false,
        };
        if let Err(error) = presentation.activate() {
            drop(session);
            return Err(error);
        }
        #[cfg(target_os = "macos")]
        if !unsafe { embedder::photon_view_set_native_metal_presentation(view, true) } {
            drop(session);
            anyhow::bail!("Photon Engine rejected native Metal presentation");
        }
        trace(format_args!(
            "Engine session ready; waiting for measured WebView viewport"
        ));
        Ok(session)
    }

    pub(in crate::platform) fn navigate_startup(&mut self) {
        if let Some(url) = self.startup_url.take() {
            self.load(&url);
        }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn has_completed_document_load(&self) -> bool {
        unsafe { embedder::photon_automation_view_has_completed_document_load(self.view) }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn document_generation(&self) -> u64 {
        unsafe { embedder::photon_automation_view_document_generation(self.view) }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn install_speedometer_result_observer(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        unsafe {
            embedder::photon_automation_view_install_speedometer_result_observer(
                self.view,
                expected_document_generation,
            )
        }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn install_jetstream_result_observer(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        unsafe {
            embedder::photon_automation_view_install_jetstream_result_observer(
                self.view,
                expected_document_generation,
            )
        }
    }

    #[cfg(unix)]
    pub(in crate::platform) fn request_jetstream_start(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        unsafe {
            embedder::photon_automation_view_request_jetstream_start(
                self.view,
                expected_document_generation,
            )
        }
    }

    pub(in crate::platform) fn execute(&mut self, command: BrowserCommand) -> anyhow::Result<()> {
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

    pub(in crate::platform) fn set_ui_wake(&self, wake: UiWake) {
        self.callbacks.set_ui_wake(wake);
        unsafe { embedder::photon_view_notify_state(self.view) };
    }

    pub(in crate::platform) fn copy_window_handle(&self, buffer: *mut c_char, capacity: usize) {
        unsafe { embedder::photon_view_copy_window_handle(self.view, buffer, capacity) };
    }

    pub(in crate::platform) fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    pub(in crate::platform) fn set_focus(&mut self, focused: bool) {
        unsafe { embedder::photon_view_set_focus(self.view, focused) }
    }

    /// Answers the page's open dialog.
    pub(in crate::platform) fn reply_dialog(&mut self, reply: DialogReply) {
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
    pub(in crate::platform) fn find_in_page(&mut self, query: &str) {
        // Interior NULs cannot cross the C ABI; drop them.
        let Ok(query) = CString::new(query.replace('\0', "")) else {
            return;
        };
        unsafe { embedder::photon_view_find_in_page(self.view, query.as_ptr(), false, true) }
    }

    /// Moves to the next match, or the previous one when `forward` is false.
    pub(in crate::platform) fn find_in_page_step(&mut self, forward: bool) {
        unsafe { embedder::photon_view_find_in_page_step(self.view, forward) }
    }

    /// Zooms in a step, out a step, or back to 100%.
    pub(in crate::platform) fn zoom(&mut self, step: ZoomStep) {
        let step = match step {
            ZoomStep::In => 1,
            ZoomStep::Out => -1,
            ZoomStep::Reset => 0,
        };
        unsafe { embedder::photon_view_zoom(self.view, step) }
    }

    pub(in crate::platform) fn restart_unresponsive_page(&mut self) {
        unsafe { embedder::photon_view_restart_unresponsive_page(self.view) }
    }

    pub(in crate::platform) fn record_navigation_surface_installed(&self, navigation_id: u64) {
        self.callbacks
            .record_navigation_surface_installed(navigation_id);
    }

    pub(in crate::platform) fn record_navigation_surface_rendered(&self, navigation_id: u64) {
        self.callbacks
            .record_navigation_surface_rendered(navigation_id);
    }

    /// Runs an item of the context menu the page last asked for.
    pub(in crate::platform) fn activate_context_menu_item(&mut self, index: usize) {
        unsafe { embedder::photon_view_activate_context_menu_item(self.view, index) }
    }

    /// Answers the open `<select>` dropdown with the chosen option, or `None`
    /// when it was dismissed.
    pub(in crate::platform) fn select_dropdown_closed(&mut self, chosen: Option<u32>) {
        unsafe {
            embedder::photon_view_select_dropdown_closed(
                self.view,
                chosen.is_some(),
                chosen.unwrap_or(0),
            )
        }
    }

    /// Answers the open file picker with the chosen files; none when it was
    /// cancelled.
    pub(in crate::platform) fn file_picker_closed(&mut self, paths: &[std::path::PathBuf]) {
        let paths: Vec<CString> = paths
            .iter()
            .filter_map(|path| path_to_cstring(path))
            .collect();
        let pointers: Vec<*const c_char> = paths.iter().map(|path| path.as_ptr()).collect();
        unsafe {
            embedder::photon_view_file_picker_closed(self.view, pointers.as_ptr(), pointers.len())
        }
    }

    pub(in crate::platform) fn permission_request_completed(
        &mut self,
        request_id: u64,
        document_id: &str,
        granted: bool,
    ) {
        let Ok(document_id) = CString::new(document_id) else {
            return;
        };
        unsafe {
            embedder::photon_view_permission_decided(
                self.view,
                request_id,
                document_id.as_ptr(),
                granted,
            )
        }
    }

    pub(in crate::platform) fn permission_view_id(&self) -> u64 {
        self.callbacks.task_id
    }

    /// Ends the search and removes its highlights.
    pub(in crate::platform) fn find_in_page_end(&mut self) {
        unsafe { embedder::photon_view_find_in_page_end(self.view) }
    }

    /// Sets the color scheme pages see through `prefers-color-scheme`.
    pub(in crate::platform) fn set_dark_color_scheme(&mut self, dark: bool) {
        const DARK: i32 = 1;
        const LIGHT: i32 = 2;
        let scheme = if dark { DARK } else { LIGHT };
        unsafe { embedder::photon_view_set_preferred_color_scheme(self.view, scheme) }
    }

    pub(in crate::platform) fn set_visible(&mut self, visible: bool) {
        unsafe { embedder::photon_view_set_visible(self.view, visible) }
    }

    pub(in crate::platform) fn toggle_audio_mute(&mut self) -> bool {
        unsafe { embedder::photon_view_toggle_audio_mute(self.view) }
    }

    /// Re-reads the display's refresh rate the next time the view is laid out.
    pub(in crate::platform) fn forget_display(&mut self) {
        self.display = None;
    }

    /// Paces Engine rendering to the display that shows this view.
    pub(in crate::platform) fn set_display(&mut self, display: u64) {
        if self.display == Some(display) {
            return;
        }
        self.display = Some(display);
        let refresh_rate = super::super::display::refresh_rate(display);
        self.callbacks
            .performance
            .set_display_refresh_rate_hz(super::super::display::known_refresh_rate(display));
        trace(format_args!(
            "display={display} refresh-rate={refresh_rate:.2}Hz"
        ));
        unsafe { embedder::photon_view_set_display_metadata(self.view, display, refresh_rate) }
    }

    pub(in crate::platform) fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.callbacks.set_diagnostics_enabled(enabled);
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::platform) fn send_pointer(
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
        history_swipe_back_enabled: bool,
        history_swipe_forward_enabled: bool,
        history_swipe_gesture_id: u64,
    ) {
        if kind == 2 || kind == 4 {
            self.callbacks.mark_input();
        }
        unsafe {
            embedder::photon_view_pointer(
                self.view,
                kind,
                x,
                y,
                button,
                buttons,
                shift,
                control,
                alt,
                meta,
                wheel_x,
                wheel_y,
                precise,
                phase,
                clicks,
                history_swipe_back_enabled,
                history_swipe_forward_enabled,
                history_swipe_gesture_id,
            )
        }
    }

    pub(in crate::platform) fn send_key(
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

    pub(in crate::platform) fn drain_releases(&mut self) {
        #[cfg(target_os = "macos")]
        deliver_pending_releases(&self.callbacks);
    }

    pub(in crate::platform) fn take_presented_surface(&mut self) -> Option<PresentedSurface> {
        self.drain_releases();
        let presented = self.presentation.take_surface();
        if presented.is_none() {
            // A rejected frame completes its lease with no GPU work to follow.
            self.drain_releases();
        }
        presented
    }

    pub(in crate::platform) fn begin_shutdown_with_surface(
        &mut self,
        presented: Option<PresentedSurface>,
    ) {
        self.begin_shutdown(
            presented
                .map(PresentedSurface::into_release_keys)
                .unwrap_or_default(),
        );
    }

    pub(in crate::platform) fn begin_shutdown(&mut self, final_surface_releases: Vec<FrameKey>) {
        if self.shutdown_started {
            return;
        }
        #[cfg(target_os = "macos")]
        if !self.view.is_null() {
            unsafe {
                embedder::photon_view_set_native_metal_presentation(self.view, false);
            }
        }
        self.presentation.stop_accepting_frames();
        self.final_surface_releases = Some(final_surface_releases);
        self.shutdown_started = true;
    }

    pub(in crate::platform) fn finish_shutdown(&mut self) {
        if self.finished {
            return;
        }
        for key in self.final_surface_releases.take().unwrap_or_default() {
            self.leases.lock().unwrap().complete(key);
        }
        self.presentation.stop_accepting_frames();
        self.drain_releases();
        let leases = self.leases.lock().unwrap().counts();
        trace(format_args!(
            "shutdown accounting received={} accepted={} presented={} lease-completed={} released={} outstanding={}",
            leases.received,
            leases.accepted,
            leases.presented,
            leases.completed,
            leases.released,
            leases.outstanding,
        ));
        // A miscount here means a frame was lost, which is worth knowing but
        // not worth taking the browser down as a tab closes.
        let balanced = leases.outstanding == 0
            && leases.received == leases.completed
            && leases.completed == leases.released;
        if !balanced {
            trace(format_args!(
                "warning: shutdown accounting is unbalanced; presentation frames were left over"
            ));
        }
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
            self.begin_shutdown(Vec::new());
            self.finish_shutdown();
        }
    }
}
