use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use velocity_metrics::{
    AttributionStatus, MetricStatistics, ProcessAttributionDiagnostics, ProcessLifecycleEvent,
    ResourceReport, summarize_metric_values,
};

use crate::{ReportBrowser, RunOrderMetadata};

pub const RESOURCE_BENCHMARK_SCHEMA_VERSION: u32 = 3;
pub const RESOURCE_MEASUREMENT_METHODOLOGY_VERSION: &str =
    "engine-registry-and-private-session-time-weighted-cpu-v3";
pub const FIXTURE_VERSION: &str = "1";
pub const MINIMUM_COMPARABLE_ITERATIONS: usize = 3;
pub const MAX_TAB_COUNT: usize = 50;
pub const MAX_TAB_SCENARIOS: usize = 10;
pub const MAX_RETAINED_RESOURCE_SAMPLES: usize = 100_000;
pub const MAX_DYNAMIC_ELEMENTS_PER_WORKLOAD: usize = 100_000;
pub const MINIMUM_SAMPLE_COVERAGE_PERCENT: f64 = 90.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComparabilityStatus {
    Comparable,
    #[default]
    Inconclusive,
    Incompatible,
}

impl ComparabilityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Comparable => "COMPARABLE",
            Self::Inconclusive => "INCONCLUSIVE",
            Self::Incompatible => "INCOMPATIBLE",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResourceSamplingCoverage {
    pub attempted_samples: usize,
    pub valid_samples: usize,
    pub failed_samples: usize,
    pub cpu_samples: usize,
    pub physical_footprint_samples: usize,
    pub resident_samples: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ResourceComparisonAssessment {
    pub tab_count: Option<usize>,
    pub status: ComparabilityStatus,
    pub browser_count: usize,
    pub reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceWorkload {
    Idle,
    Page,
    Tabs,
}

impl ResourceWorkload {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Page => "page",
            Self::Tabs => "tabs",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    Simple,
    Complex,
    Dynamic,
    TaskBacklog,
    GcPressure,
    DomStyle,
    StyleLocalized,
    StyleDescendant,
    StyleSibling,
    StyleRelational,
    LayoutUpdates,
    Animation,
}

impl FixtureKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Complex => "complex",
            Self::Dynamic => "dynamic",
            Self::TaskBacklog => "task-backlog",
            Self::GcPressure => "gc-pressure",
            Self::DomStyle => "dom-style",
            Self::StyleLocalized => "style-localized",
            Self::StyleDescendant => "style-descendant",
            Self::StyleSibling => "style-sibling",
            Self::StyleRelational => "style-relational",
            Self::LayoutUpdates => "layout-updates",
            Self::Animation => "animation",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceBenchmarkConfig {
    pub runs: usize,
    pub warmup_iterations: usize,
    pub randomized: bool,
    pub measurement_duration_ms: u64,
    pub settle_period_ms: u64,
    pub sampling_interval_ms: u64,
    pub navigation_timeout_ms: u64,
    pub readiness_timeout_ms: u64,
    pub fixture: Option<FixtureKind>,
    pub tab_counts: Vec<usize>,
    pub dynamic_intensity: u32,
    pub dynamic_update_interval_ms: u64,
}

impl ResourceBenchmarkConfig {
    pub fn defaults(workload: ResourceWorkload) -> Self {
        Self {
            runs: 5,
            warmup_iterations: 0,
            randomized: false,
            measurement_duration_ms: 10_000,
            settle_period_ms: 5_000,
            sampling_interval_ms: 100,
            navigation_timeout_ms: 30_000,
            readiness_timeout_ms: 30_000,
            fixture: match workload {
                ResourceWorkload::Idle => None,
                ResourceWorkload::Page | ResourceWorkload::Tabs => Some(FixtureKind::Simple),
            },
            tab_counts: if workload == ResourceWorkload::Tabs {
                vec![1, 5, 10]
            } else {
                Vec::new()
            },
            dynamic_intensity: 25,
            dynamic_update_interval_ms: 100,
        }
    }

    pub fn validate(&self, workload: ResourceWorkload) -> Result<(), String> {
        if NonZeroUsize::new(self.runs).is_none() || self.runs > 100 {
            return Err("--runs must be between 1 and 100".to_owned());
        }
        if self.warmup_iterations > 20 {
            return Err("--warmup must be between 0 and 20".to_owned());
        }
        if !(1_000..=3_600_000).contains(&self.measurement_duration_ms) {
            return Err("--duration must be between 1 and 3600 seconds".to_owned());
        }
        if self.settle_period_ms > 600_000 {
            return Err("--settle must not exceed 600 seconds".to_owned());
        }
        if !(10..=60_000).contains(&self.sampling_interval_ms) {
            return Err("--interval must be between 10 and 60000 milliseconds".to_owned());
        }
        if self.navigation_timeout_ms == 0 || self.navigation_timeout_ms > 600_000 {
            return Err("navigation timeout must be between 1 ms and 600 seconds".to_owned());
        }
        if self.readiness_timeout_ms == 0 || self.readiness_timeout_ms > 600_000 {
            return Err("readiness timeout must be between 1 ms and 600 seconds".to_owned());
        }
        let sample_count = self
            .measurement_duration_ms
            .div_ceil(self.sampling_interval_ms);
        if sample_count > 100_000 {
            return Err(
                "configuration would retain more than 100000 samples per iteration".to_owned(),
            );
        }
        if !(1..=10_000).contains(&self.dynamic_intensity) {
            return Err("--intensity must be between 1 and 10000".to_owned());
        }
        if !(10..=60_000).contains(&self.dynamic_update_interval_ms) {
            return Err("--update-interval must be between 10 and 60000 milliseconds".to_owned());
        }
        match workload {
            ResourceWorkload::Idle => {
                if self.fixture.is_some() || !self.tab_counts.is_empty() {
                    return Err("idle workloads do not accept --fixture or --tabs".to_owned());
                }
            }
            ResourceWorkload::Page => {
                if self.fixture.is_none() {
                    return Err("page workloads require a fixture".to_owned());
                }
                if !self.tab_counts.is_empty() {
                    return Err("page workloads do not accept --tabs".to_owned());
                }
            }
            ResourceWorkload::Tabs => {
                if self.fixture.is_none() {
                    return Err("tab workloads require a fixture".to_owned());
                }
                if self.tab_counts.is_empty() {
                    return Err("tab workloads require at least one --tabs count".to_owned());
                }
                if self.tab_counts.len() > MAX_TAB_SCENARIOS {
                    return Err(format!(
                        "--tabs accepts at most {MAX_TAB_SCENARIOS} distinct counts"
                    ));
                }
                if self
                    .tab_counts
                    .iter()
                    .any(|count| !(1..=MAX_TAB_COUNT).contains(count))
                {
                    return Err(format!(
                        "--tabs counts must be between 1 and {MAX_TAB_COUNT}"
                    ));
                }
                let unique = self.tab_counts.iter().copied().collect::<BTreeSet<_>>();
                if unique.len() != self.tab_counts.len() {
                    return Err("--tabs cannot contain duplicate counts".to_owned());
                }
                if self.fixture == Some(FixtureKind::Dynamic)
                    && self.tab_counts.iter().max().is_some_and(|count| {
                        count.saturating_mul(self.dynamic_intensity as usize)
                            > MAX_DYNAMIC_ELEMENTS_PER_WORKLOAD
                    })
                {
                    return Err(format!(
                        "dynamic tabs workload exceeds {MAX_DYNAMIC_ELEMENTS_PER_WORKLOAD} total DOM elements"
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceFixtureMetadata {
    pub kind: FixtureKind,
    pub version: String,
    pub dynamic_intensity: Option<u32>,
    pub dynamic_update_interval_ms: Option<u64>,
}

impl ResourceFixtureMetadata {
    pub fn from_config(config: &ResourceBenchmarkConfig) -> Option<Self> {
        config.fixture.map(|kind| Self {
            kind,
            version: FIXTURE_VERSION.to_owned(),
            dynamic_intensity: (kind == FixtureKind::Dynamic).then_some(config.dynamic_intensity),
            dynamic_update_interval_ms: (kind == FixtureKind::Dynamic)
                .then_some(config.dynamic_update_interval_ms),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFailureKind {
    Unsupported,
    Setup,
    Launch,
    ReadinessTimeout,
    Automation,
    Sampling,
    Cancelled,
    Cleanup,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceBenchmarkFailure {
    pub kind: ResourceFailureKind,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceBenchmarkIteration {
    pub browser: ReportBrowser,
    pub iteration: usize,
    pub warmup: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_pair_id: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics_enabled: Option<bool>,
    pub tab_count: Option<usize>,
    pub automation_protocol: Option<String>,
    pub profile_isolation: String,
    pub started_at_unix_ms: u128,
    pub resource: Option<ResourceReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsiveness: Option<ResponsivenessIterationData>,
    pub failure: Option<ResourceBenchmarkFailure>,
    pub cleanup: CleanupStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResponsivenessIterationData {
    pub workload_duration_ms: f64,
    pub metrics: serde_json::Value,
    pub process_resources: Option<velocity_metrics::ResourceReport>,
    pub process_resource_error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CleanupStatus {
    pub browser_closed: bool,
    pub profile_removed: bool,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceMetricSummary {
    pub sample_count: usize,
    pub mean: f64,
    pub median: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub sample_standard_deviation: Option<f64>,
    pub coefficient_of_variation_percent: Option<f64>,
}

impl From<MetricStatistics> for ResourceMetricSummary {
    fn from(statistics: MetricStatistics) -> Self {
        Self {
            sample_count: statistics.sample_count,
            mean: statistics.mean,
            median: statistics.median,
            minimum: statistics.minimum,
            maximum: statistics.maximum,
            sample_standard_deviation: statistics.sample_standard_deviation,
            coefficient_of_variation_percent: statistics.coefficient_of_variation_percent,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceBenchmarkSummary {
    pub browser: ReportBrowser,
    pub tab_count: Option<usize>,
    pub successful_iterations: usize,
    pub failed_iterations: usize,
    /// Statistics across per-iteration values; raw within-run samples stay in
    /// `iterations[].resource` and are summarized separately there.
    pub across_iteration_statistics: BTreeMap<String, ResourceMetricSummary>,
    pub coverage_warnings: Vec<String>,
    #[serde(default)]
    pub attribution: ProcessAttributionDiagnostics,
    #[serde(default)]
    pub sampling_coverage: ResourceSamplingCoverage,
    #[serde(default)]
    pub comparison_status: ComparabilityStatus,
    #[serde(default)]
    pub comparison_reasons: Vec<String>,
}

/// Descriptive memory measurements for one browser and tab-count scenario.
/// Incremental values are differences between scenario averages, not isolated
/// per-tab costs or a claim of linear scaling.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MemoryScalingScenario {
    pub browser: ReportBrowser,
    pub tab_count: usize,
    pub successful_iterations: usize,
    pub mean_footprint_bytes: Option<f64>,
    pub median_footprint_bytes: Option<f64>,
    pub peak_observed_footprint_bytes: Option<u64>,
    pub incremental_footprint_bytes: Option<f64>,
    pub marginal_footprint_per_added_tab_bytes: Option<f64>,
    pub measurement_status: ComparabilityStatus,
    pub measurement_reasons: Vec<String>,
    pub incremental_status: Option<ComparabilityStatus>,
    pub incremental_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceBenchmarkReport {
    pub schema_version: u32,
    #[serde(default = "legacy_measurement_methodology_version")]
    pub measurement_methodology_version: String,
    pub benchmark: String,
    pub timestamp_unix_ms: u128,
    pub operating_system: String,
    pub architecture: String,
    pub configuration: ResourceBenchmarkConfig,
    pub fixture: Option<ResourceFixtureMetadata>,
    pub run_order: RunOrderMetadata,
    pub iterations: Vec<ResourceBenchmarkIteration>,
    pub summaries: Vec<ResourceBenchmarkSummary>,
    #[serde(default)]
    pub memory_scaling: Vec<MemoryScalingScenario>,
    #[serde(default)]
    pub comparison_assessments: Vec<ResourceComparisonAssessment>,
    pub limitations: Vec<String>,
    pub cancelled: bool,
}

impl ResourceBenchmarkReport {
    pub fn new(
        workload: ResourceWorkload,
        configuration: ResourceBenchmarkConfig,
        run_order: RunOrderMetadata,
        iterations: Vec<ResourceBenchmarkIteration>,
        cancelled: bool,
    ) -> Self {
        let summaries = summarize_resource_iterations(&iterations);
        let comparison_assessments = assess_resource_comparability(&summaries);
        let memory_scaling = if workload == ResourceWorkload::Tabs {
            summarize_memory_scaling(&iterations, &summaries, &configuration.tab_counts)
        } else {
            Vec::new()
        };
        let mut limitations = vec![
            "Resource totals include only the managed session processes Velocity can verify; browser helpers outside that session are excluded.".to_owned(),
            "Resident memory is the sum of verified process RSS and may count shared pages more than once; it is not unique physical memory usage.".to_owned(),
            "Peaks are the highest sampled values and may miss activity between samples.".to_owned(),
            "Resource samples within each iteration and run-level statistics across iterations are reported separately.".to_owned(),
        ];
        if workload == ResourceWorkload::Idle {
            limitations.push(
                "Idle workload uses about:blank in a fresh isolated profile; browser background services and start-page work are not measured as an identical workload.".to_owned(),
            );
        }
        if workload == ResourceWorkload::Tabs {
            limitations.push(
                "Tab workloads are supported only by browser installations with a verified isolated-profile DevTools adapter; other installations are recorded as unsupported.".to_owned(),
            );
        }
        if comparison_assessments
            .iter()
            .any(|assessment| assessment.status != ComparabilityStatus::Comparable)
        {
            limitations.push(format!(
                "Resource comparisons require matching workloads, at least {MINIMUM_COMPARABLE_ITERATIONS} successful measured iterations, adequate sampling coverage, and verified-complete process attribution."
            ));
        }
        if memory_scaling
            .iter()
            .any(|scenario| scenario.measurement_status != ComparabilityStatus::Comparable)
        {
            limitations.push(
                "Memory scaling values with incomplete process attribution or sampling coverage are diagnostic and should not be used for efficiency rankings.".to_owned(),
            );
        }
        Self {
            schema_version: RESOURCE_BENCHMARK_SCHEMA_VERSION,
            measurement_methodology_version: RESOURCE_MEASUREMENT_METHODOLOGY_VERSION.to_owned(),
            benchmark: workload.id().to_owned(),
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            fixture: ResourceFixtureMetadata::from_config(&configuration),
            configuration,
            run_order,
            iterations,
            summaries,
            memory_scaling,
            comparison_assessments,
            limitations,
            cancelled,
        }
    }
}

fn legacy_measurement_methodology_version() -> String {
    "unspecified-legacy".to_owned()
}

pub fn summarize_resource_iterations(
    iterations: &[ResourceBenchmarkIteration],
) -> Vec<ResourceBenchmarkSummary> {
    let mut groups = BTreeMap::<(String, Option<usize>), Vec<&ResourceBenchmarkIteration>>::new();
    for iteration in iterations.iter().filter(|iteration| !iteration.warmup) {
        groups
            .entry((
                iteration.browser.installation_id.clone(),
                iteration.tab_count,
            ))
            .or_default()
            .push(iteration);
    }
    groups
        .into_values()
        .map(|iterations| {
            let first = iterations[0];
            let successful_iterations = iterations
                .iter()
                .filter(|iteration| {
                    iteration
                        .resource
                        .as_ref()
                        .is_some_and(|resource| resource.statistics.valid_samples > 0)
                        && iteration.failure.is_none()
                })
                .count();
            let failed_iterations = iterations.len() - successful_iterations;
            let mut values = BTreeMap::<&str, Vec<f64>>::new();
            let mut warnings = BTreeSet::new();
            let mut profile_strategies = BTreeSet::new();
            let mut resource_schemas = BTreeSet::new();
            let mut sample_intervals = BTreeSet::new();
            let mut sampling_coverage = ResourceSamplingCoverage::default();
            let mut attribution = ProcessAttributionDiagnostics::default();
            let mut attribution_statuses = Vec::new();
            let mut registry_completeness = Vec::new();
            let mut lifecycle_events = BTreeSet::<ProcessLifecycleEvent>::new();
            let mut registry_scopes = BTreeSet::new();
            let mut registry_sources = BTreeSet::new();
            for iteration in &iterations {
                if let Some(resource) = &iteration.resource {
                    if iteration.failure.is_none() {
                        profile_strategies.insert(iteration.profile_isolation.as_str());
                    }
                    resource_schemas.insert(resource.schema_version);
                    sample_intervals.insert(resource.sampling_interval_ms);
                    let statistics = &resource.statistics;
                    sampling_coverage.attempted_samples += statistics.attempted_samples;
                    sampling_coverage.valid_samples += statistics.valid_samples;
                    sampling_coverage.failed_samples += statistics.failed_samples;
                    sampling_coverage.cpu_samples += statistics.cpu_samples;
                    sampling_coverage.physical_footprint_samples +=
                        statistics.physical_footprint_samples;
                    sampling_coverage.resident_samples += statistics.resident_samples;
                    let details = &statistics.attribution;
                    attribution_statuses.push(details.status);
                    attribution.verified_processes = attribution
                        .verified_processes
                        .max(details.verified_processes);
                    attribution.registered_processes = attribution
                        .registered_processes
                        .max(details.registered_processes);
                    attribution.independently_verified_registered_processes = attribution
                        .independently_verified_registered_processes
                        .max(details.independently_verified_registered_processes);
                    registry_completeness.push(details.registry_snapshot_complete);
                    if let Some(scope) = &details.registry_scope {
                        registry_scopes.insert(scope.clone());
                    }
                    if let Some(source) = &details.registry_source {
                        registry_sources.insert(source.clone());
                    }
                    lifecycle_events.extend(details.lifecycle_events.iter().cloned());
                    attribution.excluded_process_candidates +=
                        details.excluded_process_candidates;
                    attribution.uncertain_processes += details.uncertain_processes;
                    attribution.exited_processes += details.exited_processes;
                    attribution.identity_verification_failures +=
                        details.identity_verification_failures;
                    for (evidence, count) in &details.evidence {
                        *attribution.evidence.entry(*evidence).or_default() += count;
                    }
                    for (reason, count) in &details.exclusion_reasons {
                        *attribution.exclusion_reasons.entry(*reason).or_default() += count;
                    }
                    warnings.extend(details.warnings.iter().cloned());
                    push_optional(&mut values, "cpu_mean_percent", statistics.cpu_mean_percent);
                    push_optional(&mut values, "cpu_peak_percent", statistics.cpu_peak_percent);
                    push_optional(
                        &mut values,
                        "physical_footprint_mean_bytes",
                        statistics.physical_footprint_mean_bytes,
                    );
                    push_optional(
                        &mut values,
                        "physical_footprint_peak_bytes",
                        statistics
                            .physical_footprint_peak_bytes
                            .map(|value| value as f64),
                    );
                    push_optional(
                        &mut values,
                        "physical_footprint_final_bytes",
                        statistics
                            .physical_footprint_final_bytes
                            .map(|value| value as f64),
                    );
                    push_optional(
                        &mut values,
                        "resident_mean_bytes",
                        statistics.resident_mean_bytes,
                    );
                    push_optional(
                        &mut values,
                        "resident_peak_bytes",
                        statistics.resident_peak_bytes.map(|value| value as f64),
                    );
                    if statistics.valid_samples > 0 {
                        values
                            .entry("peak_process_count")
                            .or_default()
                            .push(statistics.peak_process_count as f64);
                    }
                    warnings.extend(statistics.warnings.iter().cloned());
                    for sample in &resource.samples {
                        warnings.extend(sample.warnings.iter().cloned());
                    }
                }
            }
            attribution.status = combine_attribution_statuses(&attribution_statuses);
            attribution.registry_scope = (registry_scopes.len() == 1)
                .then(|| registry_scopes.into_iter().next())
                .flatten();
            attribution.registry_source = (registry_sources.len() == 1)
                .then(|| registry_sources.into_iter().next())
                .flatten();
            attribution.registry_snapshot_complete = !registry_completeness.is_empty()
                && registry_completeness.iter().all(|complete| *complete);
            attribution.lifecycle_events = lifecycle_events.into_iter().collect();
            attribution.warnings = warnings.iter().cloned().collect();
            let across_iteration_statistics = values
                .into_iter()
                .filter_map(|(name, values)| {
                    summarize_metric_values(values)
                        .map(|statistics| (name.to_owned(), statistics.into()))
                })
                .collect::<BTreeMap<_, _>>();
            let mut comparison_reasons = Vec::new();
            let incompatible = profile_strategies.len() > 1
                || resource_schemas.len() > 1
                || sample_intervals.len() > 1;
            if profile_strategies.len() > 1 {
                comparison_reasons.push(
                    "successful iterations used different profile isolation strategies".to_owned(),
                );
            }
            if resource_schemas.len() > 1 || sample_intervals.len() > 1 {
                comparison_reasons.push(
                    "iterations used incompatible resource schema or sampling intervals".to_owned(),
                );
            }
            if successful_iterations < MINIMUM_COMPARABLE_ITERATIONS {
                comparison_reasons.push(format!(
                    "{} successful measured iterations; at least {MINIMUM_COMPARABLE_ITERATIONS} are required",
                    successful_iterations
                ));
            }
            if failed_iterations > 0 {
                comparison_reasons.push(format!("{failed_iterations} measured iterations failed"));
            }
            if sampling_coverage.attempted_samples == 0 {
                comparison_reasons.push("no resource samples were collected".to_owned());
            } else {
                if sampling_coverage.failed_samples > 0 {
                    comparison_reasons.push(format!(
                        "{} scheduled samples failed collection",
                        sampling_coverage.failed_samples
                    ));
                }
                for (metric, count) in [
                    ("CPU", sampling_coverage.cpu_samples),
                    (
                        "physical footprint",
                        sampling_coverage.physical_footprint_samples,
                    ),
                    ("resident memory", sampling_coverage.resident_samples),
                ] {
                    let coverage = count as f64 * 100.0
                        / sampling_coverage.attempted_samples as f64;
                    if coverage < MINIMUM_SAMPLE_COVERAGE_PERCENT {
                        comparison_reasons.push(format!(
                            "{metric} sample coverage is {coverage:.1}%; {MINIMUM_SAMPLE_COVERAGE_PERCENT:.0}% is required"
                        ));
                    }
                }
            }
            if attribution.status != AttributionStatus::VerifiedComplete {
                comparison_reasons.push(format!(
                    "process attribution is {}; complete browser-process coverage is required",
                    attribution.status.label()
                ));
            }
            comparison_reasons.sort();
            comparison_reasons.dedup();
            let comparison_status = if incompatible {
                ComparabilityStatus::Incompatible
            } else if comparison_reasons.is_empty() {
                ComparabilityStatus::Comparable
            } else {
                ComparabilityStatus::Inconclusive
            };
            ResourceBenchmarkSummary {
                browser: first.browser.clone(),
                tab_count: first.tab_count,
                successful_iterations,
                failed_iterations,
                across_iteration_statistics,
                coverage_warnings: warnings.into_iter().collect(),
                attribution,
                sampling_coverage,
                comparison_status,
                comparison_reasons,
            }
        })
        .collect()
}

fn combine_attribution_statuses(statuses: &[AttributionStatus]) -> AttributionStatus {
    if statuses.is_empty() {
        return AttributionStatus::Unknown;
    }
    if statuses.contains(&AttributionStatus::Unsupported) {
        AttributionStatus::Unsupported
    } else if statuses.contains(&AttributionStatus::VerifiedPartial) {
        AttributionStatus::VerifiedPartial
    } else if statuses
        .iter()
        .all(|status| *status == AttributionStatus::VerifiedComplete)
    {
        AttributionStatus::VerifiedComplete
    } else {
        AttributionStatus::Unknown
    }
}

fn assess_resource_comparability(
    summaries: &[ResourceBenchmarkSummary],
) -> Vec<ResourceComparisonAssessment> {
    let mut groups = BTreeMap::<Option<usize>, Vec<&ResourceBenchmarkSummary>>::new();
    for summary in summaries {
        groups.entry(summary.tab_count).or_default().push(summary);
    }
    groups
        .into_iter()
        .map(|(tab_count, summaries)| {
            let mut reasons = Vec::new();
            let mut attribution_scopes = BTreeSet::new();
            let mut attribution_scope_missing = false;
            if summaries.len() < 2 {
                reasons.push(
                    "fewer than two browser installations produced this workload scenario"
                        .to_owned(),
                );
            }
            for summary in &summaries {
                if summary.attribution.status != AttributionStatus::VerifiedComplete
                    || !summary.attribution.registry_snapshot_complete
                {
                    attribution_scope_missing = true;
                }
                match (
                    summary.attribution.registry_source.as_ref(),
                    summary.attribution.registry_scope.as_ref(),
                ) {
                    (Some(source), Some(scope)) => {
                        attribution_scopes.insert((source.clone(), scope.clone()));
                    }
                    _ => attribution_scope_missing = true,
                }
                if summary.comparison_status == ComparabilityStatus::Incompatible {
                    reasons.extend(
                        summary
                            .comparison_reasons
                            .iter()
                            .map(|reason| format!("{}: {reason}", summary.browser.name)),
                    );
                } else if summary.comparison_status == ComparabilityStatus::Inconclusive {
                    reasons.extend(
                        summary
                            .comparison_reasons
                            .iter()
                            .map(|reason| format!("{}: {reason}", summary.browser.name)),
                    );
                }
            }
            if attribution_scope_missing {
                reasons.push(
                    "process attribution scope or completeness evidence is missing for at least one browser".to_owned(),
                );
            }
            if attribution_scopes.len() > 1 {
                reasons.push(
                    "process attribution scopes differ between browsers; resource totals are not directly comparable".to_owned(),
                );
            }
            reasons.sort();
            reasons.dedup();
            let status = if summaries
                .iter()
                .any(|summary| summary.comparison_status == ComparabilityStatus::Incompatible)
            {
                ComparabilityStatus::Incompatible
            } else if summaries.len() < 2
                || summaries
                    .iter()
                    .any(|summary| summary.comparison_status != ComparabilityStatus::Comparable)
                || attribution_scope_missing
                || attribution_scopes.len() > 1
            {
                ComparabilityStatus::Inconclusive
            } else {
                ComparabilityStatus::Comparable
            };
            ResourceComparisonAssessment {
                tab_count,
                status,
                browser_count: summaries.len(),
                reasons,
            }
        })
        .collect()
}

fn summarize_memory_scaling(
    iterations: &[ResourceBenchmarkIteration],
    summaries: &[ResourceBenchmarkSummary],
    configured_tab_counts: &[usize],
) -> Vec<MemoryScalingScenario> {
    let mut groups = BTreeMap::<(String, usize), Vec<&ResourceBenchmarkIteration>>::new();
    for iteration in iterations.iter().filter(|iteration| !iteration.warmup) {
        if let Some(tab_count) = iteration.tab_count {
            groups
                .entry((iteration.browser.installation_id.clone(), tab_count))
                .or_default()
                .push(iteration);
        }
    }

    let mut scenarios = groups
        .into_iter()
        .map(|((installation_id, tab_count), iterations)| {
            let first = iterations[0];
            let mut mean_footprints = Vec::new();
            let mut peaks = Vec::new();
            let mut successful_iterations = 0;
            for iteration in &iterations {
                if iteration.failure.is_some() {
                    continue;
                }
                let Some(resource) = &iteration.resource else {
                    continue;
                };
                if resource.statistics.valid_samples == 0 {
                    continue;
                }
                successful_iterations += 1;
                if let Some(mean) = resource
                    .statistics
                    .physical_footprint_mean_bytes
                    .filter(|value| value.is_finite())
                {
                    mean_footprints.push(mean);
                }
                if let Some(peak) = resource.statistics.physical_footprint_peak_bytes {
                    peaks.push(peak);
                }
            }
            let footprint_statistics = summarize_metric_values(mean_footprints);
            let mut measurement_status = summaries
                .iter()
                .find(|summary| {
                    summary.browser.installation_id == installation_id
                        && summary.tab_count == Some(tab_count)
                })
                .map_or(ComparabilityStatus::Inconclusive, |summary| {
                    summary.comparison_status
                });
            let mut measurement_reasons = summaries
                .iter()
                .find(|summary| {
                    summary.browser.installation_id == installation_id
                        && summary.tab_count == Some(tab_count)
                })
                .map_or_else(Vec::new, |summary| summary.comparison_reasons.clone());
            if footprint_statistics.is_none() {
                measurement_status = ComparabilityStatus::Inconclusive;
                measurement_reasons.push(
                    "no valid physical-footprint measurements exist for this scenario".to_owned(),
                );
            }
            measurement_reasons.sort();
            measurement_reasons.dedup();
            MemoryScalingScenario {
                browser: first.browser.clone(),
                tab_count,
                successful_iterations,
                mean_footprint_bytes: footprint_statistics.map(|statistics| statistics.mean),
                median_footprint_bytes: footprint_statistics.map(|statistics| statistics.median),
                peak_observed_footprint_bytes: peaks.into_iter().max(),
                incremental_footprint_bytes: None,
                marginal_footprint_per_added_tab_bytes: None,
                measurement_status,
                measurement_reasons,
                incremental_status: None,
                incremental_reasons: Vec::new(),
            }
        })
        .collect::<Vec<_>>();

    let browsers = iterations
        .iter()
        .filter(|iteration| !iteration.warmup)
        .map(|iteration| {
            (
                iteration.browser.installation_id.clone(),
                iteration.browser.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let configured_tab_counts = configured_tab_counts
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for (installation_id, browser) in browsers {
        for tab_count in &configured_tab_counts {
            if !scenarios.iter().any(|scenario| {
                scenario.browser.installation_id == installation_id
                    && scenario.tab_count == *tab_count
            }) {
                scenarios.push(MemoryScalingScenario {
                    browser: browser.clone(),
                    tab_count: *tab_count,
                    successful_iterations: 0,
                    mean_footprint_bytes: None,
                    median_footprint_bytes: None,
                    peak_observed_footprint_bytes: None,
                    incremental_footprint_bytes: None,
                    marginal_footprint_per_added_tab_bytes: None,
                    measurement_status: ComparabilityStatus::Inconclusive,
                    measurement_reasons: vec![
                        "no iteration was recorded for this configured tab-count scenario"
                            .to_owned(),
                    ],
                    incremental_status: None,
                    incremental_reasons: Vec::new(),
                });
            }
        }
    }

    let installation_ids = scenarios
        .iter()
        .map(|scenario| scenario.browser.installation_id.clone())
        .collect::<BTreeSet<_>>();
    for installation_id in installation_ids {
        let mut indexes = scenarios
            .iter()
            .enumerate()
            .filter_map(|(index, scenario)| {
                (scenario.browser.installation_id == installation_id).then_some(index)
            })
            .collect::<Vec<_>>();
        indexes.sort_by_key(|index| scenarios[*index].tab_count);
        for adjacent in indexes.windows(2) {
            let previous_index = adjacent[0];
            let current_index = adjacent[1];
            let previous = &scenarios[previous_index];
            let current = &scenarios[current_index];
            let mut reasons = Vec::new();
            let status = if previous.measurement_status == ComparabilityStatus::Incompatible
                || current.measurement_status == ComparabilityStatus::Incompatible
            {
                ComparabilityStatus::Incompatible
            } else if previous.measurement_status != ComparabilityStatus::Comparable
                || current.measurement_status != ComparabilityStatus::Comparable
            {
                reasons.push(
                    "one or both tab-count scenarios have incomplete measurement evidence; the difference is diagnostic".to_owned(),
                );
                ComparabilityStatus::Inconclusive
            } else {
                ComparabilityStatus::Comparable
            };
            let increment = previous
                .mean_footprint_bytes
                .zip(current.mean_footprint_bytes)
                .map(|(previous, current)| current - previous);
            if increment.is_none() {
                reasons.push(
                    "both adjacent tab-count scenarios need valid physical-footprint measurements"
                        .to_owned(),
                );
            }
            let tabs_added = current.tab_count.saturating_sub(previous.tab_count);
            if tabs_added == 0 {
                reasons.push("tab-count scenarios must be distinct".to_owned());
            }
            let marginal = increment
                .and_then(|increment| (tabs_added > 0).then_some(increment / tabs_added as f64));
            let scenario = &mut scenarios[current_index];
            scenario.incremental_footprint_bytes = increment;
            scenario.marginal_footprint_per_added_tab_bytes = marginal;
            scenario.incremental_status = Some(if increment.is_none() || tabs_added == 0 {
                ComparabilityStatus::Inconclusive
            } else {
                status
            });
            reasons.sort();
            reasons.dedup();
            scenario.incremental_reasons = reasons;
        }
    }
    scenarios.sort_by(|left, right| {
        left.browser
            .installation_id
            .cmp(&right.browser.installation_id)
            .then_with(|| left.tab_count.cmp(&right.tab_count))
    });
    scenarios
}

fn push_optional(values: &mut BTreeMap<&str, Vec<f64>>, key: &'static str, value: Option<f64>) {
    if let Some(value) = value.filter(|value| value.is_finite()) {
        values.entry(key).or_default().push(value);
    }
}

pub fn resource_iteration_duration(config: &ResourceBenchmarkConfig) -> Duration {
    Duration::from_millis(config.measurement_duration_ms)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use velocity_metrics::{ResourceCoverage, ResourceSample, ResourceStatistics};

    use super::*;

    fn valid_config(workload: ResourceWorkload) -> ResourceBenchmarkConfig {
        ResourceBenchmarkConfig::defaults(workload)
    }

    #[test]
    fn validates_workload_specific_options_and_tab_limits() {
        assert!(
            valid_config(ResourceWorkload::Idle)
                .validate(ResourceWorkload::Idle)
                .is_ok()
        );
        let mut idle = valid_config(ResourceWorkload::Idle);
        idle.fixture = Some(FixtureKind::Simple);
        assert!(idle.validate(ResourceWorkload::Idle).is_err());
        let mut tabs = valid_config(ResourceWorkload::Tabs);
        tabs.tab_counts = vec![1, 1];
        assert!(tabs.validate(ResourceWorkload::Tabs).is_err());
        tabs.tab_counts = vec![MAX_TAB_COUNT + 1];
        assert!(tabs.validate(ResourceWorkload::Tabs).is_err());
        tabs.tab_counts = vec![1; MAX_TAB_SCENARIOS + 1];
        assert!(tabs.validate(ResourceWorkload::Tabs).is_err());
        tabs.tab_counts = vec![10];
        tabs.fixture = Some(FixtureKind::Dynamic);
        tabs.dynamic_intensity = 10_001;
        assert!(tabs.validate(ResourceWorkload::Tabs).is_err());
        tabs.tab_counts = vec![MAX_TAB_COUNT];
        tabs.dynamic_intensity = 10_000;
        assert!(tabs.validate(ResourceWorkload::Tabs).is_err());
    }

    #[test]
    fn defaults_match_documented_resource_benchmark_values() {
        let config = valid_config(ResourceWorkload::Tabs);
        assert_eq!(config.runs, 5);
        assert_eq!(config.measurement_duration_ms, 10_000);
        assert_eq!(config.settle_period_ms, 5_000);
        assert_eq!(config.sampling_interval_ms, 100);
        assert_eq!(config.tab_counts, [1, 5, 10]);
    }

    #[test]
    fn report_serialization_preserves_missing_values_and_failure_reasons() {
        let browser = report_browser();
        let iteration = ResourceBenchmarkIteration {
            browser,
            iteration: 1,
            warmup: false,
            diagnostic_pair_id: None,
            diagnostics_enabled: None,
            tab_count: Some(5),
            automation_protocol: None,
            profile_isolation: "fresh isolated profile".to_owned(),
            started_at_unix_ms: 1,
            resource: None,
            responsiveness: None,
            failure: Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Unsupported,
                message: "tab automation not available".to_owned(),
            }),
            cleanup: CleanupStatus::default(),
        };
        let report = ResourceBenchmarkReport::new(
            ResourceWorkload::Tabs,
            valid_config(ResourceWorkload::Tabs),
            RunOrderMetadata::default(),
            vec![iteration],
            false,
        );
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("tab automation not available"));
        assert!(json.contains("INCONCLUSIVE"));
        assert!(json.contains(RESOURCE_MEASUREMENT_METHODOLOGY_VERSION));
        assert!(serde_json::from_str::<ResourceBenchmarkReport>(&json).is_ok());
    }

    #[test]
    fn memory_scaling_reports_scenario_statistics_and_descriptive_margins() {
        let mut iterations = Vec::new();
        for (tab_count, footprint) in [(1, 284.0), (5, 520.0), (10, 754.0)] {
            for iteration in 1..=3 {
                iterations.push(scaling_iteration(
                    tab_count,
                    iteration,
                    Some(footprint),
                    AttributionStatus::VerifiedComplete,
                ));
            }
        }
        let mut config = valid_config(ResourceWorkload::Tabs);
        config.tab_counts = vec![1, 5, 10];
        let report = ResourceBenchmarkReport::new(
            ResourceWorkload::Tabs,
            config,
            RunOrderMetadata::default(),
            iterations,
            false,
        );
        assert_eq!(report.memory_scaling.len(), 3);
        assert_eq!(report.memory_scaling[1].mean_footprint_bytes, Some(520.0));
        assert_eq!(
            report.memory_scaling[1].incremental_footprint_bytes,
            Some(236.0)
        );
        assert_eq!(
            report.memory_scaling[1].marginal_footprint_per_added_tab_bytes,
            Some(59.0)
        );
        assert_eq!(
            report.memory_scaling[2].marginal_footprint_per_added_tab_bytes,
            Some(46.8)
        );
        assert_eq!(
            report.memory_scaling[2].measurement_status,
            ComparabilityStatus::Comparable
        );
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(serialized.contains("memory_scaling"));
        assert!(serde_json::from_str::<ResourceBenchmarkReport>(&serialized).is_ok());
    }

    #[test]
    fn memory_scaling_is_diagnostic_with_partial_coverage_and_missing_values() {
        let mut iterations = Vec::new();
        for tab_count in [1, 5] {
            for iteration in 1..=3 {
                iterations.push(scaling_iteration(
                    tab_count,
                    iteration,
                    (tab_count == 5).then_some(500.0),
                    AttributionStatus::VerifiedPartial,
                ));
            }
        }
        let report = ResourceBenchmarkReport::new(
            ResourceWorkload::Tabs,
            valid_config(ResourceWorkload::Tabs),
            RunOrderMetadata::default(),
            iterations,
            false,
        );
        assert_eq!(report.memory_scaling.len(), 3);
        assert_eq!(report.memory_scaling[0].mean_footprint_bytes, None);
        assert_eq!(
            report.memory_scaling[0].measurement_status,
            ComparabilityStatus::Inconclusive
        );
        assert_eq!(report.memory_scaling[1].incremental_footprint_bytes, None);
        assert_eq!(
            report.memory_scaling[1].incremental_status,
            Some(ComparabilityStatus::Inconclusive)
        );
        assert_eq!(report.memory_scaling[2].mean_footprint_bytes, None);
        assert_eq!(
            report.memory_scaling[2].measurement_status,
            ComparabilityStatus::Inconclusive
        );
    }

    #[test]
    fn summaries_calculate_statistics_across_runs_and_keep_coverage_warnings() {
        let mut iterations = Vec::new();
        for index in 1..=3 {
            let browser = report_browser();
            iterations.push(ResourceBenchmarkIteration {
                browser,
                iteration: index,
                warmup: false,
                diagnostic_pair_id: None,
                diagnostics_enabled: None,
                tab_count: None,
                automation_protocol: None,
                profile_isolation: "fresh isolated profile".to_owned(),
                started_at_unix_ms: index as u128,
                resource: Some(ResourceReport {
                    schema_version: 1,
                    started_at_unix_ms: 1,
                    sampling_interval_ms: 100,
                    samples: vec![ResourceSample {
                        elapsed_ms: 100.0,
                        interval_ms: Some(100.0),
                        wall_clock_unix_ms: Some(1),
                        cpu_percent: Some(index as f64),
                        physical_footprint_bytes: Some(100 * index as u64),
                        resident_bytes: Some(200 * index as u64),
                        process_count: 2,
                        coverage: ResourceCoverage::Partial,
                        attribution: velocity_metrics::ProcessAttributionDiagnostics::default(),
                        processes: Vec::new(),
                        warnings: vec!["helper process excluded".to_owned()],
                        collection_error: None,
                    }],
                    statistics: ResourceStatistics {
                        valid_samples: 1,
                        cpu_mean_percent: Some(index as f64),
                        physical_footprint_mean_bytes: Some(100.0 * index as f64),
                        resident_mean_bytes: Some(200.0 * index as f64),
                        ..ResourceStatistics::default()
                    },
                }),
                responsiveness: None,
                failure: None,
                cleanup: CleanupStatus {
                    browser_closed: true,
                    profile_removed: true,
                    detail: None,
                },
            });
        }
        iterations.push(ResourceBenchmarkIteration {
            browser: report_browser(),
            iteration: 4,
            warmup: false,
            diagnostic_pair_id: None,
            diagnostics_enabled: None,
            tab_count: None,
            automation_protocol: None,
            profile_isolation: "fresh isolated profile".to_owned(),
            started_at_unix_ms: 4,
            resource: None,
            responsiveness: None,
            failure: Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Sampling,
                message: "managed process exited".to_owned(),
            }),
            cleanup: CleanupStatus {
                browser_closed: true,
                profile_removed: true,
                detail: None,
            },
        });
        let summaries = summarize_resource_iterations(&iterations);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].successful_iterations, 3);
        assert_eq!(summaries[0].failed_iterations, 1);
        let cpu = &summaries[0].across_iteration_statistics["cpu_mean_percent"];
        assert_eq!(cpu.sample_count, 3);
        assert_eq!(cpu.median, 2.0);
        assert!(cpu.coefficient_of_variation_percent.is_some());
        assert!(
            summaries[0]
                .coverage_warnings
                .contains(&"helper process excluded".to_owned())
        );
        assert_eq!(
            summaries[0].comparison_status,
            ComparabilityStatus::Inconclusive
        );

        let mut failed_only = iterations.clone();
        for iteration in &mut failed_only[..3] {
            if let Some(resource) = &mut iteration.resource {
                resource.statistics.warnings.clear();
                for sample in &mut resource.samples {
                    sample.coverage = ResourceCoverage::SessionScoped;
                    sample.warnings.clear();
                }
            }
        }
        let failed_only_summary = summarize_resource_iterations(&failed_only);
        assert!(failed_only_summary[0].coverage_warnings.is_empty());
        assert_eq!(failed_only_summary[0].failed_iterations, 1);
        assert_eq!(
            failed_only_summary[0].comparison_status,
            ComparabilityStatus::Inconclusive
        );

        let mut comparison_iterations = iterations[..3].to_vec();
        for (index, original) in iterations[..3].iter().enumerate() {
            let mut other_browser = original.clone();
            other_browser.browser.installation_id = "other-chrome".to_owned();
            other_browser.browser.name = "Other Chrome install".to_owned();
            if let Some(resource) = &mut other_browser.resource {
                resource.statistics.warnings.clear();
                for sample in &mut resource.samples {
                    sample.coverage = ResourceCoverage::SessionScoped;
                    sample.warnings.clear();
                }
            }
            other_browser.iteration = index + 1;
            comparison_iterations.push(other_browser);
        }
        let comparison_summaries = summarize_resource_iterations(&comparison_iterations);
        assert_eq!(comparison_summaries.len(), 2);
        assert_eq!(
            comparison_summaries[1].comparison_status,
            ComparabilityStatus::Inconclusive
        );

        let mut incompatible = iterations[..3].to_vec();
        incompatible[2].profile_isolation = "different profile policy".to_owned();
        assert_eq!(
            summarize_resource_iterations(&incompatible)[0].comparison_status,
            ComparabilityStatus::Incompatible
        );

        let mut fully_covered = iterations[..3].to_vec();
        for iteration in &mut fully_covered {
            let resource = iteration.resource.as_mut().unwrap();
            resource.statistics.attempted_samples = 1;
            resource.statistics.valid_samples = 1;
            resource.statistics.cpu_samples = 1;
            resource.statistics.physical_footprint_samples = 1;
            resource.statistics.resident_samples = 1;
            resource.statistics.attribution.status = AttributionStatus::VerifiedComplete;
            resource.statistics.attribution.verified_processes = 2;
            resource.statistics.attribution.evidence.insert(
                velocity_metrics::AttributionEvidence::PrivateSessionMembership,
                2,
            );
            resource.statistics.warnings.clear();
            for sample in &mut resource.samples {
                sample.coverage = ResourceCoverage::SessionScoped;
                sample.warnings.clear();
                sample.attribution.status = AttributionStatus::VerifiedComplete;
                sample.attribution.verified_processes = 2;
                sample.attribution.evidence.insert(
                    velocity_metrics::AttributionEvidence::PrivateSessionMembership,
                    2,
                );
            }
        }
        assert_eq!(
            summarize_resource_iterations(&fully_covered)[0].comparison_status,
            ComparabilityStatus::Comparable
        );

        fully_covered[0]
            .resource
            .as_mut()
            .unwrap()
            .statistics
            .attribution
            .status = AttributionStatus::VerifiedPartial;
        let partial = summarize_resource_iterations(&fully_covered)[0].clone();
        assert_eq!(partial.comparison_status, ComparabilityStatus::Inconclusive);
        assert!(
            partial
                .comparison_reasons
                .iter()
                .any(|reason| { reason.contains("VERIFIED PARTIAL") })
        );
    }

    #[test]
    fn comparison_assessment_requires_multiple_comparable_browser_summaries() {
        let comparable = vec![
            quality_summary("browser-a", ComparabilityStatus::Comparable),
            quality_summary("browser-b", ComparabilityStatus::Comparable),
        ];
        assert_eq!(
            assess_resource_comparability(&comparable)[0].status,
            ComparabilityStatus::Comparable
        );

        let mut different_scopes = comparable.clone();
        different_scopes[1].attribution.registry_source = Some("other registry".to_owned());
        different_scopes[1].attribution.registry_scope =
            Some("other browser process set".to_owned());
        let assessment = assess_resource_comparability(&different_scopes).remove(0);
        assert_eq!(assessment.status, ComparabilityStatus::Inconclusive);
        assert!(
            assessment
                .reasons
                .iter()
                .any(|reason| { reason.contains("process attribution scopes differ") })
        );

        let inconclusive = vec![
            quality_summary("browser-a", ComparabilityStatus::Comparable),
            quality_summary("browser-b", ComparabilityStatus::Inconclusive),
        ];
        assert_eq!(
            assess_resource_comparability(&inconclusive)[0].status,
            ComparabilityStatus::Inconclusive
        );

        let incompatible = vec![
            quality_summary("browser-a", ComparabilityStatus::Comparable),
            quality_summary("browser-b", ComparabilityStatus::Incompatible),
        ];
        assert_eq!(
            assess_resource_comparability(&incompatible)[0].status,
            ComparabilityStatus::Incompatible
        );

        assert_eq!(
            assess_resource_comparability(&comparable[..1])[0].status,
            ComparabilityStatus::Inconclusive
        );
    }

    fn quality_summary(
        installation_id: &str,
        comparison_status: ComparabilityStatus,
    ) -> ResourceBenchmarkSummary {
        let mut browser = report_browser();
        browser.installation_id = installation_id.to_owned();
        ResourceBenchmarkSummary {
            browser,
            tab_count: None,
            successful_iterations: 3,
            failed_iterations: 0,
            across_iteration_statistics: BTreeMap::new(),
            coverage_warnings: Vec::new(),
            attribution: ProcessAttributionDiagnostics {
                status: AttributionStatus::VerifiedComplete,
                registry_source: Some("fixture process registry".to_owned()),
                registry_scope: Some("fixture managed browser processes".to_owned()),
                registry_snapshot_complete: true,
                ..ProcessAttributionDiagnostics::default()
            },
            sampling_coverage: ResourceSamplingCoverage::default(),
            comparison_status,
            comparison_reasons: Vec::new(),
        }
    }

    fn report_browser() -> ReportBrowser {
        ReportBrowser {
            id: "chrome".to_owned(),
            installation_id: "chrome-fixture".to_owned(),
            name: "Chrome fixture".to_owned(),
            version: Some("1".to_owned()),
            executable_path: PathBuf::from("/fixture/chrome"),
        }
    }

    fn scaling_iteration(
        tab_count: usize,
        iteration: usize,
        footprint_bytes: Option<f64>,
        attribution_status: AttributionStatus,
    ) -> ResourceBenchmarkIteration {
        let mut statistics = ResourceStatistics {
            attempted_samples: 1,
            valid_samples: 1,
            cpu_samples: 1,
            physical_footprint_samples: usize::from(footprint_bytes.is_some()),
            resident_samples: 1,
            cpu_mean_percent: Some(1.0),
            resident_mean_bytes: Some(100.0),
            attribution: ProcessAttributionDiagnostics {
                status: attribution_status,
                verified_processes: 1,
                ..ProcessAttributionDiagnostics::default()
            },
            ..ResourceStatistics::default()
        };
        statistics.physical_footprint_mean_bytes = footprint_bytes;
        statistics.physical_footprint_peak_bytes = footprint_bytes.map(|bytes| bytes as u64 + 10);
        ResourceBenchmarkIteration {
            browser: report_browser(),
            iteration,
            warmup: false,
            diagnostic_pair_id: None,
            diagnostics_enabled: None,
            tab_count: Some(tab_count),
            automation_protocol: Some("test".to_owned()),
            profile_isolation: "fresh isolated profile".to_owned(),
            started_at_unix_ms: iteration as u128,
            resource: Some(ResourceReport {
                schema_version: 3,
                started_at_unix_ms: iteration as u128,
                sampling_interval_ms: 100,
                samples: Vec::new(),
                statistics,
            }),
            responsiveness: None,
            failure: None,
            cleanup: CleanupStatus {
                browser_closed: true,
                profile_removed: true,
                detail: None,
            },
        }
    }
}
