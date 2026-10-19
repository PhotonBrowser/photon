use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use velocity_core::{
    RESPONSIVENESS_SCHEMA_VERSION, ResponsivenessBenchmarkReport, ResponsivenessSample,
    unique_run_id,
};
use velocity_metrics::{MetricStatistics, summarize_metric_values};

use crate::{
    comparison_statistics::median_permutation_p_value,
    history::{
        default_storage_directory, hardware_comparability, record_for_selector,
        resolve_report_selector,
    },
    terminal::{print_markdown, print_table},
};

#[derive(Clone, Debug, PartialEq)]
struct MetricComparison {
    name: String,
    unit: &'static str,
    baseline: MetricStatistics,
    current: MetricStatistics,
    change_percent: f64,
    p_value: Option<f64>,
    status: &'static str,
}

pub(crate) fn paired_overhead_assessment(
    samples: &[ResponsivenessSample],
    seed: u64,
) -> serde_json::Value {
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    let mut pairs =
        BTreeMap::<usize, (Option<&ResponsivenessSample>, Option<&ResponsivenessSample>)>::new();
    for sample in samples.iter().filter(|sample| !sample.warmup) {
        let (Some(pair_id), Some(enabled)) =
            (sample.diagnostic_pair_id, sample.diagnostics_enabled)
        else {
            continue;
        };
        let pair = pairs.entry(pair_id).or_default();
        if sample.failure.is_some() {
            continue;
        } else if enabled {
            pair.1 = Some(sample);
        } else {
            pair.0 = Some(sample);
        }
    }
    let complete_pairs = pairs
        .iter()
        .filter_map(|(id, (off, on))| match (*off, *on) {
            (Some(off), Some(on)) => Some((*id, off, on)),
            _ => None,
        })
        .collect::<Vec<_>>();

    let metric_definitions = [
        ("workload_duration_ms", "ms"),
        ("process_cpu_mean_percent", "percent"),
        ("physical_footprint_mean_bytes", "bytes"),
        ("physical_footprint_peak_bytes", "bytes"),
        ("resident_memory_mean_bytes", "bytes"),
        ("resident_memory_peak_bytes", "bytes"),
    ];
    let mut metric_results = serde_json::Map::new();
    for (name, unit) in metric_definitions {
        let differences = complete_pairs
            .iter()
            .filter_map(|(id, off, on)| {
                let off_value = overhead_sample_value(off, name)?;
                let on_value = overhead_sample_value(on, name)?;
                Some((*id, off_value, on_value, on_value - off_value))
            })
            .collect::<Vec<_>>();
        let off_values = differences.iter().map(|row| row.1).collect::<Vec<_>>();
        let on_values = differences.iter().map(|row| row.2).collect::<Vec<_>>();
        let deltas = differences.iter().map(|row| row.3).collect::<Vec<_>>();
        let median_difference = median(&deltas);
        let off_median = median(&off_values);
        let percentile_interval = (deltas.len() >= 5)
            .then(|| bootstrap_median_interval(&deltas, seed ^ stable_metric_seed(name)));
        metric_results.insert(name.to_owned(), json!({
            "unit": unit,
            "paired_sample_count": deltas.len(),
            "diagnostics_off_median": off_median,
            "diagnostics_on_median": median(&on_values),
            "median_difference_on_minus_off": median_difference,
            "mean_difference_on_minus_off": (!deltas.is_empty()).then(|| deltas.iter().sum::<f64>() / deltas.len() as f64),
            "median_relative_difference_percent": off_median.filter(|value| value.abs() > f64::EPSILON).zip(median_difference).map(|(base, delta)| delta * 100.0 / base),
            "bootstrap_95_percentile_interval_difference": percentile_interval.map(|(low, high)| json!({"low": low, "high": high})),
            "pair_differences_on_minus_off": differences.iter().map(|(id, _, _, delta)| json!({"pair_id": id, "value": delta})).collect::<Vec<Value>>(),
        }));
    }

    json!({
        "method": "adjacent randomized diagnostics-off/on sessions; paired effect is on minus off within each run-order block",
        "uncertainty_method": "10,000 deterministic percentile-bootstrap resamples of paired median differences; interval omitted below five pairs",
        "seed": seed,
        "complete_successful_pair_count": complete_pairs.len(),
        "incomplete_or_failed_pair_count": pairs.len().saturating_sub(complete_pairs.len()),
        "sessions_share_browser_state": false,
        "interpretation": "paired order blocks control local time drift but each run uses a fresh browser profile; differences are overhead estimates, not optimization regressions",
        "metrics": Value::Object(metric_results),
    })
}

fn overhead_sample_value(sample: &ResponsivenessSample, metric: &str) -> Option<f64> {
    let resources = sample.process_resources.as_ref();
    let verified_resources = resources.filter(|report| {
        !report.samples.is_empty()
            && report
                .samples
                .iter()
                .all(|sample| sample.coverage == velocity_metrics::ResourceCoverage::SessionScoped)
    });
    match metric {
        "workload_duration_ms" => sample.workload_duration_ms,
        "process_cpu_mean_percent" => verified_resources?.statistics.cpu_mean_percent,
        "physical_footprint_mean_bytes" => {
            verified_resources?.statistics.physical_footprint_mean_bytes
        }
        "physical_footprint_peak_bytes" => verified_resources?
            .statistics
            .physical_footprint_peak_bytes
            .map(|value| value as f64),
        "resident_memory_mean_bytes" => verified_resources?.statistics.resident_mean_bytes,
        "resident_memory_peak_bytes" => verified_resources?
            .statistics
            .resident_peak_bytes
            .map(|value| value as f64),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    })
}

