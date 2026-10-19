use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const FRAME_TIMING_SAMPLE_CAPACITY: usize = 384;
const DRAWABLE_PRESENTATION_EVENT_CAPACITY: usize = 768;
const GPU_SAMPLING_EVENT_CAPACITY: usize = 768;

/// One actual Metal drawable presentation callback associated with the Engine
/// surface that Photon sampled into that drawable. Invalid/zero Metal times are
/// retained as events with no timestamp so coverage loss remains visible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawablePresentationEvent {
    pub generation: u64,
    pub frame_id: u64,
    pub drawable_id: u64,
    pub presented_time_seconds: Option<f64>,
}

/// GPU timing for the command buffer that samples one Engine surface into the
/// named window drawable. The drawable ID connects submission, completion,
/// and presentation feedback without treating GPU completion as presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuSamplingEvent {
    pub generation: u64,
    pub frame_id: u64,
    pub drawable_id: u64,
    pub submitted_elapsed_ns: u64,
    pub completion_callback_elapsed_ns: Option<u64>,
}

/// Timing for one Engine surface frame, recorded entirely in the Photon
/// process. Metal's host presentation timestamp has a separate clock domain.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineSurfaceFrameTiming {
    pub generation: u64,
    pub frame_id: u64,
    pub handoff_elapsed_ns: Option<u64>,
    pub surface_install_elapsed_ns: Option<u64>,
    pub gpu_submission_elapsed_ns: Option<u64>,
    /// Time the CPU received Metal's command-buffer completion callback.
    pub gpu_completion_callback_elapsed_ns: Option<u64>,
    pub drawable_id: Option<u64>,
    /// Metal `presentedTime`, in host-time seconds. Zero is represented by
    /// `presentation_dropped`; it means the drawable was not displayed.
    pub presented_time_seconds: Option<f64>,
    pub presentation_feedback_received: bool,
    pub presentation_dropped: bool,
    pub presentation_feedback_count: u64,
    pub presentation_dropped_count: u64,
    pub coalesced_before_install: bool,
}

/// Bounded snapshot of the opt-in Photon/Metal frame timing session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameTimingDiagnostics {
    pub active: bool,
    pub measurement_window_ms: Option<f64>,
    pub display_refresh_rate_hz: Option<f64>,
    pub display_refresh_rate_change_count: u64,
    pub coalesced_before_install_count: u64,
    pub evicted_frame_sample_count: u64,
    pub evicted_presentation_event_count: u64,
    pub frames: Vec<EngineSurfaceFrameTiming>,
    pub presentation_events: Vec<DrawablePresentationEvent>,
    pub gpu_sampling_events: Vec<GpuSamplingEvent>,
    pub evicted_gpu_sampling_event_count: u64,
}

#[derive(Default)]
struct FrameTimingAccumulator {
    started_at: Option<Instant>,
    stopped_at: Option<Instant>,
    frames: VecDeque<EngineSurfaceFrameTiming>,
    display_refresh_rate_hz: Option<f64>,
    display_refresh_rate_change_count: u64,
    coalesced_before_install_count: u64,
    evicted_frame_sample_count: u64,
    presentation_events: VecDeque<DrawablePresentationEvent>,
    evicted_presentation_event_count: u64,
    gpu_sampling_events: VecDeque<GpuSamplingEvent>,
    evicted_gpu_sampling_event_count: u64,
}

impl FrameTimingAccumulator {
    fn begin(&mut self, display_refresh_rate_hz: Option<f64>) {
        *self = Self {
            started_at: Some(Instant::now()),
            display_refresh_rate_hz,
            ..Self::default()
        };
    }

    fn elapsed_ns(&self) -> Option<u64> {
        Some(self.started_at?.elapsed().as_nanos().min(u64::MAX as u128) as u64)
    }

    fn frame_mut(
        &mut self,
        generation: u64,
        frame_id: u64,
    ) -> Option<&mut EngineSurfaceFrameTiming> {
        if self.started_at.is_none() || self.stopped_at.is_some() {
            return None;
        }
        if let Some(index) = self
            .frames
            .iter()
            .position(|frame| frame.generation == generation && frame.frame_id == frame_id)
        {
            return self.frames.get_mut(index);
        }
        if self.frames.len() == FRAME_TIMING_SAMPLE_CAPACITY {
            self.frames.pop_front();
            self.evicted_frame_sample_count += 1;
        }
        self.frames.push_back(EngineSurfaceFrameTiming {
            generation,
            frame_id,
            ..EngineSurfaceFrameTiming::default()
        });
        self.frames.back_mut()
    }

