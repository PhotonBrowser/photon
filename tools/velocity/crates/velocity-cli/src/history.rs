use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use velocity_core::{
    RESOURCE_BENCHMARK_SCHEMA_VERSION, ReportBrowser, ResourceBenchmarkReport, ResourceWorkload,
    StartupBenchmarkReport, StartupMetric,
};

const HISTORY_SCHEMA_VERSION: u32 = 1;
const STARTUP_SCHEMA_VERSION: u32 = 2;
const MAX_REPORT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_HISTORY_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ImportMetadata {
    pub label: Option<String>,
    pub user_commit: Option<String>,
    pub capture_hardware: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct HistoryRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub source_sha256: String,
    pub benchmark: String,
    pub benchmark_version: Option<String>,
    pub benchmark_schema_version: u32,
    pub browser_id: String,
    pub installation_id: String,
    pub browser_name: String,
    pub browser_version: Option<String>,
    pub executable_path: Option<PathBuf>,
    pub timestamp_unix_ms: u128,
    pub imported_at_unix_ms: u128,
    pub operating_system: String,
    pub architecture: String,
    pub hardware: Option<HardwareMetadata>,
    pub hardware_observation: String,
    pub repository_at_import: Option<RepositoryMetadata>,
    pub executable_build: ExecutableBuildMetadata,
    pub benchmark_source: Option<Value>,
    pub configuration: Value,
    pub scenario: Option<String>,
    pub summary: Vec<HistoryMetric>,
    pub comparability: ComparabilityMetadata,
    pub original_report_name: String,
    pub archived_report: String,
    pub label: Option<String>,
    pub user_supplied_commit: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct HardwareMetadata {
    pub machine_model: Option<String>,
    pub cpu_model: Option<String>,
    pub logical_cpu_count: Option<usize>,
    #[serde(default)]
    pub host_identity: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RepositoryMetadata {
    pub revision: Option<String>,
    pub branch: Option<String>,
    pub working_tree_dirty: Option<bool>,
    pub observed_at_import: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ExecutableBuildMetadata {
    pub revision: Option<String>,
    pub build_type: Option<String>,
    pub build_timestamp_unix_ms: Option<u128>,
    pub provenance: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct HistoryMetric {
    pub name: String,
    pub unit: String,
    pub median: f64,
    pub mean: Option<f64>,
    pub standard_deviation: Option<f64>,
    pub sample_count: usize,
    pub higher_is_better: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ComparabilityMetadata {
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct NamedBaseline {
    pub name: String,
    pub record_id: String,
    pub created_at_unix_ms: u128,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
struct HistoryStore {
    schema_version: u32,
    records: Vec<HistoryRecord>,
    baselines: BTreeMap<String, NamedBaseline>,
}

#[derive(Clone, Debug)]
pub(crate) struct ImportEntry {
    pub benchmark: String,
    pub benchmark_version: Option<String>,
    pub benchmark_schema_version: u32,
    pub browser: ReportBrowser,
    pub timestamp_unix_ms: u128,
    pub operating_system: String,
    pub architecture: String,
    pub hardware: Option<HardwareMetadata>,
    pub benchmark_source: Option<Value>,
    pub configuration: Value,
    pub scenario: Option<String>,
    pub summary: Vec<HistoryMetric>,
    pub comparability: ComparabilityMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ImportOutcome {
    pub record_ids: Vec<String>,
    pub duplicate_record_ids: Vec<String>,
}

impl HistoryRecord {
    pub(crate) fn metric(&self, name: &str) -> Option<&HistoryMetric> {
        self.summary.iter().find(|metric| metric.name == name)
    }
}

pub(crate) fn default_storage_directory() -> PathBuf {
    if let Some(path) = env::var_os("VELOCITY_DATA_DIR") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Velocity");
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("velocity");
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("velocity")
}

pub(crate) fn import_report(
    report_path: &Path,
    storage_directory: &Path,
    metadata: &ImportMetadata,
) -> Result<ImportOutcome, String> {
    let file = File::open(report_path)
        .map_err(|error| format!("cannot open report {}: {error}", report_path.display()))?;
    let size = file
        .metadata()
        .map_err(|error| format!("cannot inspect report {}: {error}", report_path.display()))?
        .len();
    if size > MAX_REPORT_BYTES {
        return Err(format!(
            "report {} is larger than the supported 512 MiB limit",
            report_path.display()
        ));
    }
    let bytes = fs::read(report_path)
        .map_err(|error| format!("cannot read report {}: {error}", report_path.display()))?;
    let digest = sha256(&bytes);
    let entries = parse_import_entries(&bytes)?;
    let repository = repository_metadata();
    let imported_at_unix_ms = now_unix_ms();
    let report_name = report_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "report.json".to_owned());
    let archived_report = format!("reports/{digest}.json");
    let captured_hardware = metadata.capture_hardware.then(host_hardware_metadata);
    let records = entries
        .into_iter()
        .map(|entry| {
            let record_id = record_id(&digest, &entry);
            let hardware = entry.hardware.clone().or_else(|| captured_hardware.clone());
            let hardware_observation = if entry.hardware.is_some() {
                "reported by benchmark result".to_owned()
            } else if hardware.is_some() {
                "captured immediately after benchmark completion; UUID is hashed".to_owned()
            } else {
                "unknown; source report contains no hardware metadata".to_owned()
            };
            HistoryRecord {
                schema_version: HISTORY_SCHEMA_VERSION,
                record_id,
                source_sha256: digest.clone(),
                benchmark: entry.benchmark,
                benchmark_version: entry.benchmark_version,
                benchmark_schema_version: entry.benchmark_schema_version,
                browser_id: entry.browser.id,
                installation_id: entry.browser.installation_id,
                browser_name: entry.browser.name,
                browser_version: entry.browser.version,
                executable_path: Some(entry.browser.executable_path),
                timestamp_unix_ms: entry.timestamp_unix_ms,
                imported_at_unix_ms,
                operating_system: entry.operating_system,
                architecture: entry.architecture,
                hardware,
                hardware_observation,
                repository_at_import: repository.clone(),
                executable_build: ExecutableBuildMetadata {
                    provenance: "unknown; report contains no verified executable build identity"
                        .to_owned(),
                    ..ExecutableBuildMetadata::default()
                },
                benchmark_source: entry.benchmark_source,
                configuration: entry.configuration,
                scenario: entry.scenario,
                summary: entry.summary,
                comparability: entry.comparability,
                original_report_name: report_name.clone(),
                archived_report: archived_report.clone(),
                label: metadata.label.clone(),
                user_supplied_commit: metadata.user_commit.clone(),
            }
        })
        .collect::<Vec<_>>();

    let _lock = lock_store(storage_directory)?;
    let mut store = load_store(storage_directory)?;
    archive_raw_report(storage_directory, &archived_report, &bytes)?;
    let mut outcome = ImportOutcome::default();
    for record in records {
        if let Some(existing) = store
            .records
            .iter()
            .find(|existing| existing.record_id == record.record_id)
        {
            outcome
                .duplicate_record_ids
                .push(existing.record_id.clone());
            continue;
        }
        outcome.record_ids.push(record.record_id.clone());
        store.records.push(record);
    }
    if !outcome.record_ids.is_empty() {
        save_store(storage_directory, &store)?;
    }
    Ok(outcome)
}

pub(crate) fn list_records(
    storage_directory: &Path,
    browser_filter: Option<&str>,
    benchmark_filter: Option<&str>,
) -> Result<Vec<HistoryRecord>, String> {
    let store = load_store(storage_directory)?;
    let mut records = store
        .records
        .into_iter()
        .filter(|record| {
            browser_filter.is_none_or(|filter| {
                record.browser_id.eq_ignore_ascii_case(filter)
                    || record.browser_name.eq_ignore_ascii_case(filter)
            })
        })
        .filter(|record| {
            benchmark_filter.is_none_or(|filter| {
                record.benchmark.eq_ignore_ascii_case(filter)
                    || record
                        .benchmark_version
                        .as_deref()
                        .is_some_and(|version| version.eq_ignore_ascii_case(filter))
            })
        })
        .collect::<Vec<_>>();
    records.sort_by_key(|record| record.timestamp_unix_ms);
    Ok(records)
}

pub(crate) fn get_record(
    storage_directory: &Path,
    record_id: &str,
) -> Result<HistoryRecord, String> {
    load_store(storage_directory)?
        .records
        .into_iter()
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| format!("history record '{record_id}' was not found"))
}

pub(crate) fn list_baselines(storage_directory: &Path) -> Result<Vec<NamedBaseline>, String> {
    Ok(load_store(storage_directory)?
        .baselines
        .into_values()
        .collect())
}

pub(crate) fn get_baseline(storage_directory: &Path, name: &str) -> Result<NamedBaseline, String> {
    load_store(storage_directory)?
        .baselines
        .remove(name)
        .ok_or_else(|| format!("baseline '{name}' was not found"))
}

pub(crate) fn set_baseline(
    storage_directory: &Path,
    name: &str,
    record_id: &str,
    replace: bool,
) -> Result<(), String> {
    validate_baseline_name(name)?;
    let _lock = lock_store(storage_directory)?;
    let mut store = load_store(storage_directory)?;
    if !store
        .records
        .iter()
        .any(|record| record.record_id == record_id)
    {
        return Err(format!("history record '{record_id}' was not found"));
    }
    if store.baselines.contains_key(name) && !replace {
        return Err(format!(
            "baseline '{name}' already exists; pass --replace to update its reference"
        ));
    }
    store.baselines.insert(
        name.to_owned(),
        NamedBaseline {
            name: name.to_owned(),
            record_id: record_id.to_owned(),
            created_at_unix_ms: now_unix_ms(),
        },
    );
    save_store(storage_directory, &store)
}

pub(crate) fn delete_baseline(storage_directory: &Path, name: &str) -> Result<(), String> {
    let _lock = lock_store(storage_directory)?;
    let mut store = load_store(storage_directory)?;
    if store.baselines.remove(name).is_none() {
        return Err(format!("baseline '{name}' was not found"));
    }
    save_store(storage_directory, &store)
}

pub(crate) fn resolve_report_selector(
    selector: &str,
    storage_directory: &Path,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(selector);
    if path.is_file() {
        return Ok(path);
    }
    let store = load_store(storage_directory)?;
    let record_id = store
        .baselines
        .get(selector)
        .map(|baseline| baseline.record_id.as_str())
        .unwrap_or(selector);
    let record = store
        .records
        .iter()
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| {
            format!(
                "'{}' is neither a report file, history record ID, nor named baseline",
                selector
            )
        })?;
    let archive = checked_archive_path(storage_directory, record)?;
    Ok(archive)
}

pub(crate) fn record_for_selector(
    selector: &str,
    storage_directory: &Path,
) -> Result<Option<HistoryRecord>, String> {
    let store = load_store(storage_directory)?;
    if let Some(baseline) = store.baselines.get(selector) {
        return Ok(store
            .records
            .iter()
            .find(|record| record.record_id == baseline.record_id)
            .cloned());
    }
    if let Some(record) = store
        .records
        .iter()
        .find(|record| record.record_id == selector)
    {
        return Ok(Some(record.clone()));
    }
    let path = PathBuf::from(selector);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read report {}: {error}", path.display()))?;
    let hash = sha256(&bytes);
    Ok(store
        .records
        .iter()
        .find(|record| record.source_sha256 == hash)
        .cloned())
}

pub(crate) fn hardware_comparability(
    left: Option<&HistoryRecord>,
    right: Option<&HistoryRecord>,
) -> Option<bool> {
    let (left, right) = (left?.hardware.as_ref()?, right?.hardware.as_ref()?);
    let known_mismatch = |left: &Option<String>, right: &Option<String>| matches!((left, right), (Some(left), Some(right)) if left != right);
    if known_mismatch(&left.machine_model, &right.machine_model)
        || known_mismatch(&left.cpu_model, &right.cpu_model)
        || matches!(
            (left.logical_cpu_count, right.logical_cpu_count),
            (Some(left), Some(right)) if left != right
        )
    {
        return Some(false);
    }
    match (&left.host_identity, &right.host_identity) {
        (Some(left), Some(right)) => Some(left == right),
        _ => None,
    }
}

pub(crate) fn export_report(
    storage_directory: &Path,
    record_id: &str,
    output_path: &Path,
) -> Result<(), String> {
    let record = get_record(storage_directory, record_id)?;
    let archived = storage_directory.join(&record.archived_report);
    let bytes = fs::read(&archived).map_err(|error| {
        format!(
            "cannot read archived report {}: {error}",
            archived.display()
        )
    })?;
    if sha256(&bytes) != record.source_sha256 {
        return Err(format!(
            "archived report for '{record_id}' failed its SHA-256 integrity check"
        ));
    }
    write_new_artifact(output_path, &bytes)
        .map_err(|error| format!("cannot export report to {}: {error}", output_path.display()))
}

pub(crate) fn parse_import_entries(bytes: &[u8]) -> Result<Vec<ImportEntry>, String> {
    let document: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid report JSON: {error}"))?;
    let benchmark = document
        .get("benchmark")
        .and_then(Value::as_str)
        .ok_or("report is missing a benchmark identity")?;
    match benchmark {
        "startup" => parse_startup_report(bytes),
        "speedometer" => crate::speedometer_cli::parse_history_report(bytes),
        "jetstream" => crate::jetstream_cli::parse_history_report(bytes),
        "idle" | "page" | "tabs" => parse_resource_report(bytes, benchmark),
        other => Err(format!("unsupported benchmark report '{other}'")),
    }
}

fn parse_startup_report(bytes: &[u8]) -> Result<Vec<ImportEntry>, String> {
    let document: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid startup report JSON: {error}"))?;
    let schema = schema_version(&document, "startup")?;
    if schema == 0 || schema > STARTUP_SCHEMA_VERSION {
        return Err(format!(
            "unsupported startup report schema {schema}; supported schemas are 1 through {STARTUP_SCHEMA_VERSION}"
        ));
    }
    let report: StartupBenchmarkReport = serde_json::from_value(document)
        .map_err(|error| format!("invalid startup report: {error}"))?;
    if report.benchmark != "startup" {
        return Err("report benchmark identity does not match startup".to_owned());
    }
    validate_browser(&report.browser)?;
    validate_environment(&report.operating_system, &report.architecture)?;
    if report.timestamp_unix_ms == 0 {
        return Err("startup report timestamp is missing or invalid".to_owned());
    }
    if report.requested_iterations == 0 {
        return Err("startup report has zero requested iterations".to_owned());
    }
    let raw_summary = velocity_core::summarize_results(&report.iterations);
    let mut summary = Vec::new();
    for metric in StartupMetric::ALL {
        if let Some(Some(value)) = report.summary.get(&metric) {
            let raw_count = raw_summary
                .get(&metric)
                .and_then(Option::as_ref)
                .map(|raw| raw.sample_count)
                .unwrap_or_default();
            if value.sample_count == 0 || raw_count != value.sample_count {
                return Err(format!(
                    "startup {} summary count does not match raw measurements",
                    metric.name()
                ));
            }
            validate_stat("startup metric", value.median_ms, value.mean_ms)?;
            summary.push(HistoryMetric {
                name: metric.name().to_owned(),
                unit: "ms".to_owned(),
                median: value.median_ms,
                mean: Some(value.mean_ms),
                standard_deviation: value.standard_deviation_ms,
                sample_count: value.sample_count,
                higher_is_better: Some(false),
            });
        }
    }
    if summary.is_empty() {
        return Err("startup report contains no valid metric summaries".to_owned());
    }
    Ok(vec![ImportEntry {
        benchmark: "startup".to_owned(),
        benchmark_version: None,
        benchmark_schema_version: schema,
        browser: report.browser,
        timestamp_unix_ms: report.timestamp_unix_ms,
        operating_system: report.operating_system,
        architecture: report.architecture,
        hardware: None,
        benchmark_source: None,
        configuration: serde_json::json!({
            "mode": report.mode,
            "requested_iterations": report.requested_iterations,
            "requested_warmup_iterations": report.requested_warmup_iterations,
            "startup_timeout_ms": report.startup_timeout_ms,
            "profile_isolated": report.profile_isolated,
            "profile_strategy": report.profile_strategy,
            "window_ready_condition": report.window_ready_condition,
        }),
        scenario: None,
        summary,
        comparability: ComparabilityMetadata {
            status: if report.profile_isolated {
                "recorded".to_owned()
            } else {
                "inconclusive".to_owned()
            },
            warnings: if report.profile_isolated {
                Vec::new()
            } else {
                vec!["report did not use an isolated browser profile".to_owned()]
            },
        },
    }])
}

fn parse_resource_report(
    bytes: &[u8],
    expected_benchmark: &str,
) -> Result<Vec<ImportEntry>, String> {
    let document: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid resource report JSON: {error}"))?;
    let schema = schema_version(&document, expected_benchmark)?;
    if schema == 0 || schema > RESOURCE_BENCHMARK_SCHEMA_VERSION {
        return Err(format!(
            "unsupported {expected_benchmark} report schema {schema}; newest supported schema is {RESOURCE_BENCHMARK_SCHEMA_VERSION}"
        ));
    }
    let report: ResourceBenchmarkReport = serde_json::from_value(document)
        .map_err(|error| format!("invalid {expected_benchmark} report: {error}"))?;
    if report.benchmark != expected_benchmark {
        return Err(format!(
            "report benchmark identity does not match {expected_benchmark}"
        ));
    }
    let workload = match expected_benchmark {
        "idle" => ResourceWorkload::Idle,
        "page" => ResourceWorkload::Page,
        "tabs" => ResourceWorkload::Tabs,
        _ => unreachable!("resource report caller validates workload"),
    };
    report
        .configuration
        .validate(workload)
        .map_err(|error| format!("invalid {expected_benchmark} configuration: {error}"))?;
    validate_environment(&report.operating_system, &report.architecture)?;
    if report.timestamp_unix_ms == 0 {
        return Err(format!(
            "{expected_benchmark} report timestamp is missing or invalid"
        ));
    }
    let mut entries = Vec::new();
    for summary in &report.summaries {
        validate_browser(&summary.browser)?;
        let actual_successful_iterations = report
            .iterations
            .iter()
            .filter(|iteration| {
                !iteration.warmup
                    && iteration.browser.installation_id == summary.browser.installation_id
                    && iteration.tab_count == summary.tab_count
                    && iteration
                        .resource
                        .as_ref()
                        .is_some_and(|resource| resource.statistics.valid_samples > 0)
                    && iteration.failure.is_none()
            })
            .count();
        if summary.successful_iterations != actual_successful_iterations {
            return Err(format!(
                "{expected_benchmark} successful iteration count does not match raw runs for {}",
                summary.browser.name
            ));
        }
        let metrics = summary
            .across_iteration_statistics
            .iter()
            .map(|(name, value)| {
                validate_stat("resource metric", value.median, value.mean)?;
                Ok(HistoryMetric {
                    name: name.clone(),
                    unit: resource_metric_unit(name).to_owned(),
                    median: value.median,
                    mean: Some(value.mean),
                    standard_deviation: value.sample_standard_deviation,
                    sample_count: value.sample_count,
                    higher_is_better: Some(false),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if metrics
            .iter()
            .any(|metric| metric.sample_count > summary.successful_iterations)
        {
            return Err(format!(
                "{expected_benchmark} summary sample count exceeds successful iterations for {}",
                summary.browser.name
            ));
        }
        if metrics.is_empty() || summary.successful_iterations == 0 {
            continue;
        }
        let mut warnings = summary.comparison_reasons.clone();
        warnings.extend(summary.coverage_warnings.iter().cloned());
        if summary.comparison_status.label() != "COMPARABLE" {
            warnings.push(format!(
                "resource report comparability is {}",
                summary.comparison_status.label()
            ));
        }
        entries.push(ImportEntry {
            benchmark: expected_benchmark.to_owned(),
            benchmark_version: Some(report.measurement_methodology_version.clone()),
            benchmark_schema_version: schema,
            browser: summary.browser.clone(),
            timestamp_unix_ms: report.timestamp_unix_ms,
            operating_system: report.operating_system.clone(),
            architecture: report.architecture.clone(),
            hardware: None,
            benchmark_source: report.fixture.as_ref().map(|fixture| {
                serde_json::to_value(fixture).unwrap_or(Value::Null)
            }),
            configuration: serde_json::json!({
            "benchmark": &report.configuration,
                "measurement_methodology_version": report.measurement_methodology_version,
                "fixture": &report.fixture,
                "tab_count": summary.tab_count,
                "profile_isolated": report.iterations.iter()
                    .filter(|iteration| iteration.browser.installation_id == summary.browser.installation_id && iteration.tab_count == summary.tab_count && !iteration.warmup)
                    .map(|iteration| iteration.profile_isolation.clone())
                    .collect::<std::collections::BTreeSet<_>>(),
            }),
            scenario: summary.tab_count.map(|count| format!("tabs-{count}")),
            summary: metrics,
            comparability: ComparabilityMetadata {
                status: summary.comparison_status.label().to_ascii_lowercase(),
                warnings,
            },
        });
    }
    if entries.is_empty() {
        return Err(format!(
            "{expected_benchmark} report contains no successful measured resource summaries"
        ));
    }
    Ok(entries)
}

fn schema_version(document: &Value, benchmark: &str) -> Result<u32, String> {
    document
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or_else(|| format!("{benchmark} report has no valid schema_version"))
}

fn validate_browser(browser: &ReportBrowser) -> Result<(), String> {
    if browser.id.trim().is_empty()
        || browser.installation_id.trim().is_empty()
        || browser.name.trim().is_empty()
    {
        return Err("report browser metadata is incomplete".to_owned());
    }
    if browser.executable_path.as_os_str().is_empty() {
        return Err("report browser executable path is missing".to_owned());
    }
    Ok(())
}

fn validate_environment(operating_system: &str, architecture: &str) -> Result<(), String> {
    if operating_system.trim().is_empty() || architecture.trim().is_empty() {
        Err("report OS or hardware architecture metadata is missing".to_owned())
    } else {
        Ok(())
    }
}

fn validate_stat(name: &str, median: f64, mean: f64) -> Result<(), String> {
    if !median.is_finite() || !mean.is_finite() || median < 0.0 || mean < 0.0 {
        Err(format!("{name} contains a non-finite or negative summary"))
    } else {
        Ok(())
    }
}

fn resource_metric_unit(name: &str) -> &'static str {
    if name.contains("bytes") {
        "bytes"
    } else if name.contains("percent") {
        "%"
    } else {
        "count"
    }
}

fn record_id(source_sha256: &str, entry: &ImportEntry) -> String {
    let identity = format!(
        "{source_sha256}\0{}\0{}\0{}",
        entry.browser.installation_id,
        entry.benchmark,
        entry.scenario.as_deref().unwrap_or("")
    );
    format!("record-{}", sha256(identity.as_bytes()))
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn lock_store(directory: &Path) -> Result<File, String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create history directory {}: {error}",
            directory.display()
        )
    })?;
    set_private_directory(directory)?;
    let lock_path = directory.join("history.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("cannot open history lock {}: {error}", lock_path.display()))?;
    file.lock_exclusive()
        .map_err(|error| format!("cannot lock history store {}: {error}", directory.display()))?;
    Ok(file)
}

fn load_store(directory: &Path) -> Result<HistoryStore, String> {
    let path = directory.join("history.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(empty_store()),
        Err(error) => {
            return Err(format!(
                "cannot read history store {}: {error}",
                path.display()
            ));
        }
    };
    if bytes.len() as u64 > MAX_HISTORY_BYTES {
        return Err(format!("history store {} exceeds 128 MiB", path.display()));
    }
    let store: HistoryStore = serde_json::from_slice(&bytes)
        .map_err(|error| format!("history store {} is corrupted: {error}", path.display()))?;
    if store.schema_version != HISTORY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported history store schema {}; supported schema is {HISTORY_SCHEMA_VERSION}",
            store.schema_version
        ));
    }
    for (name, baseline) in &store.baselines {
        if !store
            .records
            .iter()
            .any(|record| record.record_id == baseline.record_id)
        {
            return Err(format!(
                "history store baseline '{name}' references missing record '{}'",
                baseline.record_id
            ));
        }
    }
    for record in &store.records {
        if record.schema_version != HISTORY_SCHEMA_VERSION
            || !record.archived_report.starts_with("reports/")
            || Path::new(&record.archived_report)
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "history store record '{}' has an invalid schema or archive reference",
                record.record_id
            ));
        }
    }
    Ok(store)
}

