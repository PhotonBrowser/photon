//! Performance diagnostics model and GPUI overlay for Photon.

mod accumulator;
mod overlay;

pub use accumulator::{EnginePerformanceStats, PerformanceDiagnostics, PerformanceMonitor};
pub use overlay::{PerformancePalette, performance_overlay};