    fn snapshot(&self) -> FrameTimingDiagnostics {
        let measurement_window_ms = self.started_at.map(|start| {
            self.stopped_at
                .unwrap_or_else(Instant::now)
                .saturating_duration_since(start)
                .as_secs_f64()
                * 1000.0
        });
        FrameTimingDiagnostics {
            active: self.started_at.is_some() && self.stopped_at.is_none(),
            measurement_window_ms,
            display_refresh_rate_hz: self.display_refresh_rate_hz,
            display_refresh_rate_change_count: self.display_refresh_rate_change_count,
            coalesced_before_install_count: self.coalesced_before_install_count,
            evicted_frame_sample_count: self.evicted_frame_sample_count,
            evicted_presentation_event_count: self.evicted_presentation_event_count,
            frames: self.frames.iter().copied().collect(),
            presentation_events: self.presentation_events.iter().copied().collect(),
            gpu_sampling_events: self.gpu_sampling_events.iter().copied().collect(),
            evicted_gpu_sampling_event_count: self.evicted_gpu_sampling_event_count,
        }
    }
}

/// A browser tab and the performance data its Engine view currently reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerformanceTask {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub diagnostics: PerformanceDiagnostics,
}

/// Values delivered by the Photon Engine performance monitor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnginePerformanceStats {
    pub frames_per_second: Option<f64>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub managed_heap_bytes: Option<u64>,
    pub download_bytes_per_second: u64,
    pub upload_bytes_per_second: u64,
}

/// The path used to deliver the most recently presented page frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationMode {
    NativeMetal,
    CpuFallback,
}

/// A point-in-time snapshot shown by the diagnostics overlay.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerformanceDiagnostics {
    pub frames_per_second: Option<f64>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub managed_heap_bytes: Option<u64>,
    pub download_bytes_per_second: u64,
    pub upload_bytes_per_second: u64,
    pub input_to_frame_latency_ms: Option<f64>,
    pub last_frame_interval_ms: Option<f64>,
    pub longest_frame_gap_ms: Option<f64>,
    pub presentation_mode: Option<PresentationMode>,
    pub cpu_fallback_buffer_allocation_microseconds: Option<u64>,
    pub cpu_fallback_copy_microseconds: Option<u64>,
}

/// Collects Engine statistics and lightweight shell timing measurements.
#[derive(Default)]
struct PerformanceAccumulator {
    snapshot: PerformanceDiagnostics,
    last_input_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame_gaps: VecDeque<(Instant, f64)>,
    frame_timing: FrameTimingAccumulator,
}

impl PerformanceAccumulator {
    fn snapshot(&self) -> PerformanceDiagnostics {
        self.snapshot.clone()
    }

    fn mark_input(&mut self) {
        self.last_input_at = Some(Instant::now());
    }

    fn record_frame(&mut self) {
        let now = Instant::now();
        if let Some(previous) = self.last_frame_at {
            let interval_ms = now.duration_since(previous).as_secs_f64() * 1000.0;
            self.snapshot.last_frame_interval_ms = Some(interval_ms);
            while self
                .frame_gaps
                .front()
                .is_some_and(|(at, _)| now.duration_since(*at).as_secs_f64() > 10.0)
            {
                self.frame_gaps.pop_front();
            }
            if interval_ms <= 10_000.0 {
                self.frame_gaps.push_back((now, interval_ms));
            }
            self.snapshot.longest_frame_gap_ms = self
                .frame_gaps
                .iter()
                .map(|(_, gap)| *gap)
                .max_by(f64::total_cmp);
        }
        self.last_frame_at = Some(now);
        if let Some(input_at) = self.last_input_at.take() {
            self.snapshot.input_to_frame_latency_ms =
                Some(now.duration_since(input_at).as_secs_f64() * 1000.0);
        }
    }

    fn set_engine_stats(&mut self, stats: EnginePerformanceStats) {
        self.snapshot.frames_per_second = stats.frames_per_second;
        self.snapshot.cpu_percent = stats.cpu_percent;
        self.snapshot.memory_bytes = stats.memory_bytes;
        self.snapshot.managed_heap_bytes = stats.managed_heap_bytes;
        self.snapshot.download_bytes_per_second = stats.download_bytes_per_second;
        self.snapshot.upload_bytes_per_second = stats.upload_bytes_per_second;
    }

