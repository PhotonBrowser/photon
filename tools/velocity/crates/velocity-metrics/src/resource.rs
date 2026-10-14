use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

/// Native process start time at microsecond resolution, used with PID to avoid
/// confusing a recycled PID with the original process.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProcessStartTime {
    pub seconds: u64,
    pub microseconds: u64,
}

/// Root identity of a browser session launched and owned by Velocity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManagedProcessTarget {
    pub root_pid: u32,
    pub session_id: u32,
    pub root_start_time: ProcessStartTime,
    pub executable_path: PathBuf,
    #[serde(default)]
    pub application_path: PathBuf,
    #[serde(default)]
    pub isolated_profile_path: Option<PathBuf>,
    #[serde(default)]
    pub allow_profile_argument_attribution: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionStatus {
    VerifiedComplete,
    VerifiedPartial,
    #[default]
    Unknown,
    Unsupported,
}

impl AttributionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::VerifiedComplete => "VERIFIED COMPLETE",
            Self::VerifiedPartial => "VERIFIED PARTIAL",
            Self::Unknown => "UNKNOWN",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionEvidence {
    ManagedRootIdentity,
    AncestryAndPrivateSession,
    PrivateSessionMembership,
    PreviouslyVerifiedSessionMembership,
    ChromiumProfileArgumentAndBundle,
    PhotonEngineRegistryAndPrivateSession,
    PhotonEngineRegistryParentIdentityAndPrivateSession,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessExclusionReason {
    DifferentIsolatedProfile,
    OwnershipEvidenceUnavailable,
    IdentityChanged,
    ExitedDuringDiscovery,
    ProcessLimitExceeded,
    SessionMismatch,
    RegistryIdentityMismatch,
    ExecutableMismatch,
    DuplicateRegistration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessLifecycleChange {
    Started,
    Exited,
    Replaced,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProcessLifecycleEvent {
    pub change: ProcessLifecycleChange,
    pub pid: u32,
    pub start_time: ProcessStartTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessAttributionDiagnostics {
    pub status: AttributionStatus,
    pub verified_processes: usize,
    pub excluded_process_candidates: usize,
    pub uncertain_processes: usize,
    pub exited_processes: usize,
    pub identity_verification_failures: usize,
    #[serde(default)]
    pub evidence: BTreeMap<AttributionEvidence, usize>,
    #[serde(default)]
    pub exclusion_reasons: BTreeMap<ProcessExclusionReason, usize>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub registry_scope: Option<String>,
    #[serde(default)]
    pub registry_source: Option<String>,
    #[serde(default)]
    pub registered_processes: usize,
    #[serde(default)]
    pub independently_verified_registered_processes: usize,
    #[serde(default)]
    pub registry_snapshot_complete: bool,
    #[serde(default)]
    pub lifecycle_events: Vec<ProcessLifecycleEvent>,
}

/// A read-only process registration supplied by an authenticated browser
/// adapter. The collector still verifies this PID against current OS identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegisteredProcess {
    pub pid: u32,
    pub role: String,
    #[serde(default)]
    pub start_time: Option<ProcessStartTime>,
    #[serde(default)]
    pub launch_generation: Option<String>,
    #[serde(default)]
    pub parent_pid: Option<u32>,
    #[serde(default)]
    pub parent_start_time: Option<ProcessStartTime>,
}

/// An authoritative snapshot of a browser's own active process registry.
/// `complete` applies only to the explicitly named scope, not to arbitrary
/// system helpers outside that registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessRegistrySnapshot {
    pub scope: String,
    pub source: String,
    pub complete: bool,
    pub processes: Vec<RegisteredProcess>,
}

/// Sampling configuration. Intervals are stored in milliseconds for a stable
/// serialized representation and validated before a monitor starts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceConfig {
    pub sampling_interval_ms: u64,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            sampling_interval_ms: 100,
        }
    }
}

impl ResourceConfig {
    pub fn validate(self) -> Result<Self, ResourceMonitorError> {
        if self.sampling_interval_ms == 0 {
            return Err(ResourceMonitorError::InvalidConfiguration(
                "sampling interval must be greater than zero".to_owned(),
            ));
        }
        Ok(self)
    }

    pub fn interval(self) -> Duration {
        Duration::from_millis(self.sampling_interval_ms)
    }
}

/// Process resource counters collected at one instant. CPU time is cumulative
/// nanoseconds; memory quantities are bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessResource {
    pub pid: u32,
    pub start_time: ProcessStartTime,
    pub parent_pid: Option<u32>,
    #[serde(default)]
    pub parent_start_time: Option<ProcessStartTime>,
    #[serde(default)]
    pub executable_path: Option<PathBuf>,
    #[serde(default)]
    pub process_group_id: Option<u32>,
    #[serde(default)]
    pub process_session_id: Option<u32>,
    #[serde(default)]
    pub attribution_evidence: Option<AttributionEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub cpu_time_ns: Option<u64>,
    pub physical_footprint_bytes: Option<u64>,
    pub resident_bytes: Option<u64>,
}