fn bootstrap_median_interval(values: &[f64], mut seed: u64) -> (f64, f64) {
    const BOOTSTRAP_SAMPLES: usize = 10_000;
    let mut bootstrap = Vec::with_capacity(BOOTSTRAP_SAMPLES);
    let mut resample = Vec::with_capacity(values.len());
    if seed == 0 {
        seed = 0x9e37_79b9_7f4a_7c15;
    }
    for _ in 0..BOOTSTRAP_SAMPLES {
        resample.clear();
        for _ in 0..values.len() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            resample.push(values[seed as usize % values.len()]);
        }
        bootstrap.push(median(&resample).unwrap_or_default());
    }
    bootstrap.sort_by(f64::total_cmp);
    (bootstrap[249], bootstrap[9_749])
}

fn stable_metric_seed(metric: &str) -> u64 {
    metric
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

pub(crate) fn run_compare(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut baseline = None;
    let mut current = None;
    let mut threshold_percent = 5.0_f64;
    let mut output = None;
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        if !seen.insert(option.clone()) {
            return Err(format!("{option} may be specified only once"));
        }
        match option.as_str() {
            "--baseline" => baseline = Some(required(&mut arguments, &option)?),
            "--current" => current = Some(required(&mut arguments, &option)?),
            "--threshold-percent" | "--threshold" => {
                let value = required(&mut arguments, &option)?;
                threshold_percent = value
                    .parse()
                    .map_err(|_| format!("invalid percentage '{value}'"))?;
                if !threshold_percent.is_finite() || threshold_percent < 0.0 {
                    return Err("--threshold-percent must be finite and non-negative".to_owned());
                }
            }
            "--output" => output = Some(PathBuf::from(required(&mut arguments, &option)?)),
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                return Err(format!(
                    "unexpected responsiveness comparison option '{other}'"
                ));
            }
        }
    }

    let baseline_selector =
        baseline.ok_or("compare responsiveness requires --baseline <report-or-name>")?;
    let current_selector =
        current.ok_or("compare responsiveness requires --current <report-or-name>")?;
    let storage = default_storage_directory();
    let baseline_record = record_for_selector(&baseline_selector, &storage)?;
    let current_record = record_for_selector(&current_selector, &storage)?;
    if hardware_comparability(baseline_record.as_ref(), current_record.as_ref()) == Some(false) {
        return Err(
            "reports have a known hardware or host identity mismatch and are not comparable"
                .to_owned(),
        );
    }
    let baseline_path = resolve_report_selector(&baseline_selector, &storage)?;
    let current_path = resolve_report_selector(&current_selector, &storage)?;
    let baseline_report = read_report(&baseline_path)?;
    let current_report = read_report(&current_path)?;
    validate_compatibility(&baseline_report, &current_report)?;

    let rows = compare_reports(&baseline_report, &current_report, threshold_percent)?;
    if rows.is_empty() {
        return Err(
            "reports contain no matching responsiveness metrics with at least five samples per side"
                .to_owned(),
        );
    }
    print_comparison(&rows, threshold_percent);

    let mut warnings = Vec::new();
    if hardware_comparability(baseline_record.as_ref(), current_record.as_ref()).is_none() {
        warnings.push("Host hardware identity is unavailable; matching OS and architecture do not prove the runs used the same machine.".to_owned());
    }
    if baseline_report.engine_revision.is_none() || current_report.engine_revision.is_none() {
        warnings.push("At least one Engine revision is unknown.".to_owned());
    }
    if baseline_report
        .coverage
        .process_resource_complete_iterations
        < baseline_report.coverage.successful_iterations
        || current_report.coverage.process_resource_complete_iterations
            < current_report.coverage.successful_iterations
    {
        warnings.push("At least one report has incomplete managed-session process-tree resource coverage; process-resource comparisons are omitted for that report.".to_owned());
    }
    if diagnostics_enabled(&baseline_report) != diagnostics_enabled(&current_report) {
        warnings.push("Engine diagnostics were enabled for only one report. Fixture duration and throughput can estimate the collection overhead for this workload. Process-tree CPU and memory differences are overhead-control signals and should not be treated as optimization results without separate controls.".to_owned());
    }
    let output_path = output.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap_or_else(|| Path::new("tools/velocity"))
            .join("results")
            .join(format!(
                "responsiveness-comparison-{}.html",
                unique_run_id()
            ))
    });
    write_comparison_html(
        &output_path,
        &baseline_path,
        &current_path,
        &rows,
        &warnings,
        threshold_percent,
    )?;
    println!("Saved HTML: {}", output_path.display());
    if rows.iter().any(|row| row.status == "regression") {
        return Err("responsiveness regression detected".to_owned());
    }
    Ok(())
}

fn required(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn read_report(path: &Path) -> Result<ResponsivenessBenchmarkReport, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read report {}: {error}", path.display()))?;
    let report: ResponsivenessBenchmarkReport = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid responsiveness report {}: {error}", path.display()))?;
    if report.benchmark != "responsiveness"
        || report.benchmark_type != "browser_responsiveness"
        || report.schema_version != RESPONSIVENESS_SCHEMA_VERSION
    {
        return Err("report is not a supported responsiveness schema".to_owned());
    }
    crate::history::parse_import_entries(&bytes)?;
    Ok(report)
}

