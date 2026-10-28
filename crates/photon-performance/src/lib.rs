//! Framework-independent performance diagnostics and data for Photon.

mod accumulator;
mod formatting;
mod process;

pub use accumulator::{
    DrawablePresentationEvent, EnginePerformanceStats, EngineSurfaceFrameTiming,
    FrameTimingDiagnostics, PerformanceDiagnostics, PerformanceHistory, PerformanceMonitor,
    PerformanceSample, PerformanceTask, PresentationMode,
};
pub use formatting::{format_bytes, format_rate};
pub use process::{ProcessDiagnostics, ProcessSampler};
