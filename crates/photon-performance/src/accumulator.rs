use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

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
    cpu_sample_seen: bool,
    last_activity_at: Option<Instant>,
    activity_monitor_started_at: Option<Instant>,
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
        self.cpu_sample_seen |= stats.cpu_percent.is_some();
        if stats.cpu_percent.is_some_and(|cpu| cpu > 0.0)
            || stats.download_bytes_per_second > 0
            || stats.upload_bytes_per_second > 0
        {
            self.last_activity_at = Some(Instant::now());
        }
        self.snapshot.frames_per_second = stats.frames_per_second;
        self.snapshot.cpu_percent = stats.cpu_percent;
        self.snapshot.memory_bytes = stats.memory_bytes;
        self.snapshot.managed_heap_bytes = stats.managed_heap_bytes;
        self.snapshot.download_bytes_per_second = stats.download_bytes_per_second;
        self.snapshot.upload_bytes_per_second = stats.upload_bytes_per_second;
    }

    fn activity_status(&self, recent_for: Duration) -> Option<bool> {
        self.cpu_sample_seen.then(|| {
            self.last_activity_at
                .is_some_and(|at| at.elapsed() <= recent_for)
        })
    }

    fn activity_idle_for(&self) -> Option<Duration> {
        if !self.cpu_sample_seen {
            return None;
        }
        self.last_activity_at
            .or(self.activity_monitor_started_at)
            .map(|at| at.elapsed())
    }

    fn reset_activity_tracking(&mut self, enabled: bool) {
        self.cpu_sample_seen = false;
        self.last_activity_at = None;
        self.activity_monitor_started_at = enabled.then(Instant::now);
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
    overlay_enabled: AtomicBool,
    dashboard_enabled: AtomicBool,
    activity_monitor_enabled: AtomicBool,
}

impl PerformanceMonitor {
    pub fn is_enabled(&self) -> bool {
        self.overlay_enabled.load(Ordering::Acquire)
            || self.dashboard_enabled.load(Ordering::Acquire)
    }

    pub fn is_activity_monitor_enabled(&self) -> bool {
        self.activity_monitor_enabled.load(Ordering::Acquire)
    }

    pub fn should_monitor_engine(&self) -> bool {
        self.is_enabled() || self.is_activity_monitor_enabled()
    }

    /// Returns whether this page used CPU or the network recently. `None`
    /// means the Engine has not produced an attributable CPU sample yet.
    pub fn activity_status(&self, recent_for: Duration) -> Option<bool> {
        self.accumulator.lock().unwrap().activity_status(recent_for)
    }

    /// Returns how long this page has gone without measured CPU or network
    /// activity. `None` means the Engine has not produced an attributable CPU
    /// sample yet.
    pub fn activity_idle_for(&self) -> Option<Duration> {
        self.accumulator.lock().unwrap().activity_idle_for()
    }

    /// Starts or stops the lightweight Engine sampling needed by tab sleeping.
    pub fn set_activity_monitor_enabled(&self, enabled: bool) -> bool {
        let changed = self
            .activity_monitor_enabled
            .swap(enabled, Ordering::AcqRel)
            != enabled;
        if changed {
            self.accumulator
                .lock()
                .unwrap()
                .reset_activity_tracking(enabled);
        }
        changed
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

    pub fn set_enabled(&self, enabled: bool) {
        self.set_source_enabled(&self.overlay_enabled, enabled);
    }

    /// Enables collection while the internal performance page is open.
    pub fn set_dashboard_enabled(&self, enabled: bool) {
        self.set_source_enabled(&self.dashboard_enabled, enabled);
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
}