fn validate_compatibility(
    baseline: &ResponsivenessBenchmarkReport,
    current: &ResponsivenessBenchmarkReport,
) -> Result<(), String> {
    if baseline.schema_version != current.schema_version {
        return Err("responsiveness reports must use the same report schema".to_owned());
    }
    if baseline
        .configuration
        .get("diagnostics_mode")
        .and_then(serde_json::Value::as_str)
        == Some("balanced_interleaved")
        || current
            .configuration
            .get("diagnostics_mode")
            .and_then(serde_json::Value::as_str)
            == Some("balanced_interleaved")
    {
        return Err(
            "balanced diagnostics-overhead reports use paired samples; inspect their embedded overhead_assessment instead of the independent baseline comparison".to_owned(),
        );
    }
    if baseline.browser.id != current.browser.id {
        return Err("responsiveness comparison requires the same browser ID".to_owned());
    }
    if baseline.machine_information != current.machine_information {
        return Err("responsiveness reports must have matching OS and architecture".to_owned());
    }
    let workload_parameter_keys: &[&str] = match baseline.workload.as_str() {
        "task-backlog" => &["task_count", "task_source"],
        "gc-pressure" => &[
            "allocation_record_count",
            "allocation_cycles",
            "records_per_cycle",
            "retained_graph_sample_period",
            "temporary_dom_nodes_per_cycle",
        ],
        "dom-style" => &[
            "dom_style_mutation_count",
            "dom_style_branch_count",
            "dom_style_items_per_branch",
            "dom_style_selector_kinds",
        ],
        "style-localized" | "style-descendant" | "style-sibling" | "style-relational" => &[
            "style_invalidation_mutation_count",
            "style_invalidation_tree_shape",
            "style_invalidation_selector",
        ],
        "layout-updates" => &[
            "layout_update_count",
            "layout_normal_flow_count",
            "layout_absolute_count",
        ],
        "animation" => &[
            "animation_phase_count",
            "animation_phase_duration_ms",
            "animation_pre_roll_ms",
            "frame_interval_pre_roll_rule_version",
        ],
        _ => &[],
    };
    let diagnostic_parameter_keys = [
        "diagnostic_schema_version",
        "sample_capacity_per_webcontent_process",
    ];
    let resource_parameter_keys = [
        "process_resource_sampling_interval_ms",
        "process_resource_scope",
    ];
    let resource_parameters_match = resource_parameter_keys
        .iter()
        .all(|key| baseline.configuration.get(key) == current.configuration.get(key));
    let same_workload = baseline.workload == current.workload
        && baseline.fixture_revision == current.fixture_revision
        && resource_parameters_match
        && !workload_parameter_keys.is_empty()
        && workload_parameter_keys
            .iter()
            .chain(diagnostic_parameter_keys.iter())
            .all(|key| {
                baseline.configuration.get(key).is_some()
                    && baseline.configuration.get(key) == current.configuration.get(key)
            });
    if !same_workload {
        return Err(
            "responsiveness workloads, fixture revisions, and diagnostic definitions must match"
                .to_owned(),
        );
    }
    if baseline.iterations != current.iterations || baseline.warmup_count != current.warmup_count {
        return Err(
            "responsiveness reports must use the same measured iteration and warm-up counts"
                .to_owned(),
        );
    }
    let baseline_diagnostics = diagnostics_enabled(baseline);
    let current_diagnostics = diagnostics_enabled(current);
    if baseline.coverage.successful_iterations < 5
        || current.coverage.successful_iterations < 5
        || (baseline_diagnostics && baseline.coverage.complete_single_process_samples < 5)
        || (current_diagnostics && current.coverage.complete_single_process_samples < 5)
    {
        return Err(
            "at least five successful iterations are required on each side; diagnostics-enabled reports also need five complete single-process samples".to_owned(),
        );
    }
    Ok(())
}

fn compare_reports(
    baseline: &ResponsivenessBenchmarkReport,
    current: &ResponsivenessBenchmarkReport,
    threshold_percent: f64,
) -> Result<Vec<MetricComparison>, String> {
    if !threshold_percent.is_finite() || threshold_percent < 0.0 {
        return Err("comparison threshold must be finite and non-negative".to_owned());
    }
    let baseline_metrics = extract_metric_samples(baseline);
    let current_metrics = extract_metric_samples(current);
    let mut rows = Vec::new();
    for (name, baseline_samples) in &baseline_metrics {
        let Some(current_samples) = current_metrics.get(name) else {
            continue;
        };
        let (Some(baseline_statistics), Some(current_statistics)) = (
            summarize_metric_values(baseline_samples.iter().copied()),
            summarize_metric_values(current_samples.iter().copied()),
        ) else {
            continue;
        };
        if baseline_statistics.sample_count < 5 || current_statistics.sample_count < 5 {
            continue;
        }
        if baseline_statistics.median <= 0.0 {
            continue;
        }
        let change_percent = (current_statistics.median - baseline_statistics.median)
            / baseline_statistics.median
            * 100.0;
        let p_value = median_permutation_p_value(baseline_samples, current_samples);
        let higher_is_better = name.contains("throughput") || name.ends_with("_per_second");
        let crosses_threshold = change_percent.abs() >= threshold_percent;
        let status = match p_value {
            Some(p) if crosses_threshold && p < 0.05 => {
                if (change_percent > 0.0) == higher_is_better {
                    "improvement"
                } else {
                    "regression"
                }
            }
            Some(p) if crosses_threshold && p >= 0.05 => "inconclusive",
            Some(_) => "within threshold",
            None => "inconclusive",
        };
        rows.push(MetricComparison {
            name: name.clone(),
            unit: metric_unit(name),
            baseline: baseline_statistics,
            current: current_statistics,
            change_percent,
            p_value,
            status,
        });
    }
    Ok(rows)
}