    /// Starts a fresh timing window after the overlay has been enabled.
    fn reset_timing(&mut self) {
        self.last_frame_at = None;
        self.last_input_at = None;
        self.frame_gaps.clear();
        self.snapshot.last_frame_interval_ms = None;
        self.snapshot.longest_frame_gap_ms = None;
        self.snapshot.input_to_frame_latency_ms = None;
    }
}

/// Thread-safe performance collection and overlay enable state for one view.
#[derive(Default)]
pub struct PerformanceMonitor {
    accumulator: Mutex<PerformanceAccumulator>,
    frame_timing_active: Arc<AtomicBool>,
    overlay_enabled: AtomicBool,
    dashboard_enabled: AtomicBool,
    residency_enabled: AtomicBool,
}

impl PerformanceMonitor {
    pub fn is_enabled(&self) -> bool {
        self.overlay_enabled.load(Ordering::Acquire)
            || self.dashboard_enabled.load(Ordering::Acquire)
    }

    pub fn should_monitor_engine(&self) -> bool {
        self.is_enabled() || self.should_monitor_residency()
    }

    /// Keeps low-rate per-tab memory samples available to the residency policy.
    pub fn should_monitor_residency(&self) -> bool {
        self.residency_enabled.load(Ordering::Acquire)
    }