impl ProcessResource {
    fn key(&self) -> ProcessKey {
        ProcessKey {
            pid: self.pid,
            start_time: self.start_time,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProcessKey {
    pid: u32,
    start_time: ProcessStartTime,
}

/// One process-tree collection. A collector returns partial data with warnings
/// when individual process queries fail, and an error only when the managed
/// session root cannot be verified.
#[derive(Clone, Debug, Default)]
pub struct ProcessResourceSnapshot {
    pub processes: Vec<ProcessResource>,
    pub warnings: Vec<String>,
    pub attribution: ProcessAttributionDiagnostics,
}

/// Platform or browser-specific adapter for collecting a verified managed
/// browser process tree. Specialized collectors may add attribution rules only
/// when they can verify ownership; process names alone are not sufficient.
pub trait ResourceCollector {
    fn collect(
        &mut self,
        target: &ManagedProcessTarget,
    ) -> Result<ProcessResourceSnapshot, ResourceCollectionError>;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceErrorKind {
    ProcessUnavailable,
    ProcessIdentityMismatch,
    SessionMismatch,
    ProcessEnumeration,
    ResourceUnavailable,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceCollectionError {
    pub kind: ResourceErrorKind,
    pub message: String,
}

impl ResourceCollectionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: ResourceErrorKind::Other,
            message: message.into(),
        }
    }

    pub fn with_kind(kind: ResourceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for ResourceCollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ResourceCollectionError {}

/// How much of the managed process tree could be verified for this sample.
/// `SessionScoped` means every included process was verified as a descendant
/// or a previously verified member of Velocity's private session; it does not
/// imply that out-of-session browser services were found.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceCoverage {
    SessionScoped,
    Partial,
    Unavailable,
}

/// A single timestamped resource sample. Missing values remain `None` and are
/// never interpreted as zero.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceSample {
    pub elapsed_ms: f64,
    pub interval_ms: Option<f64>,
    pub wall_clock_unix_ms: Option<u128>,
    pub cpu_percent: Option<f64>,
    pub physical_footprint_bytes: Option<u64>,
    pub resident_bytes: Option<u64>,
    pub process_count: usize,
    pub coverage: ResourceCoverage,
    #[serde(default)]
    pub attribution: ProcessAttributionDiagnostics,
    pub processes: Vec<ProcessResourceSample>,
    pub warnings: Vec<String>,
    pub collection_error: Option<ResourceCollectionError>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProcessResourceSample {
    pub pid: u32,
    pub start_time: ProcessStartTime,
    pub parent_pid: Option<u32>,
    #[serde(default)]
    pub parent_start_time: Option<ProcessStartTime>,
    #[serde(default)]
    pub executable_path: Option<PathBuf>,
    #[serde(default)]
    pub process_group_id: Option<u32>,
    #[serde(default)]
    pub process_session_id: Option<u32>,
    #[serde(default)]
    pub attribution_evidence: Option<AttributionEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub cpu_percent: Option<f64>,
    pub physical_footprint_bytes: Option<u64>,
    pub resident_bytes: Option<u64>,
}

/// Summary over observed samples. CPU and memory values only include samples
/// where that metric was available.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResourceStatistics {
    pub attempted_samples: usize,
    pub valid_samples: usize,
    pub failed_samples: usize,
    pub cpu_samples: usize,
    pub physical_footprint_samples: usize,
    pub resident_samples: usize,
    pub monitoring_duration_ms: f64,
    pub cpu_mean_percent: Option<f64>,
    #[serde(default)]
    pub cpu_median_percent: Option<f64>,
    pub cpu_min_percent: Option<f64>,
    pub cpu_peak_percent: Option<f64>,
    #[serde(default)]
    pub cpu_sample_standard_deviation_percent: Option<f64>,
    #[serde(default)]
    pub cpu_coefficient_of_variation_percent: Option<f64>,
    pub physical_footprint_mean_bytes: Option<f64>,
    #[serde(default)]
    pub physical_footprint_median_bytes: Option<f64>,
    pub physical_footprint_min_bytes: Option<u64>,
    pub physical_footprint_peak_bytes: Option<u64>,
    pub physical_footprint_final_bytes: Option<u64>,
    #[serde(default)]
    pub physical_footprint_sample_standard_deviation_bytes: Option<f64>,
    #[serde(default)]
    pub physical_footprint_coefficient_of_variation_percent: Option<f64>,
    #[serde(default)]
    pub resident_median_bytes: Option<f64>,
    pub resident_mean_bytes: Option<f64>,
    pub resident_min_bytes: Option<u64>,
    pub resident_peak_bytes: Option<u64>,
    pub resident_final_bytes: Option<u64>,
    #[serde(default)]
    pub resident_sample_standard_deviation_bytes: Option<f64>,
    #[serde(default)]
    pub resident_coefficient_of_variation_percent: Option<f64>,
    pub peak_process_count: usize,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub attribution: ProcessAttributionDiagnostics,
}

/// Descriptive statistics for finite numeric observations. Standard deviation
/// uses the sample formula and is unavailable for a single observation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MetricStatistics {
    pub sample_count: usize,
    pub median: f64,
    pub mean: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub sample_standard_deviation: Option<f64>,
    pub coefficient_of_variation_percent: Option<f64>,
}

/// Summarize finite observations using the statistics shared by benchmark
/// reports. Non-finite values are omitted and never treated as zero.
pub fn summarize_metric_values(values: impl IntoIterator<Item = f64>) -> Option<MetricStatistics> {
    let mut values = values
        .into_iter()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let sample_count = values.len();
    let mean = values.iter().sum::<f64>() / sample_count as f64;
    let middle = sample_count / 2;
    let median = if sample_count.is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    };
    let sample_standard_deviation = (sample_count > 1).then(|| {
        let squared_deviations = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>();
        (squared_deviations / (sample_count - 1) as f64).sqrt()
    });
    let coefficient_of_variation_percent = sample_standard_deviation
        .filter(|_| mean != 0.0)
        .map(|deviation| deviation / mean.abs() * 100.0);
    Some(MetricStatistics {
        sample_count,
        median,
        mean,
        minimum: values[0],
        maximum: values[sample_count - 1],
        sample_standard_deviation,
        coefficient_of_variation_percent,
    })
}