fn extract_metric_samples(report: &ResponsivenessBenchmarkReport) -> BTreeMap<String, Vec<f64>> {
    let mut metrics = BTreeMap::<String, Vec<f64>>::new();
    let resource_coverage_complete = report.coverage.process_resource_complete_iterations
        == report.coverage.successful_iterations
        && report.coverage.process_resource_complete_iterations >= 5;
    for sample in &report.samples {
        if sample.warmup || sample.failure.is_some() {
            continue;
        }
        if let Some(duration) = sample
            .workload_duration_ms
            .filter(|value| value.is_finite() && *value > 0.0)
        {
            match report.workload.as_str() {
                "task-backlog" => {
                    insert(&mut metrics, "task_backlog_duration_ms", duration);
                    insert(
                        &mut metrics,
                        "task_backlog_throughput_tasks_per_second",
                        512_000.0 / duration,
                    );
                }
                "gc-pressure" => {
                    if let Some(record_count) = report
                        .configuration
                        .get("allocation_record_count")
                        .and_then(serde_json::Value::as_f64)
                    {
                        insert(&mut metrics, "gc_pressure_duration_ms", duration);
                        insert(
                            &mut metrics,
                            "gc_pressure_records_per_second",
                            record_count * 1000.0 / duration,
                        );
                    }
                }
                "dom-style" => {
                    if let Some(mutation_count) = report
                        .configuration
                        .get("dom_style_mutation_count")
                        .and_then(serde_json::Value::as_f64)
                    {
                        insert(&mut metrics, "dom_style_duration_ms", duration);
                        insert(
                            &mut metrics,
                            "dom_style_mutations_per_second",
                            mutation_count * 1000.0 / duration,
                        );
                    }
                }
                "style-localized" | "style-descendant" | "style-sibling" | "style-relational" => {
                    let metric = format!("{}_duration_ms", report.workload.replace('-', "_"));
                    insert(&mut metrics, &metric, duration);
                    if let Some(mutation_count) = report
                        .configuration
                        .get("style_invalidation_mutation_count")
                        .and_then(serde_json::Value::as_f64)
                    {
                        let metric =
                            format!("{}_mutations_per_second", report.workload.replace('-', "_"));
                        insert(&mut metrics, &metric, mutation_count * 1000.0 / duration);
                    }
                }
                "layout-updates" => {
                    if let Some(update_count) = report
                        .configuration
                        .get("layout_update_count")
                        .and_then(serde_json::Value::as_f64)
                    {
                        insert(&mut metrics, "layout_update_duration_ms", duration);
                        insert(
                            &mut metrics,
                            "layout_updates_per_second",
                            update_count * 1000.0 / duration,
                        );
                    }
                }
                "animation" => {
                    insert(&mut metrics, "animation_active_window_ms", duration);
                }
                _ => {}
            }
        }
        if resource_coverage_complete {
            if let Some(resources) = sample.process_resources.as_ref().filter(|resources| {
                !resources.samples.is_empty()
                    && resources.samples.iter().all(|sample| {
                        sample.coverage == velocity_metrics::ResourceCoverage::SessionScoped
                    })
            }) {
                let statistics = &resources.statistics;
                for (name, value) in [
                    (
                        "process_tree_physical_footprint_peak_bytes",
                        statistics
                            .physical_footprint_peak_bytes
                            .map(|value| value as f64),
                    ),
                    (
                        "process_tree_physical_footprint_mean_bytes",
                        statistics.physical_footprint_mean_bytes,
                    ),
                    (
                        "process_tree_resident_peak_bytes",
                        statistics.resident_peak_bytes.map(|value| value as f64),
                    ),
                    (
                        "process_tree_resident_mean_bytes",
                        statistics.resident_mean_bytes,
                    ),
                    ("process_tree_cpu_mean_percent", statistics.cpu_mean_percent),
                ] {
                    if let Some(value) = value {
                        insert(&mut metrics, name, value);
                    }
                }
            }
        }
        let Some(snapshot) = sample.metrics.as_ref() else {
            continue;
        };
        if let Some(frame_summary) = snapshot.get("frame_timing_summary") {
            for name in [
                "frame_interval_p50_ms",
                "frame_interval_p95_ms",
                "frame_interval_p99_ms",
                "frame_interval_max_ms",
                "frame_interval_outlier_count",
                "frame_actual_drawable_presentation_count",
                "frame_reused_count",
                "frame_zero_presented_time_event_count",
                "frame_presentation_event_retention_percent",
            ] {
                if let Some(value) = frame_summary.get(name).and_then(serde_json::Value::as_f64) {
                    insert(&mut metrics, name, value);
                }
            }
        }
        if let Some(fixture_metrics) = snapshot.get("fixture_metrics") {
            for name in [
                "raf_interval_p50_ms",
                "raf_interval_p95_ms",
                "raf_interval_p99_ms",
                "raf_interval_max_ms",
                "raf_callback_count",
            ] {
                if let Some(value) = fixture_metrics
                    .get(name)
                    .and_then(serde_json::Value::as_f64)
                {
                    insert(&mut metrics, name, value);
                }
            }
        }
        let Some(processes) = snapshot
            .pointer("/metrics/processes")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let available = processes
            .iter()
            .filter(|process| {
                process
                    .get("available")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            })
            .collect::<Vec<_>>();
        if available.len() != 1 {
            continue;
        }
        let Some(diagnostics) = available[0].get("diagnostics") else {
            continue;
        };
        if diagnostics
            .get("sample_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            == 0
        {
            continue;
        }
        for (name, path) in [
            ("task_queue_wait_p50_ms", "/overall/queue_wait/p50_ms"),
            ("task_queue_wait_p95_ms", "/overall/queue_wait/p95_ms"),
            ("task_queue_wait_p99_ms", "/overall/queue_wait/p99_ms"),
            ("task_queue_wait_max_ms", "/overall/queue_wait/max_ms"),
            (
                "task_execution_duration_p95_ms",
                "/overall/execution_duration/p95_ms",
            ),
            (
                "event_loop_queue_depth_at_enqueue_mean",
                "/overall/queue_depth_at_enqueue_mean",
            ),
        ] {
            if let Some(value) = diagnostics
                .pointer(path)
                .and_then(serde_json::Value::as_f64)
            {
                insert(&mut metrics, name, value);
            }
        }
        if let Some(value) = diagnostics
            .pointer("/by_source")
            .and_then(serde_json::Value::as_array)
            .and_then(|sources| {
                sources.iter().find(|source| {
                    source.get("source").and_then(serde_json::Value::as_str) == Some("timer")
                })
            })
            .and_then(|timer| timer.pointer("/queue_wait/p95_ms"))
            .and_then(serde_json::Value::as_f64)
        {
            insert(&mut metrics, "timer_task_queue_wait_p95_ms", value);
        }
        if diagnostics
            .pointer("/style_and_layout/available")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            for (name, path) in [
                (
                    "style_update_duration_ms",
                    "/style_and_layout/style_update_duration_ms",
                ),
                (
                    "style_reaction_elements",
                    "/style_and_layout/style_reaction_elements",
                ),
                (
                    "style_reaction_batches",
                    "/style_and_layout/style_reaction_batches",
                ),
                (
                    "style_update_submission_duration_ms",
                    "/style_and_layout/style_update_submission_duration_ms",
                ),
                (
                    "style_update_bridge_duration_ms",
                    "/style_and_layout/style_update_bridge_duration_ms",
                ),
                (
                    "style_update_apply_duration_ms",
                    "/style_and_layout/style_update_apply_duration_ms",
                ),
                (
                    "style_update_remainder_duration_ms",
                    "/style_and_layout/style_update_remainder_duration_ms",
                ),
                (
                    "style_engine_transaction_setup_duration_ms",
                    "/style_and_layout/style_engine_transaction_setup_duration_ms",
                ),
                (
                    "style_engine_transaction_setup_count",
                    "/style_and_layout/style_engine_transaction_setup_count",
                ),
                (
                    "style_engine_transaction_count",
                    "/style_and_layout/style_engine_transaction_count",
                ),
                (
                    "style_unscoped_transaction_count",
                    "/style_and_layout/style_unscoped_transaction_count",
                ),
                (
                    "style_unscoped_transaction_reaction_entries",
                    "/style_and_layout/style_unscoped_transaction_reaction_entries",
                ),
                (
                    "style_broad_matching_batch_count",
                    "/style_and_layout/style_broad_matching_batch_count",
                ),
                (
                    "style_narrow_matching_batch_count",
                    "/style_and_layout/style_narrow_matching_batch_count",
                ),
                (
                    "style_broad_reaction_elements",
                    "/style_and_layout/style_broad_reaction_elements",
                ),
                (
                    "style_narrow_reaction_elements",
                    "/style_and_layout/style_narrow_reaction_elements",
                ),
                (
                    "style_broad_matching_apply_duration_ms",
                    "/style_and_layout/style_broad_matching_apply_duration_ms",
                ),
                (
                    "style_narrow_matching_apply_duration_ms",
                    "/style_and_layout/style_narrow_matching_apply_duration_ms",
                ),
                ("full_layout_count", "/style_and_layout/full_layout_count"),
                (
                    "partial_layout_count",
                    "/style_and_layout/partial_layout_count",
                ),
                (
                    "layout_full_duration_ms",
                    "/style_and_layout/layout_full_duration_ms",
                ),
                (
                    "layout_partial_duration_ms",
                    "/style_and_layout/layout_partial_duration_ms",
                ),
            ] {
                if let Some(value) = diagnostics
                    .pointer(path)
                    .and_then(serde_json::Value::as_f64)
                {
                    insert(&mut metrics, name, value);
                }
            }
        }
        if let Some(value) = diagnostics
            .pointer("/gc/collection_count")
            .and_then(serde_json::Value::as_f64)
        {
            insert(&mut metrics, "gc_collection_count", value);
        }
        for (phase, phase_path) in [
            ("mark", "/gc/gc_mark_duration_ms"),
            ("synchronous_sweep", "/gc/gc_synchronous_sweep_duration_ms"),
            ("pause", "/gc/gc_pause_duration_ms"),
        ] {
            for percentile in ["p50", "p95", "p99", "max"] {
                let name = format!("gc_{phase}_duration_{percentile}_ms");
                let path = format!("{phase_path}/{percentile}_ms");
                if let Some(value) = diagnostics
                    .pointer(&path)
                    .and_then(serde_json::Value::as_f64)
                {
                    insert(&mut metrics, &name, value);
                }
            }
        }
        if diagnostics
            .pointer("/gc/gc_live_heap_available")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            if let Some(value) = diagnostics
                .pointer("/gc/gc_live_heap_bytes")
                .and_then(serde_json::Value::as_f64)
            {
                insert(&mut metrics, "gc_live_heap_bytes", value);
            }
        }
    }
    metrics
}

