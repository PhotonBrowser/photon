use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

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
}

/// Collects Engine statistics and lightweight shell timing measurements.
#[derive(Default)]
struct PerformanceAccumulator {
    snapshot: PerformanceDiagnostics,
    last_input_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame_gaps: VecDeque<(Instant, f64)>,
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
    enabled: AtomicBool,
}

impl PerformanceMonitor {
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
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

    pub fn set_engine_stats(&self, stats: EnginePerformanceStats) {
        self.accumulator.lock().unwrap().set_engine_stats(stats);
    }

    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.accumulator.lock().unwrap().reset_timing();
        }
        self.enabled.store(enabled, Ordering::Release);
    }
}
