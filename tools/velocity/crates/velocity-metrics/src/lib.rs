//! Shared metric definitions and values used by benchmark results.

mod resource;

pub use resource::{
    AttributionEvidence, AttributionStatus, ManagedProcessTarget, MetricStatistics,
    ProcessAttributionDiagnostics, ProcessExclusionReason, ProcessLifecycleChange,
    ProcessLifecycleEvent, ProcessRegistrySnapshot, ProcessResource, ProcessResourceSample,
    ProcessResourceSnapshot, ProcessStartTime, RegisteredProcess, ResourceCollectionError,
    ResourceCollector, ResourceConfig, ResourceCoverage, ResourceErrorKind, ResourceMonitor,
    ResourceMonitorError, ResourceReport, ResourceSample, ResourceStatistics,
    summarize_metric_values, summarize_resource_samples,
};

use serde::{Deserialize, Serialize};

/// A unit understood by Velocity's initial metric model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricUnit {
    Nanoseconds,
    Milliseconds,
    Bytes,
    Percent,
    Count,
}

/// Describes a measurement independently of any particular result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricDefinition {
    pub name: String,
    pub unit: MetricUnit,
}

/// A numeric measurement with its name and unit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MetricValue {
    pub definition: MetricDefinition,
    pub value: f64,
}