fn insert(metrics: &mut BTreeMap<String, Vec<f64>>, name: &str, value: f64) {
    if value.is_finite() {
        metrics.entry(name.to_owned()).or_default().push(value);
    }
}

fn metric_unit(name: &str) -> &'static str {
    if name.ends_with("_ms") {
        "ms"
    } else if name == "event_loop_queue_depth_at_enqueue_mean" {
        "tasks"
    } else if name == "gc_pressure_records_per_second" {
        "records/s"
    } else if name == "dom_style_mutations_per_second" {
        "mutations/s"
    } else if name == "layout_updates_per_second" {
        "updates/s"
    } else if name.ends_with("_per_second") {
        "tasks/s"
    } else if name == "style_reaction_elements" {
        "elements"
    } else if name == "style_reaction_batches" {
        "batches"
    } else if name == "full_layout_count" || name == "partial_layout_count" {
        "layouts"
    } else if name == "gc_collection_count" {
        "collections"
    } else if name == "animation_raf_callback_count"
        || name == "frame_gpu_completion_callback_count"
    {
        "callbacks"
    } else if name == "frame_gpu_submission_count" {
        "submissions"
    } else if name == "frame_actual_drawable_presentation_count" || name == "frame_reused_count" {
        "presentations"
    } else if name == "frame_interval_sample_count" || name == "frame_interval_outlier_count" {
        "intervals"
    } else if name == "frame_coalesced_before_install_count" {
        "surface updates"
    } else if name == "frame_zero_presented_time_event_count" {
        "events"
    } else if name.ends_with("_bytes") {
        "bytes"
    } else if name.ends_with("_percent") {
        "%"
    } else {
        "count"
    }
}