    /// Shared opt-in gate for Metal drawable presentation callbacks.
    pub fn frame_timing_enabled_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.frame_timing_active)
    }

    pub fn is_frame_timing_active(&self) -> bool {
        self.frame_timing_active.load(Ordering::Acquire)
    }

    pub fn snapshot(&self) -> PerformanceDiagnostics {
        self.accumulator.lock().unwrap().snapshot()
    }

    pub fn snapshot_if_enabled(&self) -> Option<PerformanceDiagnostics> {
        self.is_enabled().then(|| self.snapshot())
    }

    pub fn mark_input(&self) {
        if self.is_enabled() {
            self.accumulator.lock().unwrap().mark_input();
        }
    }

    pub fn record_frame(&self) {
        if self.is_enabled() {
            self.accumulator.lock().unwrap().record_frame();
        }
    }

    pub fn record_frame_with_presentation(
        &self,
        mode: PresentationMode,
        buffer_allocation_microseconds: Option<u64>,
        copy_microseconds: Option<u64>,
    ) {
        if !self.is_enabled() {
            return;
        }

        let mut accumulator = self.accumulator.lock().unwrap();
        accumulator.record_frame();
        accumulator.snapshot.presentation_mode = Some(mode);
        if let Some(microseconds) = buffer_allocation_microseconds {
            accumulator
                .snapshot
                .cpu_fallback_buffer_allocation_microseconds = Some(microseconds);
        }
        if let Some(microseconds) = copy_microseconds {
            accumulator.snapshot.cpu_fallback_copy_microseconds = Some(microseconds);
        }
    }

    pub fn set_engine_stats(&self, stats: EnginePerformanceStats) {
        self.accumulator.lock().unwrap().set_engine_stats(stats);
    }

    pub fn start_frame_timing(&self, display_refresh_rate_hz: Option<f64>) {
        self.accumulator
            .lock()
            .unwrap()
            .frame_timing
            .begin(display_refresh_rate_hz);
        self.frame_timing_active.store(true, Ordering::Release);
    }

    pub fn stop_frame_timing(&self) {
        let mut accumulator = self.accumulator.lock().unwrap();
        let timing = &mut accumulator.frame_timing;
        if timing.started_at.is_some() && timing.stopped_at.is_none() {
            timing.stopped_at = Some(Instant::now());
        }
        self.frame_timing_active.store(false, Ordering::Release);
    }

    pub fn frame_timing_snapshot(&self) -> FrameTimingDiagnostics {
        self.accumulator.lock().unwrap().frame_timing.snapshot()
    }

    pub fn set_display_refresh_rate_hz(&self, refresh_rate_hz: Option<f64>) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let timing = &mut accumulator.frame_timing;
        if timing.started_at.is_some()
            && timing.stopped_at.is_none()
            && timing.display_refresh_rate_hz != refresh_rate_hz
        {
            timing.display_refresh_rate_change_count += 1;
        }
        timing.display_refresh_rate_hz = refresh_rate_hz;
    }

    pub fn record_frame_handoff(&self, generation: u64, frame_id: u64) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let elapsed_ns = accumulator.frame_timing.elapsed_ns();
        if let Some(frame) = accumulator.frame_timing.frame_mut(generation, frame_id) {
            frame.handoff_elapsed_ns = elapsed_ns;
        }
    }

    pub fn record_surface_install(&self, generation: u64, frame_id: u64) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let elapsed_ns = accumulator.frame_timing.elapsed_ns();
        if let Some(frame) = accumulator.frame_timing.frame_mut(generation, frame_id) {
            frame.surface_install_elapsed_ns = elapsed_ns;
        }
    }

    pub fn record_gpu_submission(&self, generation: u64, frame_id: u64, drawable_id: u64) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let elapsed_ns = accumulator.frame_timing.elapsed_ns();
        if let Some(frame) = accumulator.frame_timing.frame_mut(generation, frame_id)
            && frame.gpu_submission_elapsed_ns.is_none()
        {
            frame.gpu_submission_elapsed_ns = elapsed_ns;
        }
        if let Some(submitted_elapsed_ns) = elapsed_ns {
            let timing = &mut accumulator.frame_timing;
            if timing.gpu_sampling_events.len() == GPU_SAMPLING_EVENT_CAPACITY {
                timing.gpu_sampling_events.pop_front();
                timing.evicted_gpu_sampling_event_count += 1;
            }
            timing.gpu_sampling_events.push_back(GpuSamplingEvent {
                generation,
                frame_id,
                drawable_id,
                submitted_elapsed_ns,
                completion_callback_elapsed_ns: None,
            });
        }
    }

    pub fn record_gpu_completion_callback(&self, generation: u64, frame_id: u64, drawable_id: u64) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let elapsed_ns = accumulator.frame_timing.elapsed_ns();
        let timing = &mut accumulator.frame_timing;
        let Some(elapsed_ns) = elapsed_ns else {
            return;
        };
        let Some(event_index) = timing.gpu_sampling_events.iter().rposition(|event| {
            event.generation == generation
                && event.frame_id == frame_id
                && event.drawable_id == drawable_id
                && event.completion_callback_elapsed_ns.is_none()
        }) else {
            return;
        };
        timing.gpu_sampling_events[event_index].completion_callback_elapsed_ns = Some(elapsed_ns);
        if let Some(frame) = timing.frame_mut(generation, frame_id)
            && frame.gpu_completion_callback_elapsed_ns.is_none()
        {
            frame.gpu_completion_callback_elapsed_ns = Some(elapsed_ns);
        }
    }

    pub fn record_drawable_presentation(
        &self,
        generation: u64,
        frame_id: u64,
        drawable_id: u64,
        presented_time_seconds: f64,
    ) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let timing = &mut accumulator.frame_timing;
        if let Some(frame) = timing.frame_mut(generation, frame_id) {
            frame.presentation_feedback_received = true;
            frame.presentation_feedback_count += 1;
            frame.drawable_id = Some(drawable_id);
            frame.presentation_dropped =
                !presented_time_seconds.is_finite() || presented_time_seconds <= 0.0;
            if frame.presentation_dropped {
                frame.presentation_dropped_count += 1;
            } else {
                frame.presented_time_seconds = Some(presented_time_seconds);
            }
            if timing.presentation_events.len() == DRAWABLE_PRESENTATION_EVENT_CAPACITY {
                timing.presentation_events.pop_front();
                timing.evicted_presentation_event_count += 1;
            }
            timing
                .presentation_events
                .push_back(DrawablePresentationEvent {
                    generation,
                    frame_id,
                    drawable_id,
                    presented_time_seconds: (presented_time_seconds.is_finite()
                        && presented_time_seconds > 0.0)
                        .then_some(presented_time_seconds),
                });
        }
    }

    pub fn record_frame_coalesced_before_install(&self, generation: u64, frame_id: u64) {
        if !self.frame_timing_active.load(Ordering::Acquire) {
            return;
        }
        let mut accumulator = self.accumulator.lock().unwrap();
        let timing = &mut accumulator.frame_timing;
        if timing.started_at.is_some() && timing.stopped_at.is_none() {
            timing.coalesced_before_install_count += 1;
            let elapsed_ns = timing.elapsed_ns();
            if let Some(frame) = timing.frame_mut(generation, frame_id) {
                frame.coalesced_before_install = true;
                if frame.handoff_elapsed_ns.is_none() {
                    frame.handoff_elapsed_ns = elapsed_ns;
                }
            }
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.set_source_enabled(&self.overlay_enabled, enabled);
    }

    /// Enables collection while the internal performance page is open.
    pub fn set_dashboard_enabled(&self, enabled: bool) {
        self.set_source_enabled(&self.dashboard_enabled, enabled);
    }

    /// Enables low-rate Engine sampling for memory-aware tab residency.
    pub fn set_residency_enabled(&self, enabled: bool) {
        self.residency_enabled.store(enabled, Ordering::Release);
    }

    fn set_source_enabled(&self, source: &AtomicBool, enabled: bool) {
        let was_enabled = self.is_enabled();
        source.store(enabled, Ordering::Release);
        if !was_enabled && self.is_enabled() {
            self.accumulator.lock().unwrap().reset_timing();
        }
    }
}

