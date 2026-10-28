//! GPUI components for Photon performance diagnostics.

mod chart;
mod overlay;

pub use chart::{ChartPoint, performance_chart};
pub use overlay::{PerformancePalette, performance_overlay};
