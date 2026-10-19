//! Shared benchmark definitions, configuration, and structured results.

use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

mod startup;
pub use startup::*;
mod resource_benchmark;
pub use resource_benchmark::*;
mod responsiveness;
pub use responsiveness::*;

pub use velocity_browser::{BrowserId, InstallationId};
pub use velocity_metrics::{MetricDefinition, MetricUnit, MetricValue};

/// Name shown by the command-line application and other front ends.
pub const FRAMEWORK_NAME: &str = "Velocity";

/// A stable identifier for a benchmark.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BenchmarkId(pub String);

/// Broad workload category for organizing benchmark definitions and reports.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkCategory {
    Startup,
    PageLoading,
    Cpu,
    Memory,
    JavaScript,
    Rendering,
    Tabs,
    Idle,
}

/// Human-readable metadata for a benchmark workload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkDefinition {
    pub id: BenchmarkId,
    pub name: String,
    pub description: String,
    pub category: BenchmarkCategory,
}

/// A request to run one benchmark against one browser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkRun {
    pub definition: BenchmarkDefinition,
    pub browser_id: BrowserId,
    pub config: BenchmarkConfig,
}

/// Settings shared by future benchmark runs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkConfig {
    iterations: NonZeroUsize,
}

impl BenchmarkConfig {
    /// Creates a configuration with the requested number of measured iterations.
    pub fn new(iterations: usize) -> Result<Self, BenchmarkConfigError> {
        let Some(iterations) = NonZeroUsize::new(iterations) else {
            return Err(BenchmarkConfigError::ZeroIterations);
        };

        Ok(Self { iterations })
    }

    /// Returns the number of measured iterations.
    pub fn iterations(self) -> usize {
        self.iterations.get()
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: NonZeroUsize::new(5).expect("five is non-zero"),
        }
    }
}

/// Validation errors for benchmark configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkConfigError {
    ZeroIterations,
}

impl std::fmt::Display for BenchmarkConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroIterations => formatter.write_str("iterations must be greater than zero"),
        }
    }
}

impl std::error::Error for BenchmarkConfigError {}

/// An error returned by a benchmark executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BenchmarkExecutionError {
    pub message: String,
}

impl std::fmt::Display for BenchmarkExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BenchmarkExecutionError {}

/// Measurements collected for one iteration of one benchmark on one browser.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BenchmarkResult {
    pub benchmark_id: BenchmarkId,
    pub browser_id: BrowserId,
    pub iteration: NonZeroUsize,
    pub metrics: Vec<MetricValue>,
    pub unavailable_metrics: Vec<UnavailableMetric>,
    pub failures: Vec<BenchmarkFailure>,
}

/// Executes a requested workload and returns its per-iteration results.
///
/// Executes a requested workload and returns its per-iteration results.
pub trait BenchmarkExecutor {
    fn execute(
        &mut self,
        run: &BenchmarkRun,
    ) -> Result<Vec<BenchmarkResult>, BenchmarkExecutionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_requires_at_least_one_iteration() {
        assert_eq!(
            BenchmarkConfig::new(0),
            Err(BenchmarkConfigError::ZeroIterations)
        );
        assert_eq!(BenchmarkConfig::new(3).unwrap().iterations(), 3);
        assert_eq!(BenchmarkConfig::default().iterations(), 5);
    }
}
