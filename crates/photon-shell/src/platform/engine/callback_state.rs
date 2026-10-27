//! What a tab's Engine callbacks share: the page state they update on the
//! UI thread, the presentation they feed, and the frames they release.

use crate::platform::app_update::update_app;
use gpui::Context;
use photon_core::{BrowserState, DialogRequest, EngineEvent};
use photon_performance::{
    EnginePerformanceStats, PerformanceMonitor, PerformanceTask, PresentationMode,
};
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    },
};

#[cfg(target_os = "macos")]
use super::super::presentation::LeaseLedger;
use super::super::presentation::PresentationRuntime;
use super::UiWake;
use super::navigation_timing::NavigationTimingProfiler;
use crate::platform::engine_ffi::embedder;
#[cfg(target_os = "macos")]
use crate::platform::trace;
use crate::platform::ui::{Favicon, FindResult, PhotonWebView};

#[derive(Clone, Default)]
pub(super) struct PageDetails {
    title: String,
    url: String,
    loading: bool,
}

pub(super) struct CallbackState {
    /// Lets a frame's GPU-completion wakeup reach this state.
    pub(super) self_ref: Weak<CallbackState>,
    pub(super) task_id: u64,
    pub(super) presentation: Arc<PresentationRuntime>,
    #[cfg(target_os = "macos")]
    pub(super) leases: Arc<Mutex<LeaseLedger>>,
    pub(super) engine_view: AtomicPtr<c_void>,
    pub(super) ui_wake: Mutex<Option<UiWake>>,
    /// At most one app task is scheduled or draining the latest page frame.
    pub(super) redraw_pending: Arc<AtomicBool>,
    pub(super) performance: Arc<PerformanceMonitor>,
    pub(super) page_details: Mutex<PageDetails>,
    pub(super) navigation_timing: Option<Arc<NavigationTimingProfiler>>,
}

pub(super) struct RuntimeCallbacks {
    pub(super) views: Mutex<Vec<Weak<CallbackState>>>,
}

impl CallbackState {
    pub(super) fn set_ui_wake(&self, wake: UiWake) {
        *self.ui_wake.lock().unwrap() = Some(wake);
        self.request_redraw();
    }

    /// Presents the newest page frame once its producer's GPU work is done.
    pub(super) fn request_redraw_when_frame_finished(&self) {
        let callbacks = self.self_ref.clone();
        let waiting = self.presentation.notify_when_latest_finished(move || {
            if let Some(callbacks) = callbacks.upgrade() {
                callbacks.request_redraw();
            }
        });
        if !waiting {
            self.request_redraw();
        }
    }

    pub(super) fn request_redraw(&self) {
        let Some(wake) = self.ui_wake.lock().unwrap().clone() else {
            return;
        };
        if self.redraw_pending.swap(true, Ordering::AcqRel) {
            return;
        }

        let redraw_pending = self.redraw_pending.clone();
        let presentation = self.presentation.clone();
        let performance = self.performance.clone();
        wake.app
            .spawn(async move |cx| {
                let Some(webview) = wake.webview.upgrade() else {
                    redraw_pending.store(false, Ordering::Release);
                    return;
                };

                loop {
                    let updated = update_app(cx, |cx| {
                        webview.update(cx, |view, cx| {
                            view.present_latest(cx);
                            if let Some(diagnostics) = performance.snapshot_if_enabled()
                                && view.diagnostics != diagnostics
                            {
                                view.diagnostics = diagnostics;
                                if view.performance_overlay_enabled {
                                    cx.notify();
                                }
                            }
                        })
                    })
                    .await;
                    if updated.is_none() {
                        redraw_pending.store(false, Ordering::Release);
                        return;
                    }

                    // Clear before checking the surface queue. A callback that
                    // arrives after this point either schedules the next task
                    // itself or leaves a frame for this task to drain.
                    redraw_pending.store(false, Ordering::Release);
                    if !presentation.has_pending_frame()
                        || redraw_pending
                            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                            .is_err()
                    {
                        return;
                    }
                }
            })
            .detach();
    }

    pub(super) fn set_page_state(&self, state: BrowserState) {
        *self.page_details.lock().unwrap() = PageDetails {
            title: state.title.clone(),
            url: state.url.clone(),
            loading: state.loading,
        };
        self.update_webview(move |view, cx| view.set_state(state, cx));
    }

    pub(super) fn record_navigation_timing(&self, event: i32, url: &str) {
        if let Some(profiler) = &self.navigation_timing {
            profiler.record_event(event, url);
        }
    }

    pub(super) fn record_navigation_surface_installed(&self, navigation_id: u64) {
        if let Some(profiler) = &self.navigation_timing {
            profiler.record_surface_installed(navigation_id);
        }
    }

