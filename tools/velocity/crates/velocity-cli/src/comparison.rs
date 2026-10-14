use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use velocity_core::{MetricSummary, StartupBenchmarkReport, StartupMetric, unique_run_id};

use crate::terminal::{print_markdown, print_table};

const MIN_REGRESSION_SAMPLES: usize = 3;
pub const DEFAULT_MIN_ABSOLUTE_REGRESSION_MS: f64 = 1.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegressionStatus {
    Regression,
    WithinThreshold,
    Improvement,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegressionThresholds {
    pub relative_percent: f64,
    pub absolute_ms: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct RegressionRow {
    pub metric: StartupMetric,
    pub baseline: MetricSummary,
    pub current: MetricSummary,
    pub change_percent: f64,
    pub change_ms: f64,
    pub status: RegressionStatus,
}

pub fn compare_startup_reports(
    baseline: &StartupBenchmarkReport,
    current: &StartupBenchmarkReport,
    thresholds: RegressionThresholds,
) -> Result<Vec<RegressionRow>, String> {
    if !thresholds.relative_percent.is_finite() || thresholds.relative_percent < 0.0 {
        return Err("regression threshold must be a finite, non-negative percentage".to_owned());
    }
    if !thresholds.absolute_ms.is_finite() || thresholds.absolute_ms < 0.0 {
        return Err(
            "minimum absolute change must be finite and non-negative milliseconds".to_owned(),
        );
    }
    if baseline.benchmark != "startup" || current.benchmark != "startup" {
        return Err("both reports must contain the startup benchmark".to_owned());
    }
    if baseline.browser.id != current.browser.id {
        return Err(format!(
            "cannot compare '{}' with '{}' reports",
            baseline.browser.id, current.browser.id
        ));
    }
    if baseline.operating_system != current.operating_system
        || baseline.architecture != current.architecture
    {
        return Err(
            "baseline and current reports must use the same OS and architecture".to_owned(),
        );
    }
    if baseline.mode != current.mode
        || baseline.requested_iterations != current.requested_iterations
        || baseline.requested_warmup_iterations != current.requested_warmup_iterations
        || baseline.startup_timeout_ms != current.startup_timeout_ms
        || baseline.profile_isolated != current.profile_isolated
        || baseline.profile_strategy != current.profile_strategy
        || baseline.window_ready_condition != current.window_ready_condition
    {
        return Err(
            "run counts, warm-up counts, timeout, startup mode, profile strategy, and window readiness condition must match".to_owned(),
        );
    }
    if !baseline.profile_isolated {
        return Err("regression analysis requires isolated browser profiles".to_owned());
    }
    if baseline.run_order.comparison_run_id.is_some()
        && current.run_order.comparison_run_id.is_some()
    {
        let baseline_pool = baseline
            .run_order
            .measured_order_by_round
            .iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        let current_pool = current
            .run_order
            .measured_order_by_round
            .iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        if baseline.run_order.randomized != current.run_order.randomized
            || baseline_pool != current_pool
        {
            return Err(
                "randomization policy and comparison browser set must match between reports"
                    .to_owned(),
            );
        }
    }

    let rows = StartupMetric::ALL
        .into_iter()
        .filter_map(|metric| {
            let baseline = baseline.summary.get(&metric).copied().flatten()?;
            let current = current.summary.get(&metric).copied().flatten()?;
            if baseline.sample_count < MIN_REGRESSION_SAMPLES
                || current.sample_count < MIN_REGRESSION_SAMPLES
                || baseline.median_ms <= 0.0
            {
                return None;
            }
            let change_ms = current.median_ms - baseline.median_ms;
            let change_percent =
                (current.median_ms - baseline.median_ms) / baseline.median_ms * 100.0;
            let status = if change_percent > thresholds.relative_percent
                && change_ms > thresholds.absolute_ms
            {
                RegressionStatus::Regression
            } else if change_percent < -thresholds.relative_percent
                && change_ms < -thresholds.absolute_ms
            {
                RegressionStatus::Improvement
            } else {
                RegressionStatus::WithinThreshold
            };
            Some(RegressionRow {
                metric,
                baseline,
                current,
                change_percent,
                change_ms,
                status,
            })
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err(format!(
            "reports have no comparable metric summaries with at least {MIN_REGRESSION_SAMPLES} valid samples"
        ));
    }
    Ok(rows)
}

pub fn print_browser_comparison(reports: &[StartupBenchmarkReport]) {
    print_markdown("## Startup comparison\n\n**Window ready** · lower times are better.");
    let mut rows = Vec::with_capacity(reports.len());
    for report in reports {
        let summary = report
            .summary
            .get(&StartupMetric::WindowReady)
            .copied()
            .flatten();
        let name = format!("{} [{}]", report.browser.name, report.browser.id);
        match summary {
            Some(summary) if report.profile_isolated => rows.push(vec![
                name,
                format!("{:.1} ms", summary.median_ms),
                format!("{:.1} ms", summary.mean_ms),
                format_ms(summary.standard_deviation_ms),
                format_percent(summary.coefficient_of_variation_percent),
                summary.sample_count.to_string(),
            ]),
            _ => rows.push(vec![
                name,
                "N/A".to_owned(),
                "N/A".to_owned(),
                "N/A".to_owned(),
                "N/A".to_owned(),
                "0".to_owned(),
            ]),
        }
    }
    print_table(
        &["Browser", "Median", "Mean", "Std Dev", "CV", "Samples"],
        rows,
        &[1, 2, 3, 4, 5],
    );
    let requested_iterations = reports
        .first()
        .map(|report| report.requested_iterations)
        .unwrap_or_default();
    print_markdown(&format!(
        "**Measured iterations:** {requested_iterations} per browser."
    ));
    if reports.len() > 1 {
        print_markdown(
            "> Values are descriptive. Profile isolation differs by engine; inspect report metadata before comparing browsers.",
        );
    }
}

pub fn save_browser_comparison_html(
    directory: &Path,
    comparison_run_id: &str,
    reports: &[StartupBenchmarkReport],
) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create HTML report directory {}: {error}",
            directory.display()
        )
    })?;
    let path = directory.join(format!("startup-comparison-{comparison_run_id}.html"));
    let body = render_browser_comparison_html(comparison_run_id, reports);
    write_unique(&path, body.as_bytes())
}

