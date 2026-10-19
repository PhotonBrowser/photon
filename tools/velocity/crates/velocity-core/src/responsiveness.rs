//! Versioned reports for browser responsiveness diagnostics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use velocity_metrics::{MetricStatistics, ResourceReport};

use crate::ReportBrowser;

pub const RESPONSIVENESS_SCHEMA_VERSION: u32 = 2;
pub const MIN_SUPPORTED_RESPONSIVENESS_SCHEMA_VERSION: u32 = 1;
pub const TASK_BACKLOG_FIXTURE_REVISION: &str = "1";
pub const GC_PRESSURE_FIXTURE_REVISION: &str = "1";
pub const GC_PRESSURE_RECORD_COUNT: u64 = 800_000;
pub const DOM_STYLE_FIXTURE_REVISION: &str = "1";
pub const DOM_STYLE_MUTATION_COUNT: u64 = 1_024;
pub const STYLE_INVALIDATION_FIXTURE_REVISION: &str = "2";
pub const STYLE_INVALIDATION_MUTATION_COUNT: u64 = 1_024;
pub const LAYOUT_UPDATES_FIXTURE_REVISION: &str = "1";
pub const LAYOUT_UPDATE_COUNT: u64 = 512;
pub const ANIMATION_FIXTURE_REVISION: &str = "2";
pub const ANIMATION_PHASE_COUNT: u64 = 5;
pub const ANIMATION_PHASE_DURATION_MS: u64 = 900;
pub const ANIMATION_PRE_ROLL_MS: u64 = 900;
pub const FRAME_INTERVAL_PRE_ROLL_RULE_VERSION: u64 = 2;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponsivenessBenchmarkReport {
    pub benchmark: String,
    pub benchmark_type: String,
    pub schema_version: u32,
    pub run_id: String,
    pub timestamp_unix_ms: u128,
    pub browser: ReportBrowser,
    pub browser_version: Option<String>,
    pub engine_revision: Option<String>,
    pub executable_revision: Option<String>,
    pub workload: String,
    pub fixture_revision: String,
    pub configuration: Value,
    pub machine_information: ResponsivenessMachineInformation,
    pub display_refresh_rate_hz: Option<f64>,
    pub iterations: usize,
    pub warmup_count: usize,
    pub metrics: BTreeMap<String, MetricStatistics>,
    pub samples: Vec<ResponsivenessSample>,
    pub coverage: ResponsivenessCoverage,
    pub warnings: Vec<String>,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponsivenessMachineInformation {
    pub operating_system: String,
    pub architecture: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponsivenessSample {
    pub iteration: usize,
    pub warmup: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_pair_id: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics_enabled: Option<bool>,
    pub workload_duration_ms: Option<f64>,
    pub metrics: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_resources: Option<ResourceReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_resource_error: Option<String>,
    pub failure: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponsivenessCoverage {
    pub requested_iterations: usize,
    pub successful_iterations: usize,
    pub failed_iterations: usize,
    pub warmup_iterations: usize,
    pub process_scoped_snapshots: usize,
    pub complete_single_process_samples: usize,
    pub retained_task_samples: usize,
    #[serde(default)]
    pub style_layout_available_snapshots: usize,
    #[serde(default)]
    pub style_layout_incomplete_snapshots: usize,
    #[serde(default)]
    pub gc_timing_available_snapshots: usize,
    #[serde(default)]
    pub gc_live_heap_available_snapshots: usize,
    #[serde(default)]
    pub process_resource_iterations: usize,
    #[serde(default)]
    pub process_resource_complete_iterations: usize,
    #[serde(default)]
    pub process_resource_sample_count: usize,
    #[serde(default)]
    pub process_resource_incomplete_sample_count: usize,
    #[serde(default)]
    pub frame_timing_available_iterations: usize,
    #[serde(default)]
    pub actual_presentation_iterations: usize,
    #[serde(default)]
    pub frame_presentation_feedback_samples: usize,
    #[serde(default)]
    pub frame_interval_samples: usize,
    #[serde(default)]
    pub evicted_frame_timing_samples: usize,
}