/// Collected samples plus their reusable statistical summary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceReport {
    pub schema_version: u32,
    pub started_at_unix_ms: u128,
    pub sampling_interval_ms: u64,
    pub samples: Vec<ResourceSample>,
    pub statistics: ResourceStatistics,
}

/// Synchronous resource sampler around a platform collector. It makes no
/// background thread and does not own or signal the browser process.
pub struct ResourceMonitor<C> {
    target: ManagedProcessTarget,
    collector: C,
    started_at: Instant,
    started_at_unix_ms: u128,
    previous_at: Instant,
    previous: BTreeMap<ProcessKey, ProcessResource>,
    config: ResourceConfig,
    samples: Vec<ResourceSample>,
    warnings: BTreeSet<String>,
    failed_samples: usize,
}

impl<C: ResourceCollector> ResourceMonitor<C> {
    pub fn start(
        target: ManagedProcessTarget,
        config: ResourceConfig,
        mut collector: C,
    ) -> Result<Self, ResourceMonitorError> {
        let config = config.validate()?;
        let initial = collector
            .collect(&target)
            .map_err(ResourceMonitorError::Collection)?;
        if !initial.processes.iter().any(|process| {
            process.pid == target.root_pid && process.start_time == target.root_start_time
        }) {
            return Err(ResourceMonitorError::RootNotVerified);
        }
        let started_at = Instant::now();
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let previous = index_processes(initial.processes);
        let warnings = initial.warnings.into_iter().collect();
        Ok(Self {
            target,
            collector,
            started_at,
            started_at_unix_ms,
            previous_at: started_at,
            previous,
            config,
            samples: Vec::new(),
            warnings,
            failed_samples: 0,
        })
    }

    pub fn sample(&mut self) -> &ResourceSample {
        let collected = self.collector.collect(&self.target);
        let collected_at = Instant::now();
        let elapsed_ms = collected_at.duration_since(self.started_at).as_secs_f64() * 1_000.0;
        let interval = collected_at.duration_since(self.previous_at);
        let wall_clock_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_millis());