fn checked_archive_path(directory: &Path, record: &HistoryRecord) -> Result<PathBuf, String> {
    let relative = Path::new(&record.archived_report);
    if !record.archived_report.starts_with("reports/")
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "history record '{}' has an unsafe archive path",
            record.record_id
        ));
    }
    Ok(directory.join(relative))
}

fn empty_store() -> HistoryStore {
    HistoryStore {
        schema_version: HISTORY_SCHEMA_VERSION,
        records: Vec::new(),
        baselines: BTreeMap::new(),
    }
}

fn save_store(directory: &Path, store: &HistoryStore) -> Result<(), String> {
    let path = directory.join("history.json");
    let mut bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| format!("cannot serialize history store: {error}"))?;
    bytes.push(b'\n');
    atomic_write(&path, &bytes)
        .map_err(|error| format!("cannot save history store {}: {error}", path.display()))
}

fn archive_raw_report(directory: &Path, relative: &str, bytes: &[u8]) -> Result<(), String> {
    let path = directory.join(relative);
    let parent = path
        .parent()
        .ok_or_else(|| "history archive path has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create report archive {}: {error}", parent.display()))?;
    set_private_directory(parent)?;
    match fs::read(&path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => {
            return Err(format!(
                "archived report {} has a hash collision",
                path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot inspect archived report {}: {error}",
                path.display()
            ));
        }
    }
    atomic_write(&path, bytes)
        .map_err(|error| format!("cannot archive original report {}: {error}", path.display()))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for _ in 0..16 {
        let temporary = parent.join(format!(
            ".{}.{}.{}.tmp",
            path.file_name().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            now_unix_ns()
        ));
        match create_private_file(&temporary) {
            Ok(mut file) => {
                let result = file.write_all(bytes).and_then(|()| file.sync_all());
                if let Err(error) = result {
                    let _ = fs::remove_file(&temporary);
                    return Err(error);
                }
                if let Err(error) = fs::rename(&temporary, path) {
                    let _ = fs::remove_file(&temporary);
                    return Err(error);
                }
                if let Ok(directory) = File::open(parent) {
                    let _ = directory.sync_all();
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate an atomic-write temporary file",
    ))
}

pub(crate) fn write_new_artifact(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".velocity-export-{}.tmp", now_unix_ns()));
    let mut file = create_private_file(&temporary)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