pub fn save_regression_html(
    directory: &Path,
    baseline_path: &Path,
    current_path: &Path,
    baseline: &StartupBenchmarkReport,
    current: &StartupBenchmarkReport,
    thresholds: RegressionThresholds,
    rows: &[RegressionRow],
) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create HTML report directory {}: {error}",
            directory.display()
        )
    })?;
    let path = directory.join(format!("startup-regression-{}.html", unique_run_id()));
    let body = render_regression_html(
        baseline_path,
        current_path,
        baseline,
        current,
        thresholds,
        rows,
    );
    write_unique(&path, body.as_bytes())
}

pub fn print_regression_comparison(
    rows: &[RegressionRow],
    thresholds: RegressionThresholds,
) -> bool {
    print_markdown("## Startup regression comparison\n\nMedian latency by metric.");
    let table_rows = rows
        .iter()
        .map(|row| {
            vec![
                row.metric.name().to_owned(),
                format!("{:.1} ms", row.baseline.median_ms),
                format!("{:.1} ms", row.current.median_ms),
                format!("{:+.1}%", row.change_percent),
                format!("{:+.2} ms", row.change_ms),
                row.baseline.sample_count.to_string(),
                row.current.sample_count.to_string(),
                status_label(row.status).to_owned(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(
        &[
            "Metric",
            "Baseline",
            "Current",
            "Change",
            "Δ ms",
            "N baseline",
            "N current",
            "Status",
        ],
        table_rows,
        &[1, 2, 3, 4, 5, 6],
    );
    print_markdown(&format!(
        "**Regression thresholds:** more than {:.1}% slower and more than {:.2} ms absolute change.\n\nMetrics with fewer than {MIN_REGRESSION_SAMPLES} valid samples in either report are excluded.",
        thresholds.relative_percent, thresholds.absolute_ms
    ));
    rows.iter()
        .any(|row| row.status == RegressionStatus::Regression)
}

fn render_browser_comparison_html(
    comparison_run_id: &str,
    reports: &[StartupBenchmarkReport],
) -> String {
    let mut html = html_header("Velocity startup comparison");
    html.push_str("<h1>Velocity — Startup Comparison</h1>");
    html.push_str(&format!(
        "<p>Comparison run <code>{}</code></p>",
        escape_html(comparison_run_id)
    ));
    html.push_str("<table><thead><tr><th>Browser</th><th>Version</th><th>Median</th><th>Mean</th><th>Std Dev</th><th>CV</th><th>Samples</th></tr></thead><tbody>");
    for report in reports {
        let summary = report
            .summary
            .get(&StartupMetric::WindowReady)
            .copied()
            .flatten();
        let (median, mean, deviation, coefficient, samples) = match summary {
            Some(summary) if report.profile_isolated => (
                format!("{:.1} ms", summary.median_ms),
                format!("{:.1} ms", summary.mean_ms),
                format_ms(summary.standard_deviation_ms),
                format_percent(summary.coefficient_of_variation_percent),
                summary.sample_count.to_string(),
            ),
            _ => (
                "N/A".into(),
                "N/A".into(),
                "N/A".into(),
                "N/A".into(),
                "0".into(),
            ),
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{median}</td><td>{mean}</td><td>{deviation}</td><td>{coefficient}</td><td>{samples}</td></tr>",
            escape_html(&format!("{} [{}]", report.browser.name, report.browser.id)),
            escape_html(report.browser.version.as_deref().unwrap_or("unknown")),
        ));
    }
    html.push_str("</tbody></table>");
    html.push_str("<p>Values are descriptive. Startup profiles use browser-specific isolation mechanisms, and window readiness measures a stable native window rather than page readiness.</p>");
    html.push_str("<h2>Run metadata and raw measurements</h2>");
    for report in reports {
        let serialized = serde_json::to_string_pretty(report).unwrap_or_else(|error| {
            format!("Could not serialize this report for display: {error}")
        });
        html.push_str(&format!(
            "<details><summary>{} — {} — {} measured iterations</summary><pre>{}</pre></details>",
            escape_html(&report.browser.name),
            escape_html(report.browser.version.as_deref().unwrap_or("unknown")),
            report.requested_iterations,
            escape_html(&serialized),
        ));
    }
    html.push_str("</main></body></html>");
    html
}

fn render_regression_html(
    baseline_path: &Path,
    current_path: &Path,
    baseline: &StartupBenchmarkReport,
    current: &StartupBenchmarkReport,
    thresholds: RegressionThresholds,
    rows: &[RegressionRow],
) -> String {
    let mut html = html_header("Velocity startup regression analysis");
    html.push_str(&format!(
        "<h1>Velocity — Startup Regression Analysis</h1><p>{} version {} compared with {} version {}.</p>",
        escape_html(&current.browser.name),
        escape_html(current.browser.version.as_deref().unwrap_or("unknown")),
        escape_html(&baseline.browser.name),
        escape_html(baseline.browser.version.as_deref().unwrap_or("unknown")),
    ));
    html.push_str(&format!(
        "<p>Baseline: <code>{}</code><br>Current: <code>{}</code><br>Regression requires both more than {:.1}% and more than {:.2} ms slower<br>Metrics require at least {MIN_REGRESSION_SAMPLES} valid samples in each report.</p>",
        escape_html(&baseline_path.display().to_string()),
        escape_html(&current_path.display().to_string()),
        thresholds.relative_percent,
        thresholds.absolute_ms,
    ));
    html.push_str("<table><thead><tr><th>Metric</th><th>Baseline median</th><th>Current median</th><th>Change</th><th>Change (ms)</th><th>Status</th><th>Baseline samples</th><th>Current samples</th></tr></thead><tbody>");
    for row in rows {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{:.1} ms</td><td>{:.1} ms</td><td>{:+.1}%</td><td>{:+.2} ms</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            row.metric.name(),
            row.baseline.median_ms,
            row.current.median_ms,
            row.change_percent,
            row.change_ms,
            status_label(row.status),
            row.baseline.sample_count,
            row.current.sample_count,
        ));
    }
    html.push_str("</tbody></table><h2>Raw report metadata</h2>");
    for (label, report) in [("Baseline", baseline), ("Current", current)] {
        let serialized = serde_json::to_string_pretty(report).unwrap_or_default();
        html.push_str(&format!(
            "<details><summary>{label} report</summary><pre>{}</pre></details>",
            escape_html(&serialized),
        ));
    }
    html.push_str("</main></body></html>");
    html
}

fn html_header(title: &str) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><style>:root{--page:#f7f8fa;--text:#202124;--line:#d8dce2;--surface:#fff;--subtle:#eef1f5;--code:#f2f4f7}body{font:16px/1.5 -apple-system,BlinkMacSystemFont,sans-serif;color:var(--text);background:var(--page);margin:0}main{max-width:1100px;margin:40px auto;padding:0 24px}h1,h2{line-height:1.2}table{border-collapse:collapse;width:100%;background:var(--surface);margin:24px 0}th,td{border-bottom:1px solid var(--line);padding:10px 12px;text-align:right;white-space:nowrap}th:first-child,td:first-child{text-align:left}thead{background:var(--subtle)}details{background:var(--surface);border:1px solid var(--line);border-radius:6px;margin:12px 0;padding:12px}summary{cursor:pointer;font-weight:600}pre{overflow:auto;max-height:600px;background:var(--code);padding:16px;border-radius:4px}code{overflow-wrap:anywhere}</style><title>",
    );
    html.push_str(&escape_html(title));
    html.push_str("</title></head><body><main>");
    html
}