        let sample = match collected {
            Ok(snapshot) => {
                let current = index_processes(snapshot.processes);
                let mut warnings = snapshot.warnings;
                let mut attribution = snapshot.attribution;
                if attribution.verified_processes == 0 {
                    attribution.verified_processes = current.len();
                }
                if attribution.warnings.is_empty() {
                    attribution.warnings = warnings.clone();
                }
                let (cpu_percent, processes) =
                    derive_cpu_sample(&self.previous, &current, interval, &mut warnings);
                let physical_footprint_bytes =
                    sum_memory(&current, |process| process.physical_footprint_bytes);
                let resident_bytes = sum_memory(&current, |process| process.resident_bytes);
                if physical_footprint_bytes.is_none() && !current.is_empty() {
                    warnings.push(
                        "physical footprint is unavailable for one or more verified processes"
                            .to_owned(),
                    );
                }
                if resident_bytes.is_none() && !current.is_empty() {
                    warnings.push(
                        "resident memory is unavailable for one or more verified processes"
                            .to_owned(),
                    );
                }
                self.previous = current;
                self.previous_at = collected_at;
                let coverage = if warnings.is_empty() {
                    ResourceCoverage::SessionScoped
                } else {
                    ResourceCoverage::Partial
                };
                for warning in &warnings {
                    self.warnings.insert(warning.clone());
                }
                ResourceSample {
                    elapsed_ms,
                    interval_ms: Some(interval.as_secs_f64() * 1_000.0),
                    wall_clock_unix_ms,
                    cpu_percent,
                    physical_footprint_bytes,
                    resident_bytes,
                    process_count: processes.len().max(self.previous.len()),
                    coverage,
                    attribution,
                    processes,
                    warnings,
                    collection_error: None,
                }
            }
            Err(error) => {
                self.failed_samples += 1;
                let message = error.to_string();
                self.warnings.insert(message.clone());
                ResourceSample {
                    elapsed_ms,
                    interval_ms: Some(interval.as_secs_f64() * 1_000.0),
                    wall_clock_unix_ms,
                    cpu_percent: None,
                    physical_footprint_bytes: None,
                    resident_bytes: None,
                    process_count: 0,
                    coverage: ResourceCoverage::Unavailable,
                    attribution: ProcessAttributionDiagnostics {
                        uncertain_processes: 1,
                        warnings: vec![message.clone()],
                        ..ProcessAttributionDiagnostics::default()
                    },
                    processes: Vec::new(),
                    warnings: vec![message.clone()],
                    collection_error: Some(error),
                }
            }
        };
        self.samples.push(sample);
        self.samples.last().expect("sample was just appended")
    }

    pub fn config(&self) -> ResourceConfig {
        self.config
    }

    /// Provides controlled access to the platform collector for refreshing
    /// optional browser process-registration snapshots between samples.
    pub fn collector_mut(&mut self) -> &mut C {
        &mut self.collector
    }

    pub fn stop(self) -> ResourceReport {
        let monitoring_duration_ms = self.started_at.elapsed().as_secs_f64() * 1_000.0;
        let statistics = summarize_resource_samples(
            &self.samples,
            self.failed_samples,
            monitoring_duration_ms,
            self.warnings.into_iter().collect(),
        );
        ResourceReport {
            schema_version: 3,
            started_at_unix_ms: self.started_at_unix_ms,
            sampling_interval_ms: self.config.sampling_interval_ms,
            samples: self.samples,
            statistics,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceMonitorError {
    InvalidConfiguration(String),
    Collection(ResourceCollectionError),
    RootNotVerified,
}

impl fmt::Display for ResourceMonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid monitor configuration: {message}")
            }
            Self::Collection(error) => {
                write!(formatter, "cannot start resource monitoring: {error}")
            }
            Self::RootNotVerified => {
                formatter.write_str("resource collector did not verify the managed browser process")
            }
        }
    }
}

impl std::error::Error for ResourceMonitorError {}

fn index_processes(processes: Vec<ProcessResource>) -> BTreeMap<ProcessKey, ProcessResource> {
    processes
        .into_iter()
        .map(|process| (process.key(), process))
        .collect()
}

fn derive_cpu_sample(
    previous: &BTreeMap<ProcessKey, ProcessResource>,
    current: &BTreeMap<ProcessKey, ProcessResource>,
    interval: Duration,
    warnings: &mut Vec<String>,
) -> (Option<f64>, Vec<ProcessResourceSample>) {
    let previous_keys = previous.keys().copied().collect::<BTreeSet<_>>();
    let current_keys = current.keys().copied().collect::<BTreeSet<_>>();
    if interval.is_zero() {
        warnings.push("CPU unavailable for a zero-duration sampling interval".to_owned());
        return (None, memory_samples(current));
    }
    if previous_keys != current_keys {
        warnings.push(format!(
            "CPU aggregate unavailable because verified process membership changed ({} appeared, {} disappeared)",
            current_keys.difference(&previous_keys).count(),
            previous_keys.difference(&current_keys).count()
        ));
        return (None, memory_samples(current));
    }
    let mut total_delta_ns = 0_u128;
    let mut samples = Vec::with_capacity(current.len());
    for (key, process) in current {
        let cpu_percent = match (
            previous.get(key).and_then(|prior| prior.cpu_time_ns),
            process.cpu_time_ns,
        ) {
            (Some(prior), Some(current_time)) if current_time >= prior => {
                let delta = current_time - prior;
                total_delta_ns = total_delta_ns.saturating_add(u128::from(delta));
                Some(delta as f64 / interval.as_secs_f64() / 10_000_000.0)
            }
            (Some(_), Some(_)) => {
                warnings.push(format!(
                    "CPU counter reset for PID {}; interval omitted",
                    process.pid
                ));
                return (None, memory_samples(current));
            }
            _ => {
                warnings.push(format!("CPU counter unavailable for PID {}", process.pid));
                return (None, memory_samples(current));
            }
        };
        samples.push(ProcessResourceSample {
            pid: process.pid,
            start_time: process.start_time,
            parent_pid: process.parent_pid,
            parent_start_time: process.parent_start_time,
            executable_path: process.executable_path.clone(),
            process_group_id: process.process_group_id,
            process_session_id: process.process_session_id,
            attribution_evidence: process.attribution_evidence,
            role: process.role.clone(),
            cpu_percent,
            physical_footprint_bytes: process.physical_footprint_bytes,
            resident_bytes: process.resident_bytes,
        });
    }
    let aggregate = total_delta_ns as f64 / interval.as_secs_f64() / 10_000_000.0;
    (Some(aggregate), samples)
}