fn create_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn set_private_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!(
                "cannot make history directory {} private: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn host_hardware_metadata() -> HardwareMetadata {
    #[cfg(target_os = "macos")]
    let read_setting = |name: &str| -> Option<String> {
        let output = Command::new("/usr/sbin/sysctl")
            .args(["-n", name])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
        (!value.is_empty()).then_some(value)
    };
    #[cfg(target_os = "macos")]
    let machine_model = read_setting("hw.model");
    #[cfg(target_os = "linux")]
    let machine_model = fs::read_to_string("/sys/devices/virtual/dmi/id/product_name")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let machine_model = None;

    #[cfg(target_os = "macos")]
    let cpu_model = read_setting("machdep.cpu.brand_string");
    #[cfg(target_os = "macos")]
    let host_identity = Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| {
            output.lines().find_map(|line| {
                let (key, value) = line.split_once('=')?;
                key.contains("IOPlatformUUID")
                    .then(|| value.trim().trim_matches('"').to_owned())
            })
        })
        .filter(|value| !value.is_empty())
        .map(|value| sha256(value.as_bytes()));
    #[cfg(target_os = "linux")]
    let cpu_model = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.trim()
                    .eq_ignore_ascii_case("model name")
                    .then(|| value.trim().to_owned())
            })
        });
    #[cfg(target_os = "linux")]
    let host_identity = fs::read_to_string("/etc/machine-id")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(|value| sha256(value.as_bytes()));
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let cpu_model = None;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let host_identity = None;

    HardwareMetadata {
        machine_model,
        cpu_model,
        logical_cpu_count: std::thread::available_parallelism().ok().map(usize::from),
        host_identity,
    }
}