/// One point in the performance page's rolling history.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerformanceSample {
    pub elapsed_seconds: f64,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
}

/// A bounded history for the aggregate values shown by the performance page.
#[derive(Default)]
pub struct PerformanceHistory {
    started_at: Option<Instant>,
    samples: VecDeque<(Instant, PerformanceSample)>,
}

impl PerformanceHistory {
    const WINDOW_SECONDS: f64 = 60.0;

    pub fn push(&mut self, cpu_percent: Option<f64>, memory_bytes: Option<u64>) {
        let now = Instant::now();
        self.push_at(now, cpu_percent, memory_bytes);
    }

    fn push_at(&mut self, now: Instant, cpu_percent: Option<f64>, memory_bytes: Option<u64>) {
        let started_at = *self.started_at.get_or_insert(now);
        let sample = PerformanceSample {
            elapsed_seconds: now.duration_since(started_at).as_secs_f64(),
            cpu_percent,
            memory_bytes,
        };
        self.samples.push_back((now, sample));
        while self
            .samples
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at).as_secs_f64() > Self::WINDOW_SECONDS)
        {
            self.samples.pop_front();
        }
    }

    pub fn samples(&self) -> Vec<PerformanceSample> {
        self.samples.iter().map(|(_, sample)| *sample).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn performance_history_keeps_only_the_last_minute() {
        let start = Instant::now();
        let mut history = PerformanceHistory::default();
        history.push_at(start, Some(1.0), Some(10));
        history.push_at(start + Duration::from_secs(30), Some(2.0), Some(20));
        history.push_at(start + Duration::from_secs(61), Some(3.0), Some(30));

        let samples = history.samples();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].elapsed_seconds, 30.0);
        assert_eq!(samples[1].cpu_percent, Some(3.0));
    }

    #[test]
    fn diagnostics_remain_enabled_until_all_sources_are_disabled() {
        let monitor = PerformanceMonitor::default();
        monitor.set_enabled(true);
        monitor.set_dashboard_enabled(true);
        monitor.set_enabled(false);
        assert!(monitor.is_enabled());
        monitor.set_dashboard_enabled(false);
        assert!(!monitor.is_enabled());
    }

    #[test]
    fn residency_sampling_does_not_enable_visible_diagnostics() {
        let monitor = PerformanceMonitor::default();
        monitor.set_residency_enabled(true);

        assert!(!monitor.is_enabled());
        assert!(monitor.should_monitor_residency());
        assert!(monitor.should_monitor_engine());

        monitor.set_residency_enabled(false);
        assert!(!monitor.should_monitor_engine());
    }

    #[test]
    fn drawable_feedback_gate_tracks_diagnostic_session_lifecycle() {
        let monitor = PerformanceMonitor::default();
        let enabled = monitor.frame_timing_enabled_flag();
        assert!(!enabled.load(Ordering::Acquire));

        monitor.start_frame_timing(None);
        assert!(enabled.load(Ordering::Acquire));
        monitor.stop_frame_timing();
        assert!(!enabled.load(Ordering::Acquire));
    }

    #[test]
    fn frame_timing_preserves_stage_order_feedback_and_bounded_samples() {
        let monitor = PerformanceMonitor::default();
        monitor.record_frame_handoff(1, 1);
        monitor.record_drawable_presentation(1, 1, 1, 123.0);
        assert!(monitor.frame_timing_snapshot().frames.is_empty());
        assert!(
            monitor
                .frame_timing_snapshot()
                .presentation_events
                .is_empty()
        );

        monitor.start_frame_timing(Some(120.0));
        monitor.record_frame_handoff(1, 42);
        monitor.record_surface_install(1, 42);
        monitor.record_gpu_submission(1, 42, 7);
        monitor.record_gpu_completion_callback(1, 42, 7);
        monitor.record_gpu_submission(1, 42, 8);
        monitor.record_gpu_completion_callback(1, 42, 8);
        monitor.record_drawable_presentation(1, 42, 7, 123.25);
        monitor.record_drawable_presentation(1, 42, 8, 0.0);
        monitor.record_frame_handoff(1, 43);
        monitor.record_frame_coalesced_before_install(1, 43);
        monitor.set_display_refresh_rate_hz(Some(60.0));

        let snapshot = monitor.frame_timing_snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.display_refresh_rate_hz, Some(60.0));
        assert_eq!(snapshot.display_refresh_rate_change_count, 1);
        assert_eq!(snapshot.coalesced_before_install_count, 1);
        assert_eq!(snapshot.frames.len(), 2);
        let frame = snapshot.frames[0];
        assert_eq!(frame.frame_id, 42);
        assert!(frame.handoff_elapsed_ns <= frame.surface_install_elapsed_ns);
        assert!(frame.surface_install_elapsed_ns <= frame.gpu_submission_elapsed_ns);
        assert!(frame.gpu_submission_elapsed_ns <= frame.gpu_completion_callback_elapsed_ns);
        assert_eq!(frame.presented_time_seconds, Some(123.25));
        assert_eq!(frame.presentation_feedback_count, 2);
        assert_eq!(frame.presentation_dropped_count, 1);
        assert_eq!(snapshot.presentation_events.len(), 2);
        assert_eq!(snapshot.presentation_events[0].drawable_id, 7);
        assert_eq!(
            snapshot.presentation_events[0].presented_time_seconds,
            Some(123.25)
        );
        assert_eq!(snapshot.presentation_events[1].presented_time_seconds, None);
        assert_eq!(snapshot.gpu_sampling_events.len(), 2);
        assert_eq!(snapshot.gpu_sampling_events[0].drawable_id, 7);
        assert!(
            snapshot.gpu_sampling_events[0]
                .completion_callback_elapsed_ns
                .is_some()
        );
        assert_eq!(snapshot.gpu_sampling_events[1].drawable_id, 8);
        assert!(
            snapshot.gpu_sampling_events[1]
                .completion_callback_elapsed_ns
                .is_some()
        );
        assert!(snapshot.frames[1].coalesced_before_install);

        monitor.stop_frame_timing();
        monitor.record_frame_handoff(1, 44);
        monitor.record_drawable_presentation(1, 42, 9, 124.0);
        let stopped = monitor.frame_timing_snapshot();
        assert!(!stopped.active);
        assert_eq!(stopped.frames.len(), 2);
        assert_eq!(stopped.presentation_events.len(), 2);
    }

    #[test]
    fn frame_timing_ring_reports_evicted_samples() {
        let monitor = PerformanceMonitor::default();
        monitor.start_frame_timing(None);
        for frame_id in 1..=FRAME_TIMING_SAMPLE_CAPACITY as u64 + 3 {
            monitor.record_frame_handoff(1, frame_id);
        }

        let snapshot = monitor.frame_timing_snapshot();
        assert_eq!(snapshot.frames.len(), FRAME_TIMING_SAMPLE_CAPACITY);
        assert_eq!(snapshot.evicted_frame_sample_count, 3);
        assert_eq!(snapshot.frames[0].frame_id, 4);
    }

    #[test]
    fn drawable_presentation_ring_keeps_reused_surface_events_and_reports_evictions() {
        let monitor = PerformanceMonitor::default();
        monitor.start_frame_timing(None);
        for index in 0..DRAWABLE_PRESENTATION_EVENT_CAPACITY + 3 {
            monitor.record_drawable_presentation(2, 42, index as u64 + 1, 100.0 + index as f64);
        }

        let snapshot = monitor.frame_timing_snapshot();
        assert_eq!(
            snapshot.presentation_events.len(),
            DRAWABLE_PRESENTATION_EVENT_CAPACITY
        );
        assert_eq!(snapshot.evicted_presentation_event_count, 3);
        assert_eq!(snapshot.presentation_events[0].frame_id, 42);
        assert_eq!(snapshot.presentation_events[0].drawable_id, 4);
        assert_eq!(
            snapshot.presentation_events[0].presented_time_seconds,
            Some(103.0)
        );
        assert_eq!(
            snapshot.frames[0].presentation_feedback_count,
            (DRAWABLE_PRESENTATION_EVENT_CAPACITY + 3) as u64
        );
    }
}