fn memory_samples(current: &BTreeMap<ProcessKey, ProcessResource>) -> Vec<ProcessResourceSample> {
    current
        .values()
        .map(|process| ProcessResourceSample {
            pid: process.pid,
            start_time: process.start_time,
            parent_pid: process.parent_pid,
            parent_start_time: process.parent_start_time,
            executable_path: process.executable_path.clone(),
            process_group_id: process.process_group_id,
            process_session_id: process.process_session_id,
            attribution_evidence: process.attribution_evidence,
            role: process.role.clone(),
            cpu_percent: None,
            physical_footprint_bytes: process.physical_footprint_bytes,
            resident_bytes: process.resident_bytes,
        })
        .collect()
}

fn sum_memory(
    processes: &BTreeMap<ProcessKey, ProcessResource>,
    value: impl Fn(&ProcessResource) -> Option<u64>,
) -> Option<u64> {
    if processes.is_empty() {
        return None;
    }
    processes
        .values()
        .try_fold(0_u64, |total, process| total.checked_add(value(process)?))
}

/// Summarize collected resource samples with missing values omitted.
pub fn summarize_resource_samples(
    samples: &[ResourceSample],
    failed_samples: usize,
    monitoring_duration_ms: f64,
    warnings: Vec<String>,
) -> ResourceStatistics {
    let cpu = samples
        .iter()
        .filter_map(|sample| sample.cpu_percent)
        .collect::<Vec<_>>();
    let footprint = samples
        .iter()
        .filter_map(|sample| sample.physical_footprint_bytes)
        .collect::<Vec<_>>();
    let resident = samples
        .iter()
        .filter_map(|sample| sample.resident_bytes)
        .collect::<Vec<_>>();
    let cpu_statistics = summarize_metric_values(cpu.iter().copied());
    let footprint_statistics = summarize_metric_values(footprint.iter().map(|value| *value as f64));
    let resident_statistics = summarize_metric_values(resident.iter().map(|value| *value as f64));
    let valid_samples = samples
        .iter()
        .filter(|sample| {
            sample.cpu_percent.is_some()
                || sample.physical_footprint_bytes.is_some()
                || sample.resident_bytes.is_some()
        })
        .count();
    ResourceStatistics {
        attempted_samples: samples.len(),
        valid_samples,
        failed_samples,
        cpu_samples: cpu.len(),
        physical_footprint_samples: footprint.len(),
        resident_samples: resident.len(),
        monitoring_duration_ms,
        cpu_mean_percent: weighted_cpu_mean(samples)
            .or_else(|| cpu_statistics.map(|stats| stats.mean)),
        cpu_median_percent: cpu_statistics.map(|stats| stats.median),
        cpu_min_percent: cpu.iter().copied().reduce(f64::min),
        cpu_peak_percent: cpu.iter().copied().reduce(f64::max),
        cpu_sample_standard_deviation_percent: cpu_statistics
            .and_then(|stats| stats.sample_standard_deviation),
        cpu_coefficient_of_variation_percent: cpu_statistics
            .and_then(|stats| stats.coefficient_of_variation_percent),
        physical_footprint_mean_bytes: footprint_statistics.map(|stats| stats.mean),
        physical_footprint_median_bytes: footprint_statistics.map(|stats| stats.median),
        physical_footprint_min_bytes: footprint.iter().copied().min(),
        physical_footprint_peak_bytes: footprint.iter().copied().max(),
        physical_footprint_final_bytes: footprint.last().copied(),
        physical_footprint_sample_standard_deviation_bytes: footprint_statistics
            .and_then(|stats| stats.sample_standard_deviation),
        physical_footprint_coefficient_of_variation_percent: footprint_statistics
            .and_then(|stats| stats.coefficient_of_variation_percent),
        resident_median_bytes: resident_statistics.map(|stats| stats.median),
        resident_mean_bytes: resident_statistics.map(|stats| stats.mean),
        resident_min_bytes: resident.iter().copied().min(),
        resident_peak_bytes: resident.iter().copied().max(),
        resident_final_bytes: resident.last().copied(),
        resident_sample_standard_deviation_bytes: resident_statistics
            .and_then(|stats| stats.sample_standard_deviation),
        resident_coefficient_of_variation_percent: resident_statistics
            .and_then(|stats| stats.coefficient_of_variation_percent),
        peak_process_count: samples
            .iter()
            .map(|sample| sample.process_count)
            .max()
            .unwrap_or(0),
        warnings,
        attribution: summarize_attribution(samples),
    }
}

fn weighted_cpu_mean(samples: &[ResourceSample]) -> Option<f64> {
    let (weighted_sum, total_weight) = samples.iter().fold((0.0, 0.0), |(sum, weight), sample| {
        let Some(cpu) = sample.cpu_percent.filter(|value| value.is_finite()) else {
            return (sum, weight);
        };
        let Some(interval) = sample
            .interval_ms
            .filter(|value| value.is_finite() && *value > 0.0)
        else {
            return (sum, weight);
        };
        (sum + cpu * interval, weight + interval)
    });
    (total_weight > 0.0).then_some(weighted_sum / total_weight)
}