fn write_unique(path: &Path, contents: &[u8]) -> Result<PathBuf, String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create HTML report {}: {error}", path.display()))?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(path);
        return Err(format!(
            "cannot write HTML report {}: {error}",
            path.display()
        ));
    }
    Ok(path.to_path_buf())
}

fn format_ms(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.1} ms"))
}

fn format_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.1}%"))
}

fn status_label(status: RegressionStatus) -> &'static str {
    match status {
        RegressionStatus::Regression => "REGRESSION",
        RegressionStatus::WithinThreshold => "within threshold",
        RegressionStatus::Improvement => "improvement",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use velocity_core::{MetricSummary, ReportBrowser, RunOrderMetadata, StartupMode};

    use super::*;

    fn report(median_ms: f64) -> StartupBenchmarkReport {
        let summary = MetricSummary {
            sample_count: 3,
            median_ms,
            mean_ms: median_ms,
            minimum_ms: median_ms,
            maximum_ms: median_ms,
            standard_deviation_ms: Some(0.0),
            coefficient_of_variation_percent: Some(0.0),
        };
        StartupBenchmarkReport {
            schema_version: 2,
            benchmark: "startup".to_owned(),
            browser: ReportBrowser {
                id: "photon".to_owned(),
                installation_id: "photon-test".to_owned(),
                name: "Photon".to_owned(),
                version: Some("0.1.0".to_owned()),
                executable_path: PathBuf::from("/fixture/Photon.app/Contents/MacOS/Photon"),
            },
            timestamp_unix_ms: 1,
            operating_system: "macos".to_owned(),
            architecture: "aarch64".to_owned(),
            mode: StartupMode::Cold,
            requested_iterations: 3,
            requested_warmup_iterations: 0,
            startup_timeout_ms: 30_000,
            profile_isolated: true,
            profile_strategy: "private-test-profile".to_owned(),
            os_file_cache_cold: false,
            window_ready_condition: "verified window".to_owned(),
            iterations: Vec::new(),
            warmup_iterations: Vec::new(),
            run_order: RunOrderMetadata::default(),
            run_failures: Vec::new(),
            summary: BTreeMap::from([
                (StartupMetric::WindowReady, Some(summary)),
                (StartupMetric::ProcessStart, None),
                (StartupMetric::WindowVisible, None),
            ]),
            limitations: Vec::new(),
            cancelled: false,
        }
    }

    fn compare(
        baseline_ms: f64,
        current_ms: f64,
        threshold_percent: f64,
        min_absolute_ms: f64,
    ) -> RegressionStatus {
        compare_startup_reports(
            &report(baseline_ms),
            &report(current_ms),
            RegressionThresholds {
                relative_percent: threshold_percent,
                absolute_ms: min_absolute_ms,
            },
        )
        .unwrap()[0]
            .status
    }

    #[test]
    fn relative_threshold_detects_regressions_when_absolute_floor_is_zero() {
        assert_eq!(
            compare(100.0, 106.0, 5.0, 0.0),
            RegressionStatus::Regression
        );
        assert_eq!(
            compare(100.0, 104.0, 5.0, 0.0),
            RegressionStatus::WithinThreshold
        );
    }

    #[test]
    fn absolute_threshold_is_also_required() {
        assert_eq!(
            compare(100.0, 111.0, 0.0, 10.0),
            RegressionStatus::Regression
        );
        assert_eq!(
            compare(100.0, 109.0, 0.0, 10.0),
            RegressionStatus::WithinThreshold
        );
    }

    #[test]
    fn both_regression_thresholds_must_be_exceeded() {
        assert_eq!(
            compare(100.0, 106.0, 5.0, 10.0),
            RegressionStatus::WithinThreshold
        );
        assert_eq!(
            compare(100.0, 111.0, 5.0, 10.0),
            RegressionStatus::Regression
        );
    }

    #[test]
    fn small_absolute_startup_changes_do_not_trigger_large_percentage_regressions() {
        assert_eq!(
            compare(1.0, 1.2, 5.0, DEFAULT_MIN_ABSOLUTE_REGRESSION_MS),
            RegressionStatus::WithinThreshold
        );
    }

    #[test]
    fn meaningful_improvements_use_the_same_absolute_and_relative_floors() {
        assert_eq!(
            compare(100.0, 80.0, 5.0, 10.0),
            RegressionStatus::Improvement
        );
        assert_eq!(
            compare(100.0, 95.0, 0.0, 10.0),
            RegressionStatus::WithinThreshold
        );
    }
}
