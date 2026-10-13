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

use super::super::ffi::embedder;
use super::super::presentation::{
    GpuActivity, LeaseLedger, PresentationRuntime, PresentedSurface, RetiredSurface,
};
use super::callbacks::*;
use super::{EngineRuntime, UiWake, ZoomStep};
use crate::platform::trace;

static NEXT_PERFORMANCE_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub(in crate::platform) struct EngineSession {
    _runtime: Rc<EngineRuntime>,
    view: *mut c_void,
    startup_url: Option<CString>,
    callbacks: Arc<CallbackState>,
    pub(in crate::platform) presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    pub(in crate::platform) gpu_activity: Arc<GpuActivity>,
    final_surface_releases: Option<Arc<Mutex<Vec<RetiredSurface>>>>,
    display: Option<u64>,
    pub(in crate::platform) shutdown_started: bool,
    finished: bool,
}

pub(in crate::platform) struct RequestedWebView {
    pub(in crate::platform) session: EngineSession,
    pub(in crate::platform) popup: bool,
    pub(in crate::platform) activate: bool,
    pub(in crate::platform) needs_confirmation: bool,
    pub(in crate::platform) width: Option<i32>,
    pub(in crate::platform) height: Option<i32>,
}

impl EngineSession {
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
        let service = std::env::var("PHOTON_PRESENTATION_XPC_SERVICE")
            .context("presentation broker service is not set")?;
        let channel_id = presentation_channel_id();
        let leases = Arc::new(Mutex::new(LeaseLedger::default()));
        let gpu_activity = Arc::new(GpuActivity::default());
        // Engine threads reach both through raw callback contexts and touch
        // only their mutex- and atomic-guarded state; the main-thread-only
        // fields (pixel buffers, the UI wake) are used on the main thread.
        #[expect(
            clippy::arc_with_non_send_sync,
            reason = "shared with Engine callbacks through a raw context pointer"
        )]
        let presentation = Arc::new(PresentationRuntime::new(
            &service,
            channel_id,
            leases.clone(),
            gpu_activity.clone(),
        )?);
        presentation
            .release_scheduler
            .store(runtime.runtime, Ordering::Release);
        #[expect(
            clippy::arc_with_non_send_sync,
            reason = "shared with Engine callbacks through a raw context pointer"
        )]
        let callbacks = Arc::new(CallbackState {
            task_id: NEXT_PERFORMANCE_TASK_ID.fetch_add(1, Ordering::Relaxed),
            presentation: presentation.clone(),
            leases: leases.clone(),
            engine_view: AtomicPtr::new(std::ptr::null_mut()),
            ui_wake: Mutex::new(None),
            performance: PerformanceMonitor::default(),
            page_details: Mutex::default(),
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

    pub(in crate::platform) fn navigate_startup(&mut self) {
        if let Some(url) = self.startup_url.take() {
            self.load(&url);
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
        use std::os::unix::ffi::OsStrExt;
        let paths: Vec<CString> = paths
            .iter()
            .filter_map(|path| CString::new(path.as_os_str().as_bytes()).ok())
            .collect();
        let pointers: Vec<*const c_char> = paths.iter().map(|path| path.as_ptr()).collect();
        unsafe {
            embedder::photon_view_file_picker_closed(self.view, pointers.as_ptr(), pointers.len())
        }
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
        let final_releases = if let Some(presented) = presented {
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
        self.begin_shutdown(final_releases);
    }

    pub(in crate::platform) fn begin_shutdown(
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

    pub(in crate::platform) fn finish_shutdown(&mut self) {
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
        // A miscount here means a frame or command buffer was lost, which is
        // worth knowing but not worth taking the browser down as a tab closes.
        let balanced = leases.outstanding == 0
            && leases.received == leases.completed
            && leases.completed == leases.released
            && gpu_submitted == gpu_completed
            && gpu_in_flight == 0;
        if !balanced {
            trace(format_args!(
                "warning: shutdown accounting is unbalanced; presentation frames or Metal work were left over"
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
            self.begin_shutdown(Arc::new(Mutex::new(Vec::new())));
            self.finish_shutdown();
        }
    }
}

/// This process's presentation channel: the one `photon run` names, or,
/// when macOS launches the app, as for a link from another app, one made
/// for this process alone.
fn presentation_channel_id() -> String {
    static CHANNEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CHANNEL
        .get_or_init(|| {
            std::env::var("PHOTON_PRESENTATION_CHANNEL_ID").unwrap_or_else(|_| {
                let started = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                format!("photon-gpui-ce-{}-{started}", std::process::id())
            })
        })
        .clone()
}