fn diagnostics_enabled(report: &ResponsivenessBenchmarkReport) -> bool {
    report
        .configuration
        .get("diagnostics_enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn print_comparison(rows: &[MetricComparison], threshold_percent: f64) {
    print_markdown(&format!(
        "## Responsiveness comparison\n\nChanges are based on per-iteration samples. A regression or improvement requires a threshold crossing of {threshold_percent:.2}% and a two-sided median permutation p-value below 0.05."
    ));
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            vec![
                row.name.clone(),
                format!("{:.3} {}", row.baseline.median, row.unit),
                format!("{:.3} {}", row.current.median, row.unit),
                format!("{:+.2}%", row.change_percent),
                row.p_value
                    .map_or_else(|| "N/A".to_owned(), |value| format!("{value:.4}")),
                row.status.to_owned(),
                format!(
                    "{} / {}",
                    row.baseline.sample_count, row.current.sample_count
                ),
            ]
        })
        .collect();
    print_table(
        &[
            "Metric",
            "Baseline median",
            "Current median",
            "Change",
            "p",
            "Status",
            "Samples",
        ],
        table,
        &[1, 2, 3, 4, 6],
    );
}

fn write_comparison_html(
    path: &Path,
    baseline_path: &Path,
    current_path: &Path,
    rows: &[MetricComparison],
    warnings: &[String],
    threshold_percent: f64,
) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let rows = rows
        .iter()
        .map(|row| {
            format!(
                "<tr><td>{}</td><td>{:.3} {}</td><td>{:.3} {}</td><td>{:+.2}%</td><td>{}</td><td>{}</td><td>{} / {}</td></tr>",
                escape(&row.name),
                row.baseline.median,
                row.unit,
                row.current.median,
                row.unit,
                row.change_percent,
                row.p_value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.4}")),
                row.status,
                row.baseline.sample_count,
                row.current.sample_count,
            )
        })
        .collect::<String>();
    let warnings = warnings
        .iter()
        .map(|warning| format!("<li>{}</li>", escape(warning)))
        .collect::<String>();
    let html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Velocity responsiveness comparison</title><h1>Responsiveness comparison</h1><p>Baseline: <code>{}</code><br>Current: <code>{}</code></p><p>At least five observations per side are required. Classification uses a two-sided median permutation test (p &lt; 0.05) and a {:.2}% relative threshold.</p><table><thead><tr><th>Metric</th><th>Baseline median</th><th>Current median</th><th>Change</th><th>p</th><th>Status</th><th>Samples</th></tr></thead><tbody>{rows}</tbody></table><h2>Warnings</h2><ul>{warnings}</ul>",
        escape(&baseline_path.display().to_string()),
        escape(&current_path.display().to_string()),
        threshold_percent,
    );
    fs::write(path, html).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn print_help() {
    print_markdown(
        "## `velocity compare responsiveness`\n\nUsage: `velocity compare responsiveness --baseline <report-or-name> --current <report-or-name> [--threshold-percent PERCENT] [--output FILE.html]`. Reports must use the same browser, machine OS/architecture, workload, fixture revision, diagnostic schema and iteration counts. At least five complete single-process iterations are required on each side. Statistical labels require both the configured relative threshold and p &lt; 0.05; otherwise changes are shown as within-threshold or inconclusive.",
    );
}

