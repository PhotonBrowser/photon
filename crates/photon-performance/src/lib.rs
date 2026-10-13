//! Performance diagnostics model and GPUI overlay for Photon.

mod accumulator;
mod chart;
mod overlay;
mod process;

pub use accumulator::{
    EnginePerformanceStats, PerformanceDiagnostics, PerformanceHistory, PerformanceMonitor,
    PerformanceSample, PerformanceTask, PresentationMode,
};
pub use chart::{ChartPoint, performance_chart};
pub use overlay::{PerformancePalette, format_bytes, format_rate, performance_overlay};
pub use process::{ProcessDiagnostics, ProcessSampler};