fn validate_baseline_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 80 || name.chars().any(|character| character.is_control()) {
        Err("baseline names must contain 1 to 80 non-control characters".to_owned())
    } else {
        Ok(())
    }
}

fn repository_metadata() -> Option<RepositoryMetadata> {
    let output = |arguments: &[&str]| -> Option<String> {
        let result = Command::new("git").args(arguments).output().ok()?;
        if !result.status.success() {
            return None;
        }
        let value = String::from_utf8(result.stdout).ok()?.trim().to_owned();
        (!value.is_empty()).then_some(value)
    };
    let revision = output(&["rev-parse", "HEAD"]);
    let branch = output(&["branch", "--show-current"]);
    let working_tree_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| !result.stdout.is_empty());
    (revision.is_some() || branch.is_some()).then_some(RepositoryMetadata {
        revision,
        branch,
        working_tree_dirty,
        observed_at_import: true,
    })
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_unix_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use velocity_core::{
        CleanupStatus, RESOURCE_MEASUREMENT_METHODOLOGY_VERSION, ResourceBenchmarkConfig,
        ResourceBenchmarkIteration, ResourceBenchmarkReport, RunOrderMetadata,
    };
    use velocity_metrics::{ResourceCoverage, ResourceReport, ResourceSample, ResourceStatistics};

    fn temporary_directory(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "velocity-history-{name}-{}-{}",
            std::process::id(),
            now_unix_ms()
        ))
    }

    #[test]
    fn invalid_and_future_reports_are_rejected_before_storage_changes() {
        let path = temporary_directory("invalid.json");
        let error =
            parse_import_entries(br#"{"benchmark":"future","schema_version":99}"#).unwrap_err();
        assert!(error.contains("unsupported benchmark"));
        let error =
            parse_import_entries(br#"{"benchmark":"startup","schema_version":999}"#).unwrap_err();
        assert!(error.contains("unsupported startup report schema"));
        let _ = fs::remove_file(path);
    }

    fn startup_report_json() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "benchmark": "startup",
            "browser": {
                "id": "photon",
                "installation_id": "photon-development",
                "name": "Photon",
                "version": null,
                "executable_path": "/Applications/Photon.app/Contents/MacOS/Photon"
            },
            "timestamp_unix_ms": 1792080000000_u64,
            "operating_system": "macos",
            "architecture": "aarch64",
            "mode": "cold",
            "requested_iterations": 3,
            "requested_warmup_iterations": 0,
            "startup_timeout_ms": 30000,
            "profile_isolated": true,
            "profile_strategy": "private profile",
            "os_file_cache_cold": false,
            "window_ready_condition": "stable window",
            "iterations": [
                {"benchmark_id":"startup","browser_id":"photon","iteration":1,"metrics":[{"definition":{"name":"window_ready_ms","unit":"milliseconds"},"value":490.0}],"unavailable_metrics":[],"failures":[]},
                {"benchmark_id":"startup","browser_id":"photon","iteration":2,"metrics":[{"definition":{"name":"window_ready_ms","unit":"milliseconds"},"value":500.0}],"unavailable_metrics":[],"failures":[]},
                {"benchmark_id":"startup","browser_id":"photon","iteration":3,"metrics":[{"definition":{"name":"window_ready_ms","unit":"milliseconds"},"value":510.0}],"unavailable_metrics":[],"failures":[]}
            ],
            "summary": {
                "window_ready_ms": {
                    "sample_count": 3,
                    "median_ms": 500.0,
                    "mean_ms": 500.0,
                    "minimum_ms": 490.0,
                    "maximum_ms": 510.0,
                    "standard_deviation_ms": 10.0,
                    "coefficient_of_variation_percent": 2.0
                }
            },
            "limitations": [],
            "cancelled": false
        })).unwrap()
    }

    fn resource_report_json() -> Vec<u8> {
        let browser = ReportBrowser {
            id: "photon".to_owned(),
            installation_id: "photon-development".to_owned(),
            name: "Photon".to_owned(),
            version: Some("0.1.0".to_owned()),
            executable_path: PathBuf::from("/Applications/Photon.app/Contents/MacOS/Photon"),
        };
        let mut statistics = ResourceStatistics {
            attempted_samples: 1,
            valid_samples: 1,
            cpu_samples: 1,
            physical_footprint_samples: 1,
            resident_samples: 1,
            cpu_mean_percent: Some(20.0),
            physical_footprint_mean_bytes: Some(100.0),
            resident_mean_bytes: Some(200.0),
            peak_process_count: 1,
            ..ResourceStatistics::default()
        };
        statistics.cpu_peak_percent = Some(20.0);
        let resource = ResourceReport {
            schema_version: 1,
            started_at_unix_ms: 1,
            sampling_interval_ms: 100,
            samples: vec![ResourceSample {
                elapsed_ms: 100.0,
                interval_ms: Some(100.0),
                wall_clock_unix_ms: Some(1),
                cpu_percent: Some(20.0),
                physical_footprint_bytes: Some(100),
                resident_bytes: Some(200),
                process_count: 1,
                coverage: ResourceCoverage::SessionScoped,
                attribution: Default::default(),
                processes: Vec::new(),
                warnings: Vec::new(),
                collection_error: None,
            }],
            statistics,
        };
        let report = ResourceBenchmarkReport::new(
            ResourceWorkload::Page,
            ResourceBenchmarkConfig::defaults(ResourceWorkload::Page),
            RunOrderMetadata::default(),
            vec![ResourceBenchmarkIteration {
                browser,
                iteration: 1,
                warmup: false,
                tab_count: None,
                automation_protocol: Some("Photon Automation Protocol".to_owned()),
                profile_isolation: "private profile".to_owned(),
                started_at_unix_ms: 1,
                resource: Some(resource),
                failure: None,
                cleanup: CleanupStatus {
                    browser_closed: true,
                    profile_removed: true,
                    detail: None,
                },
            }],
            false,
        );
        serde_json::to_vec(&report).unwrap()
    }

    #[test]
    fn report_import_is_idempotent_and_preserves_exact_original_bytes() {
        let directory = temporary_directory("import");
        fs::create_dir_all(&directory).unwrap();
        let report_path = directory.join("photon.json");
        let original = startup_report_json();
        fs::write(&report_path, &original).unwrap();
        let store_path = directory.join("history-store");

        let first = import_report(&report_path, &store_path, &ImportMetadata::default()).unwrap();
        let second = import_report(&report_path, &store_path, &ImportMetadata::default()).unwrap();
        assert_eq!(first.record_ids.len(), 1);
        assert_eq!(second.record_ids, Vec::<String>::new());
        assert_eq!(second.duplicate_record_ids, first.record_ids);
        let record = get_record(&store_path, &first.record_ids[0]).unwrap();
        assert_eq!(record.benchmark, "startup");
        assert_eq!(record.browser_version, None);
        assert_eq!(record.executable_build.revision, None);
        assert_eq!(record.summary[0].median, 500.0);
        assert_eq!(
            list_records(&store_path, Some("photon"), Some("startup"))
                .unwrap()
                .len(),
            1
        );
        set_baseline(&store_path, "photon-initial", &record.record_id, false).unwrap();
        assert_eq!(
            fs::read(resolve_report_selector("photon-initial", &store_path).unwrap()).unwrap(),
            original
        );
        assert_eq!(fs::read(&report_path).unwrap(), original);
        assert_eq!(
            fs::read(store_path.join(record.archived_report)).unwrap(),
            original
        );
        let export_path = directory.join("exported.json");
        export_report(&store_path, &record.record_id, &export_path).unwrap();
        assert_eq!(fs::read(export_path).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn startup_summary_without_matching_raw_samples_is_rejected() {
        let mut document: Value = serde_json::from_slice(&startup_report_json()).unwrap();
        document["iterations"] = Value::Array(Vec::new());
        assert!(
            parse_import_entries(&serde_json::to_vec(&document).unwrap())
                .unwrap_err()
                .contains("raw measurements")
        );
    }

    #[test]
    fn resource_schema_import_keeps_methodology_and_sample_coverage() {
        let entries = parse_import_entries(&resource_report_json()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].benchmark, "page");
        assert_eq!(
            entries[0].benchmark_version.as_deref(),
            Some(RESOURCE_MEASUREMENT_METHODOLOGY_VERSION)
        );
        assert_eq!(
            entries[0]
                .summary
                .iter()
                .find(|metric| metric.name == "cpu_mean_percent")
                .unwrap()
                .median,
            20.0
        );
    }

    #[test]
    fn concurrent_duplicate_imports_leave_one_record_and_archive() {
        let directory = temporary_directory("concurrent");
        fs::create_dir_all(&directory).unwrap();
        let report_path = directory.join("startup.json");
        fs::write(&report_path, startup_report_json()).unwrap();
        let store = directory.join("store");
        let threads = (0..4)
            .map(|_| {
                let path = report_path.clone();
                let store = store.clone();
                std::thread::spawn(move || {
                    import_report(&path, &store, &ImportMetadata::default()).unwrap()
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(list_records(&store, None, None).unwrap().len(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn import_storage_failure_does_not_modify_the_source_report() {
        let directory = temporary_directory("storage-failure");
        fs::create_dir_all(&directory).unwrap();
        let report = directory.join("startup.json");
        let original = startup_report_json();
        fs::write(&report, &original).unwrap();
        let invalid_store = directory.join("not-a-directory");
        fs::write(&invalid_store, b"occupied").unwrap();
        assert!(import_report(&report, &invalid_store, &ImportMetadata::default()).is_err());
        assert_eq!(fs::read(&report).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn corrupt_history_store_is_reported_without_being_overwritten() {
        let directory = temporary_directory("corrupt");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("history.json");
        fs::write(&path, b"{not json").unwrap();
        assert!(load_store(&directory).unwrap_err().contains("is corrupted"));
        assert_eq!(fs::read(&path).unwrap(), b"{not json");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn named_baselines_are_immutable_references_and_delete_keeps_records() {
        let directory = temporary_directory("baseline");
        fs::create_dir_all(&directory).unwrap();
        let mut store = empty_store();
        store.records.push(HistoryRecord {
            schema_version: HISTORY_SCHEMA_VERSION,
            record_id: "record-a".to_owned(),
            source_sha256: "hash".to_owned(),
            benchmark: "speedometer".to_owned(),
            benchmark_version: Some("3.1".to_owned()),
            benchmark_schema_version: 2,
            browser_id: "photon".to_owned(),
            installation_id: "photon-dev".to_owned(),
            browser_name: "Photon".to_owned(),
            browser_version: None,
            executable_path: None,
            timestamp_unix_ms: 1,
            imported_at_unix_ms: 2,
            operating_system: "macos".to_owned(),
            architecture: "aarch64".to_owned(),
            hardware: None,
            hardware_observation: "unknown".to_owned(),
            repository_at_import: None,
            executable_build: ExecutableBuildMetadata {
                provenance: "unknown".to_owned(),
                ..ExecutableBuildMetadata::default()
            },
            benchmark_source: None,
            configuration: Value::Null,
            scenario: None,
            summary: Vec::new(),
            comparability: ComparabilityMetadata::default(),
            original_report_name: "report.json".to_owned(),
            archived_report: "reports/hash.json".to_owned(),
            label: None,
            user_supplied_commit: None,
        });
        save_store(&directory, &store).unwrap();
        set_baseline(&directory, "photon-main", "record-a", false).unwrap();
        assert!(set_baseline(&directory, "photon-main", "record-a", false).is_err());
        set_baseline(&directory, "photon-main", "record-a", true).unwrap();
        delete_baseline(&directory, "photon-main").unwrap();
        assert_eq!(
            get_record(&directory, "record-a").unwrap().record_id,
            "record-a"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn hardware_comparison_rejects_known_mismatches_and_warns_on_missing_metadata() {
        let directory = temporary_directory("host-compare");
        fs::create_dir_all(&directory).unwrap();
        let report_path = directory.join("startup.json");
        fs::write(&report_path, startup_report_json()).unwrap();
        let store = directory.join("store");
        let outcome = import_report(&report_path, &store, &ImportMetadata::default()).unwrap();
        let mut left = get_record(&store, &outcome.record_ids[0]).unwrap();
        let mut right = left.clone();
        let hardware = HardwareMetadata {
            machine_model: Some("Mac17,4".to_owned()),
            cpu_model: Some("Apple M5".to_owned()),
            logical_cpu_count: Some(10),
            host_identity: Some("hashed-host-a".to_owned()),
        };
        left.hardware = Some(hardware.clone());
        right.hardware = Some(hardware);
        assert_eq!(
            hardware_comparability(Some(&left), Some(&right)),
            Some(true)
        );
        right.hardware.as_mut().unwrap().host_identity = Some("hashed-host-b".to_owned());
        assert_eq!(
            hardware_comparability(Some(&left), Some(&right)),
            Some(false)
        );
        right.hardware.as_mut().unwrap().host_identity = Some("hashed-host-a".to_owned());
        right.hardware.as_mut().unwrap().cpu_model = None;
        assert_eq!(
            hardware_comparability(Some(&left), Some(&right)),
            Some(true)
        );
        right.hardware.as_mut().unwrap().host_identity = None;
        assert_eq!(hardware_comparability(Some(&left), Some(&right)), None);
        right.hardware.as_mut().unwrap().cpu_model = Some("Different CPU".to_owned());
        assert_eq!(
            hardware_comparability(Some(&left), Some(&right)),
            Some(false)
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