    pub(super) fn record_navigation_surface_rendered(&self, navigation_id: u64) {
        if let Some(profiler) = &self.navigation_timing {
            profiler.record_surface_rendered(navigation_id);
        }
    }

    pub(super) fn set_page_error(&self, message: String) {
        self.update_webview(move |view, cx| {
            view.state.apply(EngineEvent::LoadFailed(message));
            view.history_swipe_navigation_failed();
            view.state_changed(cx);
        });
    }

    pub(super) fn set_page_favicon(&self, favicon: Option<Favicon>) {
        self.update_webview(move |view, cx| view.set_favicon(favicon, cx));
    }

    pub(super) fn set_audio_state(&self, playing: bool, muted: bool) {
        self.update_webview(move |view, cx| view.set_audio_state(playing, muted, cx));
    }

    pub(super) fn navigation_committed(&self) {
        self.update_webview(|view, cx| {
            view.cancel_file_picker(cx);
            view.dialogs.navigation_committed();
            view.history_swipe_navigation_committed();
        });
    }

    pub(super) fn history_swipe_candidate(
        &self,
        gesture_id: u64,
        back: bool,
        unconsumed_delta: f64,
    ) {
        self.update_webview(move |view, cx| {
            view.history_swipe_candidate(gesture_id, back, unconsumed_delta as f32, cx);
        });
    }

    pub(super) fn request_page_dialog(&self, request: DialogRequest) {
        self.update_webview(move |view, cx| view.request_dialog(request, cx));
    }

    pub(super) fn handle_engine_crash(&self, failed_url: Option<String>) {
        self.update_webview(move |view, cx| view.handle_engine_crash(failed_url, cx));
    }

    pub(super) fn set_find_result(&self, result: FindResult) {
        self.update_webview(move |view, cx| view.set_find_result(result, cx));
    }

    pub(super) fn set_zoom_level(&self, zoom_level: f64) {
        self.update_webview(move |view, cx| view.set_zoom_level(zoom_level, cx));
    }

    pub(super) fn set_page_unresponsive(&self, unresponsive: bool) {
        self.update_webview(move |view, cx| view.set_page_unresponsive(unresponsive, cx));
    }

    pub(super) fn crash_recovered(&self) {
        self.update_webview(|view, cx| view.crash_recovered(cx));
    }

    pub(super) fn mark_input(&self) {
        self.performance.mark_input();
    }

    /// Tracks frame gaps and input latency, which only the diagnostics
    /// overlay shows, so frames skip this while it is hidden.
    pub(super) fn record_cpu_fallback_frame(
        &self,
        buffer_allocation_microseconds: u64,
        copy_microseconds: u64,
    ) {
        self.performance.record_frame_with_presentation(
            PresentationMode::CpuFallback,
            Some(buffer_allocation_microseconds),
            Some(copy_microseconds),
        );
    }

    #[cfg(target_os = "macos")]
    pub(super) fn record_native_frame(&self) {
        self.performance
            .record_frame_with_presentation(PresentationMode::NativeMetal, None, None);
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
        self.sync_engine_monitor();
        if enabled {
            self.publish_diagnostics();
        }
    }

    pub(super) fn set_dashboard_enabled(&self, enabled: bool) {
        self.performance.set_dashboard_enabled(enabled);
        self.sync_engine_monitor();
        if enabled {
            self.publish_diagnostics();
        }
    }

    pub(super) fn set_residency_monitor_enabled(&self, enabled: bool) {
        self.performance.set_residency_enabled(enabled);
        self.sync_engine_monitor();
    }

    pub(super) fn performance_task(&self) -> PerformanceTask {
        let details = self.page_details.lock().unwrap().clone();
        PerformanceTask {
            id: self.task_id,
            title: details.title,
            url: details.url,
            loading: details.loading,
            diagnostics: self.performance.snapshot(),
        }
    }

    fn sync_engine_monitor(&self) {
        let view = self.engine_view.load(Ordering::Acquire);
        if !view.is_null() {
            unsafe {
                embedder::photon_view_set_performance_monitor_enabled(
                    view,
                    self.performance.is_enabled(),
                );
                embedder::photon_view_set_residency_monitor_enabled(
                    view,
                    self.performance.should_monitor_residency(),
                );
            }
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
                update_app(cx, |cx| webview.update(cx, update)).await;
            })
            .detach();
    }
}

#[cfg(target_os = "macos")]
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
                release.consumer_gpu_work_complete,
            );
        }
        trace(format_args!(
            "FrameReleased frame={} gen={} delivered-to-engine",
            release.key.frame, release.key.generation
        ));
        callbacks.leases.lock().unwrap().release(release.key);
    }
}