fn summarize_attribution(samples: &[ResourceSample]) -> ProcessAttributionDiagnostics {
    let mut summary = ProcessAttributionDiagnostics::default();
    let mut warnings = BTreeSet::new();
    let mut registry_scopes = BTreeSet::new();
    let mut registry_sources = BTreeSet::new();
    let mut lifecycle_events = BTreeSet::new();
    let mut all_complete = !samples.is_empty();
    let mut all_registry_snapshots_complete = !samples.is_empty();
    let mut registry_snapshot_count = 0;
    let mut any_partial = false;
    let mut any_unsupported = false;
    for sample in samples {
        let diagnostics = &sample.attribution;
        summary.verified_processes = summary
            .verified_processes
            .max(diagnostics.verified_processes.max(sample.process_count));
        summary.excluded_process_candidates += diagnostics.excluded_process_candidates;
        summary.uncertain_processes += diagnostics.uncertain_processes;
        summary.exited_processes += diagnostics.exited_processes;
        summary.identity_verification_failures += diagnostics.identity_verification_failures;
        summary.registered_processes = summary
            .registered_processes
            .max(diagnostics.registered_processes);
        summary.independently_verified_registered_processes = summary
            .independently_verified_registered_processes
            .max(diagnostics.independently_verified_registered_processes);
        if let Some(scope) = &diagnostics.registry_scope {
            registry_scopes.insert(scope.clone());
        }
        if let Some(source) = &diagnostics.registry_source {
            registry_sources.insert(source.clone());
        }
        if diagnostics.registry_source.is_some() {
            registry_snapshot_count += 1;
            all_registry_snapshots_complete &= diagnostics.registry_snapshot_complete;
        } else {
            all_registry_snapshots_complete = false;
        }
        lifecycle_events.extend(diagnostics.lifecycle_events.iter().cloned());
        for (evidence, count) in &diagnostics.evidence {
            *summary.evidence.entry(*evidence).or_default() += count;
        }
        for (reason, count) in &diagnostics.exclusion_reasons {
            *summary.exclusion_reasons.entry(*reason).or_default() += count;
        }
        warnings.extend(diagnostics.warnings.iter().cloned());
        match diagnostics.status {
            AttributionStatus::VerifiedComplete => {}
            AttributionStatus::VerifiedPartial => {
                all_complete = false;
                any_partial = true;
            }
            AttributionStatus::Unknown => all_complete = false,
            AttributionStatus::Unsupported => {
                all_complete = false;
                any_unsupported = true;
            }
        }
    }
    summary.status = if all_complete {
        AttributionStatus::VerifiedComplete
    } else if any_unsupported {
        AttributionStatus::Unsupported
    } else if any_partial {
        AttributionStatus::VerifiedPartial
    } else {
        AttributionStatus::Unknown
    };
    summary.warnings = warnings.into_iter().collect();
    summary.registry_scope = (registry_scopes.len() == 1)
        .then(|| registry_scopes.into_iter().next())
        .flatten();
    summary.registry_source = (registry_sources.len() == 1)
        .then(|| registry_sources.into_iter().next())
        .flatten();
    summary.registry_snapshot_complete =
        registry_snapshot_count == samples.len() && all_registry_snapshots_complete;
    summary.lifecycle_events = lifecycle_events.into_iter().collect();
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, path::PathBuf};

    struct MockCollector {
        snapshots: VecDeque<Result<ProcessResourceSnapshot, ResourceCollectionError>>,
    }

    impl ResourceCollector for MockCollector {
        fn collect(
            &mut self,
            _: &ManagedProcessTarget,
        ) -> Result<ProcessResourceSnapshot, ResourceCollectionError> {
            self.snapshots
                .pop_front()
                .unwrap_or_else(|| Ok(snapshot(vec![process(10, 1_000, 100, 90)])))
        }
    }

    fn target() -> ManagedProcessTarget {
        ManagedProcessTarget {
            root_pid: 10,
            session_id: 10,
            root_start_time: ProcessStartTime {
                seconds: 1,
                microseconds: 10,
            },
            executable_path: PathBuf::from("/fixture/browser"),
            application_path: PathBuf::from("/fixture/browser.app"),
            isolated_profile_path: None,
            allow_profile_argument_attribution: false,
        }
    }

    fn process(pid: u32, cpu: u64, footprint: u64, resident: u64) -> ProcessResource {
        ProcessResource {
            pid,
            start_time: ProcessStartTime {
                seconds: 1,
                microseconds: u64::from(pid),
            },
            parent_pid: (pid != 10).then_some(10),
            parent_start_time: (pid != 10).then_some(ProcessStartTime {
                seconds: 1,
                microseconds: 10,
            }),
            executable_path: Some(PathBuf::from("/fixture/browser")),
            process_group_id: Some(10),
            process_session_id: Some(10),
            attribution_evidence: Some(if pid == 10 {
                AttributionEvidence::ManagedRootIdentity
            } else {
                AttributionEvidence::AncestryAndPrivateSession
            }),
            role: None,
            cpu_time_ns: Some(cpu),
            physical_footprint_bytes: Some(footprint),
            resident_bytes: Some(resident),
        }
    }

    fn snapshot(processes: Vec<ProcessResource>) -> ProcessResourceSnapshot {
        ProcessResourceSnapshot {
            processes,
            warnings: Vec::new(),
            attribution: ProcessAttributionDiagnostics::default(),
        }
    }

    fn monitor(
        snapshots: Vec<Result<ProcessResourceSnapshot, ResourceCollectionError>>,
    ) -> ResourceMonitor<MockCollector> {
        ResourceMonitor::start(
            target(),
            ResourceConfig::default(),
            MockCollector {
                snapshots: std::iter::once(Ok(snapshot(vec![process(10, 0, 100, 90)])))
                    .chain(snapshots)
                    .collect(),
            },
        )
        .unwrap()
    }

    #[test]
    fn default_interval_is_one_hundred_milliseconds_and_zero_is_invalid() {
        assert_eq!(ResourceConfig::default().sampling_interval_ms, 100);
        assert!(
            ResourceConfig {
                sampling_interval_ms: 0
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn start_requires_verified_root_identity() {
        let result = ResourceMonitor::start(
            target(),
            ResourceConfig::default(),
            MockCollector {
                snapshots: VecDeque::from([Ok(snapshot(vec![process(11, 0, 1, 1)]))]),
            },
        );
        let error = match result {
            Ok(_) => panic!("expected missing root to fail"),
            Err(error) => error,
        };
        assert_eq!(error, ResourceMonitorError::RootNotVerified);
    }

    #[test]
    fn memory_sums_verified_processes_and_cpu_uses_cumulative_time_delta() {
        let mut monitor = monitor(vec![Ok(snapshot(vec![process(
            10,
            1_000_000_000,
            100,
            80,
        )]))]);
        std::thread::sleep(Duration::from_millis(20));
        let sample = monitor.sample().clone();
        assert!(sample.cpu_percent.unwrap() > 0.0);
        assert_eq!(sample.physical_footprint_bytes, Some(100));
        assert_eq!(sample.resident_bytes, Some(80));
    }

    #[test]
    fn multicore_cpu_is_not_clamped_to_one_hundred_percent() {
        let mut monitor = monitor(vec![Ok(snapshot(vec![
            process(10, 1_000_000_000, 100, 80),
            process(11, 1_000_000_000, 100, 80),
        ]))]);
        // Make the first snapshot contain both processes too.
        monitor.previous = index_processes(vec![process(10, 0, 100, 80), process(11, 0, 100, 80)]);
        let sample = monitor.sample().clone();
        assert_eq!(sample.physical_footprint_bytes, Some(200));
        assert_eq!(sample.resident_bytes, Some(160));
        assert_eq!(sample.process_count, 2);
        assert!(sample.cpu_percent.unwrap() > 100.0);
    }

    #[test]
    fn counter_reset_and_process_set_changes_make_cpu_unavailable() {
        let mut reset = monitor(vec![Ok(snapshot(vec![process(10, 10, 100, 80)]))]);
        reset.previous = index_processes(vec![process(10, 20, 100, 80)]);
        assert!(reset.sample().cpu_percent.is_none());

        let mut changed = monitor(vec![Ok(snapshot(vec![
            process(10, 10, 100, 80),
            process(11, 10, 20, 10),
        ]))]);
        let sample = changed.sample();
        assert!(sample.cpu_percent.is_none());
        assert!(
            sample
                .warnings
                .iter()
                .any(|warning| warning.contains("membership changed"))
        );
    }

    #[test]
    fn cpu_delta_supports_multiple_cores_and_zero_duration_is_unavailable() {
        let previous = index_processes(vec![
            process(10, 1_000_000_000, 100, 80),
            process(11, 0, 100, 80),
        ]);
        let current = index_processes(vec![
            process(10, 3_000_000_000, 100, 80),
            process(11, 1_000_000_000, 100, 80),
        ]);
        let (cpu, _) =
            derive_cpu_sample(&previous, &current, Duration::from_secs(1), &mut Vec::new());
        assert_eq!(cpu, Some(300.0));

        let mut warnings = Vec::new();
        let (cpu, _) = derive_cpu_sample(&previous, &current, Duration::ZERO, &mut warnings);
        assert_eq!(cpu, None);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("zero-duration"))
        );
    }

    #[test]
    fn process_disappearance_and_pid_reuse_make_cpu_unavailable() {
        let previous = index_processes(vec![process(10, 0, 100, 80), process(11, 0, 50, 40)]);
        let current = index_processes(vec![process(10, 1_000_000_000, 100, 80)]);
        let mut warnings = Vec::new();
        let (cpu, _) =
            derive_cpu_sample(&previous, &current, Duration::from_secs(1), &mut warnings);
        assert_eq!(cpu, None);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("1 disappeared"));

        let previous = index_processes(vec![process(10, 0, 100, 80)]);
        let mut reused = process(10, 1_000_000_000, 100, 80);
        reused.start_time.seconds += 1;
        let current = index_processes(vec![reused]);
        let mut warnings = Vec::new();
        let (cpu, _) =
            derive_cpu_sample(&previous, &current, Duration::from_secs(1), &mut warnings);
        assert_eq!(cpu, None);
        assert!(warnings[0].contains("1 appeared"));
        assert!(warnings[0].contains("1 disappeared"));
    }

    #[test]
    fn failed_collection_keeps_a_failed_sample_and_recovers() {
        let mut monitor = monitor(vec![
            Err(ResourceCollectionError::new("root exited")),
            Ok(snapshot(vec![process(10, 100, 100, 80)])),
        ]);
        assert!(monitor.sample().collection_error.is_some());
        assert!(monitor.sample().collection_error.is_none());
        let report = monitor.stop();
        assert_eq!(report.statistics.attempted_samples, 2);
        assert_eq!(report.statistics.failed_samples, 1);
    }

    #[test]
    fn missing_memory_is_not_serialized_as_zero() {
        let mut missing = process(10, 1, 2, 3);
        missing.physical_footprint_bytes = None;
        let mut monitor = monitor(vec![Ok(snapshot(vec![missing]))]);
        let sample = monitor.sample().clone();
        assert_eq!(sample.physical_footprint_bytes, None);
        let json = serde_json::to_string(&sample).unwrap();
        assert!(json.contains("\"physical_footprint_bytes\":null"));
    }

    #[test]
    fn statistics_ignore_missing_samples_and_report_final_observation() {
        let samples = vec![
            ResourceSample {
                elapsed_ms: 10.0,
                interval_ms: Some(10.0),
                wall_clock_unix_ms: None,
                cpu_percent: Some(20.0),
                physical_footprint_bytes: Some(100),
                resident_bytes: Some(80),
                process_count: 2,
                coverage: ResourceCoverage::SessionScoped,
                attribution: ProcessAttributionDiagnostics::default(),
                processes: Vec::new(),
                warnings: Vec::new(),
                collection_error: None,
            },
            ResourceSample {
                elapsed_ms: 20.0,
                interval_ms: Some(10.0),
                wall_clock_unix_ms: None,
                cpu_percent: None,
                physical_footprint_bytes: Some(200),
                resident_bytes: Some(150),
                process_count: 4,
                coverage: ResourceCoverage::Partial,
                attribution: ProcessAttributionDiagnostics::default(),
                processes: Vec::new(),
                warnings: Vec::new(),
                collection_error: Some(ResourceCollectionError::new("unavailable")),
            },
        ];
        let summary = summarize_resource_samples(&samples, 1, 20.0, Vec::new());
        assert_eq!(summary.cpu_mean_percent, Some(20.0));
        assert_eq!(summary.cpu_samples, 1);
        assert_eq!(summary.physical_footprint_mean_bytes, Some(150.0));
        assert_eq!(summary.physical_footprint_samples, 2);
        assert_eq!(summary.physical_footprint_final_bytes, Some(200));
        assert_eq!(summary.peak_process_count, 4);
        assert_eq!(summary.valid_samples, 2);
    }

    #[test]
    fn cpu_mean_is_weighted_by_actual_elapsed_sampling_intervals() {
        let mut first = sample_for_statistics(10.0, 100.0);
        first.elapsed_ms = 100.0;
        let mut second = sample_for_statistics(90.0, 900.0);
        second.elapsed_ms = 1_000.0;
        let summary = summarize_resource_samples(&[first, second], 0, 1_000.0, Vec::new());
        assert_eq!(summary.cpu_mean_percent, Some(82.0));
        assert_eq!(summary.cpu_median_percent, Some(50.0));
    }

    fn sample_for_statistics(cpu_percent: f64, interval_ms: f64) -> ResourceSample {
        ResourceSample {
            elapsed_ms: interval_ms,
            interval_ms: Some(interval_ms),
            wall_clock_unix_ms: None,
            cpu_percent: Some(cpu_percent),
            physical_footprint_bytes: Some(100),
            resident_bytes: Some(80),
            process_count: 1,
            coverage: ResourceCoverage::SessionScoped,
            attribution: ProcessAttributionDiagnostics::default(),
            processes: Vec::new(),
            warnings: Vec::new(),
            collection_error: None,
        }
    }

    #[test]
    fn report_serializes_structured_samples() {
        let mut monitor = monitor(Vec::new());
        let _ = monitor.sample();
        let report = monitor.stop();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["schema_version"], 3);
        assert!(value["samples"].is_array());
    }

    #[test]
    fn collection_errors_serialize_with_a_machine_readable_kind() {
        let error = ResourceCollectionError::with_kind(
            ResourceErrorKind::ProcessIdentityMismatch,
            "recorded process identity changed",
        );
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value["kind"], "process_identity_mismatch");
        assert_eq!(value["message"], "recorded process identity changed");
    }
}