#[cfg(test)]
mod tests {
    use super::{
        compare_reports, extract_metric_samples, paired_overhead_assessment, validate_compatibility,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use velocity_core::{
        ReportBrowser, ResponsivenessBenchmarkReport, ResponsivenessCoverage,
        ResponsivenessMachineInformation, ResponsivenessSample,
    };
    use velocity_metrics::{ResourceCoverage, ResourceReport, ResourceSample, ResourceStatistics};

    fn process_resource_report(footprint: u64) -> ResourceReport {
        ResourceReport {
            schema_version: 3,
            started_at_unix_ms: 1,
            sampling_interval_ms: 100,
            samples: vec![ResourceSample {
                elapsed_ms: 100.0,
                interval_ms: Some(100.0),
                wall_clock_unix_ms: Some(101),
                cpu_percent: Some(12.0),
                physical_footprint_bytes: Some(footprint),
                resident_bytes: Some(footprint / 2),
                process_count: 2,
                coverage: ResourceCoverage::SessionScoped,
                attribution: Default::default(),
                processes: Vec::new(),
                warnings: Vec::new(),
                collection_error: None,
            }],
            statistics: ResourceStatistics {
                attempted_samples: 1,
                valid_samples: 1,
                cpu_samples: 1,
                physical_footprint_samples: 1,
                resident_samples: 1,
                cpu_mean_percent: Some(12.0),
                physical_footprint_mean_bytes: Some(footprint as f64),
                physical_footprint_peak_bytes: Some(footprint),
                resident_mean_bytes: Some((footprint / 2) as f64),
                resident_peak_bytes: Some(footprint / 2),
                ..ResourceStatistics::default()
            },
        }
    }

    fn report(values: [f64; 5]) -> ResponsivenessBenchmarkReport {
        let samples = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| ResponsivenessSample {
                iteration: index + 1,
                warmup: false,
                diagnostic_pair_id: None,
                diagnostics_enabled: Some(true),
                workload_duration_ms: Some(512.0),
                metrics: Some(serde_json::json!({
                    "metrics": {"processes": [{
                        "available": true,
                        "diagnostics": {
                            "sample_count": 512,
                            "overall": {
                                "queue_wait": {"p50_ms": value, "p95_ms": value, "p99_ms": value, "max_ms": value},
                                "execution_duration": {"p95_ms": 0.2}
                            },
                            "queue_depth_at_enqueue_mean": 1.0,
                            "by_source": [{"source": "timer", "queue_wait": {"p95_ms": value}}],
                            "gc": {
                                "collection_count": 2,
                                "timing_sample_count": 2,
                                "gc_mark_duration_ms": {"p50_ms": 0.4, "p95_ms": 0.5, "p99_ms": 0.5, "max_ms": 0.6},
                                "gc_synchronous_sweep_duration_ms": {"p50_ms": 0.1, "p95_ms": 0.2, "p99_ms": 0.2, "max_ms": 0.3},
                                "gc_pause_duration_ms": {"p50_ms": 0.8, "p95_ms": 1.0, "p99_ms": 1.0, "max_ms": 1.1},
                                "gc_live_heap_available": true,
                                "gc_live_heap_bytes": 4096
                            }
                        }
                    }]}
                })),
                process_resources: None,
                process_resource_error: None,
                failure: None,
            })
            .collect::<Vec<_>>();
        ResponsivenessBenchmarkReport {
            benchmark: "responsiveness".to_owned(),
            benchmark_type: "browser_responsiveness".to_owned(),
            schema_version: super::RESPONSIVENESS_SCHEMA_VERSION,
            run_id: "test".to_owned(),
            timestamp_unix_ms: 1,
            browser: ReportBrowser {
                id: "photon".to_owned(),
                installation_id: "photon-development".to_owned(),
                name: "Photon".to_owned(),
                version: None,
                executable_path: PathBuf::from("Photon"),
            },
            browser_version: None,
            engine_revision: None,
            executable_revision: None,
            workload: "task-backlog".to_owned(),
            fixture_revision: "1".to_owned(),
            configuration: serde_json::json!({
                "task_count": 512,
                "task_source": "timer",
                "diagnostic_schema_version": 1,
                "sample_capacity_per_webcontent_process": 1024,
                "diagnostics_enabled": true
            }),
            machine_information: ResponsivenessMachineInformation {
                operating_system: "macos".to_owned(),
                architecture: "aarch64".to_owned(),
            },
            display_refresh_rate_hz: None,
            iterations: 5,
            warmup_count: 0,
            metrics: BTreeMap::new(),
            samples,
            coverage: ResponsivenessCoverage {
                requested_iterations: 5,
                successful_iterations: 5,
                failed_iterations: 0,
                warmup_iterations: 0,
                process_scoped_snapshots: 5,
                complete_single_process_samples: 5,
                retained_task_samples: 2560,
                style_layout_available_snapshots: 5,
                style_layout_incomplete_snapshots: 0,
                gc_timing_available_snapshots: 5,
                gc_live_heap_available_snapshots: 5,
                ..ResponsivenessCoverage::default()
            },
            warnings: Vec::new(),
            cancelled: false,
        }
    }

    #[test]
    fn presentation_metric_units_match_their_measurement_kind() {
        assert_eq!(
            super::metric_unit("frame_actual_drawable_presentation_count"),
            "presentations"
        );
        assert_eq!(
            super::metric_unit("frame_interval_sample_count"),
            "intervals"
        );
        assert_eq!(
            super::metric_unit("frame_interval_outlier_count"),
            "intervals"
        );
        assert_eq!(
            super::metric_unit("frame_gpu_submission_count"),
            "submissions"
        );
        assert_eq!(
            super::metric_unit("frame_gpu_completion_callback_count"),
            "callbacks"
        );
        assert_eq!(super::metric_unit("frame_reused_count"), "presentations");
        assert_eq!(
            super::metric_unit("frame_coalesced_before_install_count"),
            "surface updates"
        );
        assert_eq!(
            super::metric_unit("event_loop_queue_depth_at_enqueue_mean"),
            "tasks"
        );
        assert_eq!(super::metric_unit("unknown_count_metric"), "count");
    }

    #[test]
    fn task_queue_comparison_requires_matching_definitions_and_significant_evidence() {
        let baseline = report([1.0, 1.1, 1.2, 1.3, 1.4]);
        let current = report([4.0, 4.1, 4.2, 4.3, 4.4]);
        validate_compatibility(&baseline, &current).unwrap();
        let rows = compare_reports(&baseline, &current, 5.0).unwrap();
        let queue = rows
            .iter()
            .find(|row| row.name == "task_queue_wait_p95_ms")
            .unwrap();
        assert_eq!(queue.status, "regression");
        assert!(queue.p_value.unwrap() < 0.05);
        assert!(
            rows.iter()
                .any(|row| row.name == "gc_pause_duration_p95_ms")
        );
        assert!(rows.iter().any(|row| row.name == "gc_live_heap_bytes"));

        let mut changed_fixture = current;
        changed_fixture.fixture_revision = "2".to_owned();
        assert!(validate_compatibility(&baseline, &changed_fixture).is_err());

        let mut disabled = baseline.clone();
        disabled.configuration["diagnostics_enabled"] = serde_json::json!(false);
        disabled.coverage.complete_single_process_samples = 0;
        for sample in &mut disabled.samples {
            sample.metrics = Some(serde_json::Value::Null);
        }
        let enabled = report([1.0, 1.1, 1.2, 1.3, 1.4]);
        validate_compatibility(&disabled, &enabled).unwrap();
        let overhead_rows = compare_reports(&disabled, &enabled, 5.0).unwrap();
        assert_eq!(overhead_rows.len(), 2);
        assert!(
            overhead_rows
                .iter()
                .all(|row| row.name.starts_with("task_backlog_"))
        );
    }

    #[test]
    fn animation_comparison_requires_matching_presentation_pre_roll_rule() {
        let animation_report = || {
            let mut report = report([1.0, 1.1, 1.2, 1.3, 1.4]);
            report.workload = "animation".to_owned();
            report.fixture_revision = velocity_core::ANIMATION_FIXTURE_REVISION.to_owned();
            report.configuration["animation_phase_count"] =
                serde_json::json!(velocity_core::ANIMATION_PHASE_COUNT);
            report.configuration["animation_phase_duration_ms"] =
                serde_json::json!(velocity_core::ANIMATION_PHASE_DURATION_MS);
            report.configuration["animation_pre_roll_ms"] =
                serde_json::json!(velocity_core::ANIMATION_PRE_ROLL_MS);
            report.configuration["frame_interval_pre_roll_rule_version"] =
                serde_json::json!(velocity_core::FRAME_INTERVAL_PRE_ROLL_RULE_VERSION);
            report
        };
        let baseline = animation_report();
        validate_compatibility(&baseline, &animation_report()).unwrap();

        let mut changed_rule = animation_report();
        changed_rule.configuration["frame_interval_pre_roll_rule_version"] = serde_json::json!(1);
        assert!(validate_compatibility(&baseline, &changed_rule).is_err());
    }

    #[test]
    fn process_tree_resource_metrics_require_complete_attribution_and_matching_provenance() {
        let mut baseline = report([1.0, 1.1, 1.2, 1.3, 1.4]);
        let mut current = report([1.0, 1.1, 1.2, 1.3, 1.4]);
        for (index, sample) in baseline.samples.iter_mut().enumerate() {
            sample.process_resources = Some(process_resource_report(10_000 + index as u64));
        }
        for (index, sample) in current.samples.iter_mut().enumerate() {
            sample.process_resources = Some(process_resource_report(20_000 + index as u64));
        }
        for report in [&mut baseline, &mut current] {
            report.coverage.process_resource_iterations = 5;
            report.coverage.process_resource_complete_iterations = 5;
            report.coverage.process_resource_sample_count = 5;
            report.configuration["process_resource_sampling_interval_ms"] =
                serde_json::Value::from(100);
            report.configuration["process_resource_scope"] =
                serde_json::Value::from("verified_managed_process_session_tree");
        }

        validate_compatibility(&baseline, &current).unwrap();
        let samples = extract_metric_samples(&baseline);
        assert_eq!(
            samples["process_tree_physical_footprint_peak_bytes"],
            vec![10_000.0, 10_001.0, 10_002.0, 10_003.0, 10_004.0]
        );
        assert!(
            compare_reports(&baseline, &current, 5.0)
                .unwrap()
                .iter()
                .any(|row| row.name == "process_tree_physical_footprint_peak_bytes")
        );

        baseline.coverage.process_resource_complete_iterations = 4;
        assert!(
            !extract_metric_samples(&baseline)
                .contains_key("process_tree_physical_footprint_peak_bytes")
        );
        assert!(validate_compatibility(&baseline, &current).is_ok());
        let rows = compare_reports(&baseline, &current, 5.0).unwrap();
        assert!(
            !rows
                .iter()
                .any(|row| row.name == "process_tree_physical_footprint_peak_bytes")
        );
    }

    #[test]
    fn overhead_assessment_uses_paired_differences_and_keeps_incomplete_pair_count() {
        let mut samples = Vec::new();
        for pair_id in 1..=6 {
            for (enabled, duration) in [(false, 20.0), (true, 21.0)] {
                samples.push(ResponsivenessSample {
                    iteration: pair_id * 2 - usize::from(enabled),
                    warmup: false,
                    diagnostic_pair_id: Some(pair_id),
                    diagnostics_enabled: Some(enabled),
                    workload_duration_ms: Some(duration + pair_id as f64),
                    metrics: None,
                    process_resources: None,
                    process_resource_error: None,
                    failure: None,
                });
            }
        }
        samples.push(ResponsivenessSample {
            iteration: 13,
            warmup: false,
            diagnostic_pair_id: Some(7),
            diagnostics_enabled: Some(true),
            workload_duration_ms: None,
            metrics: None,
            process_resources: None,
            process_resource_error: None,
            failure: Some("fixture failed".to_owned()),
        });

        let result = paired_overhead_assessment(&samples, 123);
        assert_eq!(result["complete_successful_pair_count"], 6);
        assert_eq!(result["incomplete_or_failed_pair_count"], 1);
        let duration = &result["metrics"]["workload_duration_ms"];
        assert_eq!(duration["paired_sample_count"], 6);
        assert_eq!(duration["median_difference_on_minus_off"], 1.0);
        assert!(duration["bootstrap_95_percentile_interval_difference"].is_object());
        assert_eq!(duration["pair_differences_on_minus_off"][0]["pair_id"], 1);
    }
}
