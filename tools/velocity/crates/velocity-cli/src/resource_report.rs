use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use velocity_core::{ResourceBenchmarkReport, ResourceWorkload};

#[cfg(target_os = "macos")]
pub(crate) fn print_report_summary(report: &ResourceBenchmarkReport) {
    use crate::terminal::{print_markdown, print_table};
    print_markdown(
        "## Resource benchmark results\n\nValues are medians/means of per-iteration resource summaries. Lower usage is descriptive; incomplete process coverage limits comparisons.",
    );
    let rows: Vec<Vec<String>> = report
        .summaries
        .iter()
        .map(|summary| {
            let stats = &summary.across_iteration_statistics;
            let cpu = stats.get("cpu_mean_percent");
            let footprint = stats.get("physical_footprint_mean_bytes");
            let footprint_final = stats.get("physical_footprint_final_bytes");
            let resident = stats.get("resident_mean_bytes");
            let peak_processes = stats.get("peak_process_count");
            vec![
                summary.browser.name.clone(),
                summary
                    .tab_count
                    .map_or("—".to_owned(), |tabs| tabs.to_string()),
                format_stat(cpu, "%"),
                format_stat(footprint, "MiB"),
                format_stat(footprint_final, "MiB"),
                format_stat(resident, "MiB"),
                peak_processes
                    .map_or_else(|| "N/A".to_owned(), |stat| format!("{:.0}", stat.maximum)),
                format!(
                    "{}/{}",
                    summary.successful_iterations,
                    summary.successful_iterations + summary.failed_iterations
                ),
                format!(
                    "{}/{} ({} failed; {} CPU, {} footprint, {} RSS)",
                    summary.sampling_coverage.valid_samples,
                    summary.sampling_coverage.attempted_samples,
                    summary.sampling_coverage.failed_samples,
                    summary.sampling_coverage.cpu_samples,
                    summary.sampling_coverage.physical_footprint_samples,
                    summary.sampling_coverage.resident_samples
                ),
                summary.attribution.status.label().to_owned(),
                summary.comparison_status.label().to_owned(),
            ]
        })
        .collect();
    print_table(
        &[
            "Browser",
            "Tabs",
            "CPU mean",
            "Footprint mean",
            "Footprint final",
            "Resident RSS mean",
            "Peak processes",
            "Runs ok/total",
            "Samples",
            "Attribution",
            "Comparability",
        ],
        rows,
        &[2, 3, 4, 5, 6],
    );
    if !report.memory_scaling.is_empty() {
        print_markdown(
            "### Memory scaling\n\nIncremental values describe differences between adjacent tab-count scenarios. They are not isolated per-tab costs.",
        );
        let rows: Vec<Vec<String>> = report
            .memory_scaling
            .iter()
            .map(|scenario| {
                vec![
                    scenario.browser.name.clone(),
                    scenario.tab_count.to_string(),
                    format_mib(scenario.mean_footprint_bytes),
                    format_mib(scenario.median_footprint_bytes),
                    format_mib(
                        scenario
                            .peak_observed_footprint_bytes
                            .map(|value| value as f64),
                    ),
                    format_mib(scenario.incremental_footprint_bytes),
                    format_mib(scenario.marginal_footprint_per_added_tab_bytes),
                    scenario.measurement_status.label().to_owned(),
                    scenario
                        .incremental_status
                        .map_or_else(|| "—".to_owned(), |status| status.label().to_owned()),
                ]
            })
            .collect();
        print_table(
            &[
                "Browser",
                "Tabs",
                "Mean",
                "Median",
                "Peak observed",
                "Increment status",
                "Marginal / added tab",
                "Measurement",
                "Increment",
            ],
            rows,
            &[2, 3, 4, 5, 6],
        );
    }
    for summary in &report.summaries {
        for reason in &summary.comparison_reasons {
            println!("Comparison — {}: {reason}", summary.browser.name);
        }
        for warning in &summary.coverage_warnings {
            println!("Warning — {}: {warning}", summary.browser.name);
        }
        println!(
            "Attribution — {} ({}): {} verified process(es); {} excluded candidate observation(s); {} uncertain candidate observation(s); {} exited process observation(s); {} identity verification failure(s)",
            summary.browser.name,
            summary.attribution.status.label(),
            summary.attribution.verified_processes,
            summary.attribution.excluded_process_candidates,
            summary.attribution.uncertain_processes,
            summary.attribution.exited_processes,
            summary.attribution.identity_verification_failures,
        );
        if let Some(scope) = &summary.attribution.registry_scope {
            println!(
                "  Process registry: {} ({}) — {} registered, {} independently verified; snapshot {}",
                scope,
                summary
                    .attribution
                    .registry_source
                    .as_deref()
                    .unwrap_or("source unavailable"),
                summary.attribution.registered_processes,
                summary
                    .attribution
                    .independently_verified_registered_processes,
                if summary.attribution.registry_snapshot_complete {
                    "complete within scope"
                } else {
                    "incomplete"
                }
            );
        }
        for (evidence, count) in &summary.attribution.evidence {
            println!("  Evidence {:?}: {count} process observation(s)", evidence);
        }
        for (reason, count) in &summary.attribution.exclusion_reasons {
            println!("  Exclusion {:?}: {count} candidate observation(s)", reason);
        }
    }
    for assessment in &report.comparison_assessments {
        println!(
            "Scenario {:?}: {}",
            assessment.tab_count,
            assessment.status.label()
        );
        for reason in &assessment.reasons {
            println!("  Reason: {reason}");
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn format_stat(
    statistics: Option<&velocity_core::ResourceMetricSummary>,
    unit: &str,
) -> String {
    let Some(statistics) = statistics else {
        return "N/A".to_owned();
    };
    if unit == "MiB" {
        format!("{:.1} MiB", statistics.mean / (1024.0 * 1024.0))
    } else {
        format!("{:.1}%", statistics.mean)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn format_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.1}%"))
}

#[cfg(target_os = "macos")]
pub(crate) fn format_mib(value: Option<f64>) -> String {
    value.map_or_else(
        || "N/A".to_owned(),
        |value| format!("{:.1} MiB", value / (1024.0 * 1024.0)),
    )
}

#[cfg(target_os = "macos")]
pub(crate) fn title(workload: ResourceWorkload) -> &'static str {
    match workload {
        ResourceWorkload::Idle => "Idle",
        ResourceWorkload::Page => "Single-Page",
        ResourceWorkload::Tabs => "Multi-Tab",
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn write_reports(
    directory: &Path,
    run_id: &str,
    report: &ResourceBenchmarkReport,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create report directory {}: {error}",
            directory.display()
        )
    })?;
    let stem = format!("{}-{}", report.benchmark, run_id);
    let json_path = directory.join(format!("{stem}.json"));
    let html_path = directory.join(format!("{stem}.html"));
    write_unique(&json_path, |file| {
        serde_json::to_writer_pretty(&mut *file, report).map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())
    })?;
    let html = render_html(report);
    write_unique(&html_path, |file| {
        file.write_all(html.as_bytes())
            .map_err(|error| error.to_string())
    })?;
    Ok((json_path, html_path))
}

#[cfg(target_os = "macos")]
fn write_unique(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> Result<(), String>,
) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create report {}: {error}", path.display()))?;
    write(&mut file).map_err(|error| format!("cannot write report {}: {error}", path.display()))
}

#[cfg(target_os = "macos")]
pub(crate) fn render_html(report: &ResourceBenchmarkReport) -> String {
    let report_title = match report.benchmark.as_str() {
        "idle" => "Idle",
        "page" => "Single-Page",
        "tabs" => "Multi-Tab",
        _ => "Resource",
    };
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Velocity {} resource report</title><style>:root{{--ink:#1f2933;--muted:#52606d;--line:#bcccdc;--surface:#f0f4f8;--series-1:#176b87;--series-2:#8c4a6e;--series-3:#507d2a;--series-4:#8a5b00;--series-5:#51429c}}body{{font:16px/1.5 system-ui,sans-serif;color:var(--ink);max-width:1120px;margin:2rem auto;padding:0 1rem}}h1,h2{{line-height:1.2}}.muted{{color:var(--muted)}}.table-wrap{{overflow-x:auto}}table{{border-collapse:collapse;width:100%;margin:1rem 0 2rem}}th,td{{border-bottom:1px solid var(--line);padding:.6rem;text-align:left}}th{{background:var(--surface)}}.warning{{background:var(--surface);border-left:4px solid var(--series-4);padding:.65rem 1rem;margin:.5rem 0}}svg{{width:100%;height:auto;border:1px solid var(--line);background:white}}.axis{{stroke:var(--line);stroke-width:1}}.series-0{{stroke:var(--series-1)}}.series-1{{stroke:var(--series-2)}}.series-2{{stroke:var(--series-3)}}.series-3{{stroke:var(--series-4)}}.series-4{{stroke:var(--series-5)}}.trace{{fill:none;stroke-width:1.5;opacity:.75}}.point{{stroke-width:1}}.point.series-0{{fill:var(--series-1)}}.point.series-1{{fill:var(--series-2)}}.point.series-2{{fill:var(--series-3)}}.point.series-3{{fill:var(--series-4)}}.point.series-4{{fill:var(--series-5)}}code{{overflow-wrap:anywhere}}</style></head><body><h1>Velocity — {} resource benchmark</h1><p class=\"muted\">Benchmark: <code>{}</code> · Methodology: <code>{}</code> · OS: {} · Architecture: {} · Fixture: {}</p>",
        report.benchmark,
        report_title,
        escape_html(&report.benchmark),
        escape_html(&report.measurement_methodology_version),
        escape_html(&report.operating_system),
        escape_html(&report.architecture),
        report
            .fixture
            .as_ref()
            .map(|fixture| format!("{} v{}", fixture.kind.name(), escape_html(&fixture.version)))
            .unwrap_or_else(|| "about:blank".to_owned())
    );
    html.push_str("<h2>Comparison classification</h2>");
    for assessment in &report.comparison_assessments {
        html.push_str(&format!(
            "<p><strong>{}</strong> · {} browser(s): {}</p>",
            assessment.tab_count.map_or_else(
                || "single workload".to_owned(),
                |count| format!("{count} tabs")
            ),
            assessment.browser_count,
            assessment.status.label()
        ));
        for reason in &assessment.reasons {
            html.push_str(&format!("<p class=\"warning\">{}</p>", escape_html(reason)));
        }
    }
    html.push_str("<h2>Per-browser resource summaries</h2><div class=\"table-wrap\"><table><thead><tr><th>Browser</th><th>Tabs</th><th>CPU mean across runs</th><th>CPU peak across runs</th><th>Footprint mean across runs</th><th>Footprint peak across runs</th><th>Footprint final across runs</th><th>Resident RSS mean across runs</th><th>Runs ok / total</th><th>Sampling (valid / attempted; failed)</th><th>Metric samples (CPU / footprint / RSS)</th><th>Attribution</th><th>Comparability</th></tr></thead><tbody>");
    for summary in &report.summaries {
        let stats = &summary.across_iteration_statistics;
        html.push_str(&format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td><td>{}/{}/{} failed</td><td>{} / {} / {}</td><td>{} (verified {})</td><td>{}</td></tr>", escape_html(&summary.browser.name), summary.tab_count.map_or("—".to_owned(), |value| value.to_string()), html_stat(stats.get("cpu_mean_percent"), false), html_stat(stats.get("cpu_peak_percent"), false), html_stat(stats.get("physical_footprint_mean_bytes"), true), html_stat(stats.get("physical_footprint_peak_bytes"), true), html_stat(stats.get("physical_footprint_final_bytes"), true), html_stat(stats.get("resident_mean_bytes"), true), summary.successful_iterations, summary.successful_iterations + summary.failed_iterations, summary.sampling_coverage.valid_samples, summary.sampling_coverage.attempted_samples, summary.sampling_coverage.failed_samples, summary.sampling_coverage.cpu_samples, summary.sampling_coverage.physical_footprint_samples, summary.sampling_coverage.resident_samples, summary.attribution.status.label(), summary.attribution.verified_processes, summary.comparison_status.label()));
        for reason in &summary.comparison_reasons {
            html.push_str(&format!(
                "<tr><td colspan=\"13\" class=\"warning\">{}</td></tr>",
                escape_html(reason)
            ));
        }
        html.push_str(&format!(
            "<tr><td colspan=\"13\"><details><summary>Process attribution diagnostics — {}</summary><p>Excluded candidate observations: {} · uncertain candidate observations: {} · exited process observations: {} · identity verification failures: {}</p>",
            escape_html(&summary.browser.name),
            summary.attribution.excluded_process_candidates,
            summary.attribution.uncertain_processes,
            summary.attribution.exited_processes,
            summary.attribution.identity_verification_failures,
        ));
        if summary.attribution.evidence.is_empty() {
            html.push_str("<p>No attribution evidence was collected.</p>");
        } else {
            html.push_str("<ul>");
            for (evidence, count) in &summary.attribution.evidence {
                html.push_str(&format!(
                    "<li>{:?}: {} process observation(s)</li>",
                    evidence, count
                ));
            }
            html.push_str("</ul>");
        }
        if !summary.attribution.exclusion_reasons.is_empty() {
            html.push_str("<p>Exclusion reasons:</p><ul>");
            for (reason, count) in &summary.attribution.exclusion_reasons {
                html.push_str(&format!(
                    "<li>{:?}: {} candidate observation(s)</li>",
                    reason, count
                ));
            }
            html.push_str("</ul>");
        }
        if let Some(scope) = &summary.attribution.registry_scope {
            html.push_str(&format!(
                "<p>Process registry: {} · source: {} · {} registered · {} independently verified · snapshot {}</p>",
                escape_html(scope),
                escape_html(summary.attribution.registry_source.as_deref().unwrap_or("unavailable")),
                summary.attribution.registered_processes,
                summary.attribution.independently_verified_registered_processes,
                if summary.attribution.registry_snapshot_complete { "complete within scope" } else { "incomplete" }
            ));
        }
        if !summary.attribution.lifecycle_events.is_empty() {
            html.push_str("<p>Process lifecycle changes:</p><ul>");
            for event in &summary.attribution.lifecycle_events {
                html.push_str(&format!(
                    "<li>{:?}: PID {} · identity {}.{} · role {}</li>",
                    event.change,
                    event.pid,
                    event.start_time.seconds,
                    event.start_time.microseconds,
                    event.role.as_deref().unwrap_or("unknown")
                ));
            }
            html.push_str("</ul>");
        }
        let mut roles =
            BTreeMap::<String, BTreeSet<(u32, velocity_metrics::ProcessStartTime)>>::new();
        for iteration in report.iterations.iter().filter(|iteration| {
            iteration.browser.installation_id == summary.browser.installation_id
                && iteration.tab_count == summary.tab_count
        }) {
            if let Some(resource) = &iteration.resource {
                for sample in &resource.samples {
                    for process in &sample.processes {
                        if let Some(role) = &process.role {
                            roles
                                .entry(role.clone())
                                .or_default()
                                .insert((process.pid, process.start_time));
                        }
                    }
                }
            }
        }
        if !roles.is_empty() {
            html.push_str("<p>Observed process roles:</p><ul>");
            for (role, identities) in roles {
                html.push_str(&format!(
                    "<li>{}: {} distinct verified process identity/identities</li>",
                    escape_html(&role),
                    identities.len()
                ));
            }
            html.push_str("</ul>");
        }
        html.push_str("</details></td></tr>");
    }
    html.push_str("</tbody></table></div><h2>CPU over time</h2><p class=\"muted\">CPU percent uses one fully utilized logical core as 100%. The x-axis is elapsed time within each iteration. Separate runs are overlaid and did not occur simultaneously.</p>");
    html.push_str(&render_chart(report, true));
    html.push_str("<h2>Physical footprint over time</h2><p class=\"muted\">Bytes are converted to MiB. The y-axis starts at zero. Separate runs are overlaid and did not occur simultaneously.</p>");
    html.push_str(&render_chart(report, false));
    if report.benchmark == ResourceWorkload::Tabs.id() {
        html.push_str("<h2>Memory by tab count</h2><p class=\"muted\">Each point is the mean of per-run mean physical footprint for that browser and tab count. The y-axis starts at zero; repeated runs were measured sequentially.</p>");
        html.push_str(&render_tab_memory_chart(report));
        html.push_str(&render_memory_scaling_table(report));
    }
    html.push_str("<h2>Failures and coverage</h2>");
    let mut warning_count = 0;
    for summary in &report.summaries {
        for warning in &summary.coverage_warnings {
            warning_count += 1;
            html.push_str(&format!(
                "<p class=\"warning\"><strong>{}</strong>: {}</p>",
                escape_html(&summary.browser.name),
                escape_html(warning)
            ));
        }
    }
    for iteration in &report.iterations {
        if let Some(failure) = &iteration.failure {
            warning_count += 1;
            html.push_str(&format!(
                "<p class=\"warning\"><strong>{} {}</strong>: {}</p>",
                escape_html(&iteration.browser.name),
                if iteration.warmup {
                    "warm-up"
                } else {
                    "iteration"
                },
                escape_html(&failure.message)
            ));
        }
        if let Some(detail) = &iteration.cleanup.detail {
            warning_count += 1;
            html.push_str(&format!(
                "<p class=\"warning\"><strong>{} cleanup</strong>: {}</p>",
                escape_html(&iteration.browser.name),
                escape_html(detail)
            ));
        }
    }
    if warning_count == 0 {
        html.push_str("<p>No coverage or cleanup warnings were recorded.</p>");
    }
    html.push_str(&format!("<h2>Configuration and limitations</h2><p>Runs: {} · warm-ups: {} · duration: {} ms · settling: {} ms · sample interval: {} ms · navigation timeout: {} ms · readiness timeout: {} ms</p><ul>", report.configuration.runs, report.configuration.warmup_iterations, report.configuration.measurement_duration_ms, report.configuration.settle_period_ms, report.configuration.sampling_interval_ms, report.configuration.navigation_timeout_ms, report.configuration.readiness_timeout_ms));
    for limitation in &report.limitations {
        html.push_str(&format!("<li>{}</li>", escape_html(limitation)));
    }
    html.push_str("</ul><p class=\"muted\">Raw measurements and per-sample timestamps are preserved in the companion JSON report. Memory footprint aggregates verified processes; summed resident memory may double-count shared pages.</p></body></html>");
    html
}

#[cfg(target_os = "macos")]
fn render_memory_scaling_table(report: &ResourceBenchmarkReport) -> String {
    if report.memory_scaling.is_empty() {
        return "<p>No measured memory scaling scenarios are available.</p>".to_owned();
    }
    let mut html = String::from(
        "<h3>Memory scaling values</h3><p class=\"muted\">Incremental and marginal values are descriptive differences between measured scenarios. They are not isolated per-tab memory costs and are inconclusive when process attribution is incomplete.</p><div class=\"table-wrap\"><table><thead><tr><th>Browser</th><th>Tabs</th><th>Mean footprint</th><th>Median footprint</th><th>Peak observed</th><th>Increment from prior scenario</th><th>Marginal per added tab</th><th>Measurement status</th><th>Increment status</th></tr></thead><tbody>",
    );
    for scenario in &report.memory_scaling {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&scenario.browser.name),
            scenario.tab_count,
            html_bytes(scenario.mean_footprint_bytes),
            html_bytes(scenario.median_footprint_bytes),
            html_bytes(scenario.peak_observed_footprint_bytes.map(|value| value as f64)),
            html_bytes(scenario.incremental_footprint_bytes),
            html_bytes(scenario.marginal_footprint_per_added_tab_bytes),
            scenario.measurement_status.label(),
            scenario.incremental_status.map_or("—", |status| status.label())
        ));
        for reason in scenario
            .measurement_reasons
            .iter()
            .chain(&scenario.incremental_reasons)
        {
            html.push_str(&format!(
                "<tr><td colspan=\"9\" class=\"warning\">{} — {}</td></tr>",
                escape_html(&scenario.browser.name),
                escape_html(reason)
            ));
        }
    }
    html.push_str("</tbody></table></div>");
    html
}

#[cfg(target_os = "macos")]
fn render_tab_memory_chart(report: &ResourceBenchmarkReport) -> String {
    const WIDTH: f64 = 900.0;
    const HEIGHT: f64 = 300.0;
    const LEFT: f64 = 72.0;
    const RIGHT: f64 = 20.0;
    const TOP: f64 = 18.0;
    const BOTTOM: f64 = 42.0;

    let mut series = BTreeMap::<String, (String, Vec<(usize, f64)>)>::new();
    for summary in &report.summaries {
        let (Some(tab_count), Some(statistics)) = (
            summary.tab_count,
            summary
                .across_iteration_statistics
                .get("physical_footprint_mean_bytes"),
        ) else {
            continue;
        };
        let mib = statistics.mean / 1_048_576.0;
        if !mib.is_finite() || mib < 0.0 {
            continue;
        }
        series
            .entry(summary.browser.installation_id.clone())
            .or_insert_with(|| (summary.browser.name.clone(), Vec::new()))
            .1
            .push((tab_count, mib));
    }

    let tab_counts = series
        .values()
        .flat_map(|(_, points)| points.iter().map(|(tabs, _)| *tabs))
        .collect::<BTreeSet<_>>();
    let maximum_tabs = tab_counts.iter().next_back().copied().unwrap_or(1).max(1) as f64;
    let maximum_memory = series
        .values()
        .flat_map(|(_, points)| points.iter().map(|(_, memory)| *memory))
        .fold(0.0_f64, f64::max);
    let memory_axis = (maximum_memory / 100.0).ceil().max(1.0) * 100.0;
    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let mut svg = format!(
        "<svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=\"img\" aria-label=\"Mean physical footprint by browser tab count\"><line class=\"axis\" x1=\"{LEFT}\" y1=\"{TOP}\" x2=\"{LEFT}\" y2=\"{}\"/><line class=\"axis\" x1=\"{LEFT}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        HEIGHT - BOTTOM,
        HEIGHT - BOTTOM,
        WIDTH - RIGHT,
        HEIGHT - BOTTOM
    );
    for tick in 0..=4 {
        let value = memory_axis * tick as f64 / 4.0;
        let y = TOP + (1.0 - value / memory_axis) * plot_height;
        svg.push_str(&format!(
            "<line class=\"axis\" x1=\"{LEFT}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\"/><text x=\"6\" y=\"{:.1}\">{value:.0} MiB</text>",
            WIDTH - RIGHT,
            y + 4.0
        ));
    }
    let mut x_ticks = BTreeSet::from([0]);
    x_ticks.extend(tab_counts.iter().copied());
    for tick in x_ticks {
        let x = LEFT + tick as f64 / maximum_tabs * plot_width;
        svg.push_str(&format!(
            "<line class=\"axis\" x1=\"{x:.1}\" y1=\"{}\" x2=\"{x:.1}\" y2=\"{}\"/><text x=\"{:.1}\" y=\"{}\">{tick}</text>",
            HEIGHT - BOTTOM,
            HEIGHT - BOTTOM + 4.0,
            x - 3.0,
            HEIGHT - 10.0
        ));
    }

    let mut legend = Vec::new();
    for (series_index, (name, points)) in series.values_mut().enumerate() {
        points.sort_by_key(|(tabs, _)| *tabs);
        let class = series_index % 5;
        let coordinates = points
            .iter()
            .map(|(tabs, mib)| {
                let x = LEFT + *tabs as f64 / maximum_tabs * plot_width;
                let y = TOP + (1.0 - (*mib / memory_axis).clamp(0.0, 1.0)) * plot_height;
                (x, y, *tabs, *mib)
            })
            .collect::<Vec<_>>();
        if coordinates.len() > 1 {
            let points = coordinates
                .iter()
                .map(|(x, y, _, _)| format!("{x:.1},{y:.1}"))
                .collect::<Vec<_>>()
                .join(" ");
            svg.push_str(&format!(
                "<polyline class=\"trace series-{class}\" points=\"{points}\"><title>{}</title></polyline>",
                escape_html(name)
            ));
        }
        for (x, y, tabs, mib) in coordinates {
            let label = format!("{name} — {tabs} tab(s): {mib:.1} MiB");
            svg.push_str(&format!(
                "<circle class=\"point series-{class}\" cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"4\"><title>{}</title></circle>",
                escape_html(&label)
            ));
        }
        legend.push(format!(
            "<span class=\"series-{class}\">{}</span>",
            escape_html(name)
        ));
    }
    if series.is_empty() {
        svg.push_str(&format!(
            "<text x=\"{LEFT}\" y=\"{}\">No tab-count measurements available</text>",
            TOP + 30.0
        ));
    }
    svg.push_str("</svg><p class=\"muted\">Browser tab count</p>");
    if !legend.is_empty() {
        svg.push_str(&format!("<p class=\"muted\">{}</p>", legend.join(" · ")));
    }
    svg
}

#[cfg(target_os = "macos")]
fn render_chart(report: &ResourceBenchmarkReport, cpu: bool) -> String {
    const WIDTH: f64 = 900.0;
    const HEIGHT: f64 = 260.0;
    const LEFT: f64 = 58.0;
    const RIGHT: f64 = 15.0;
    const TOP: f64 = 15.0;
    const BOTTOM: f64 = 34.0;
    let duration = report.configuration.measurement_duration_ms.max(1) as f64;
    let mut maximum: f64 = 1.0;
    for iteration in report
        .iterations
        .iter()
        .filter(|iteration| !iteration.warmup)
    {
        if let Some(resource) = &iteration.resource {
            for sample in &resource.samples {
                let value = if cpu {
                    sample.cpu_percent
                } else {
                    sample
                        .physical_footprint_bytes
                        .map(|value| value as f64 / 1_048_576.0)
                };
                if let Some(value) = value.filter(|value| value.is_finite()) {
                    maximum = maximum.max(value);
                }
            }
        }
    }
    let max_axis = if cpu {
        (maximum / 25.0).ceil().max(1.0) * 25.0
    } else {
        (maximum / 100.0).ceil().max(1.0) * 100.0
    };
    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let mut svg = format!(
        "<svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=\"img\" aria-label=\"{} resource samples\"><line class=\"axis\" x1=\"{LEFT}\" y1=\"{TOP}\" x2=\"{LEFT}\" y2=\"{}\"/><line class=\"axis\" x1=\"{LEFT}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/><text x=\"4\" y=\"18\">{:.0}</text><text x=\"4\" y=\"{}\">0</text><text x=\"{LEFT}\" y=\"{}\">0 s</text><text x=\"{}\" y=\"{}\">{} s</text>",
        if cpu {
            "CPU percent"
        } else {
            "Physical footprint in MiB"
        },
        HEIGHT - BOTTOM,
        HEIGHT - BOTTOM,
        WIDTH - RIGHT,
        HEIGHT - BOTTOM,
        max_axis,
        HEIGHT - BOTTOM + 4.0,
        HEIGHT - 7.0,
        WIDTH - RIGHT - 35.0,
        HEIGHT - 7.0,
        duration / 1000.0
    );
    let mut series_index = 0;
    let mut legend = Vec::new();
    for summary in &report.summaries {
        for iteration in report.iterations.iter().filter(|iteration| {
            !iteration.warmup
                && iteration.resource.is_some()
                && iteration.browser.installation_id == summary.browser.installation_id
                && iteration.tab_count == summary.tab_count
        }) {
            let Some(resource) = iteration.resource.as_ref() else {
                continue;
            };
            let samples = &resource.samples;
            if samples.is_empty() {
                continue;
            }
            let stride = samples.len().div_ceil(500).max(1);
            let points = samples
                .iter()
                .step_by(stride)
                .filter_map(|sample| {
                    let value = if cpu {
                        sample.cpu_percent
                    } else {
                        sample
                            .physical_footprint_bytes
                            .map(|value| value as f64 / 1_048_576.0)
                    }?;
                    let x = LEFT + (sample.elapsed_ms / duration).clamp(0.0, 1.0) * plot_width;
                    let y = TOP + (1.0 - value.clamp(0.0, max_axis) / max_axis) * plot_height;
                    Some(format!("{x:.1},{y:.1}"))
                })
                .collect::<Vec<_>>()
                .join(" ");
            let class = series_index % 5;
            let label = format!(
                "{}{} run {}",
                summary.browser.name,
                summary
                    .tab_count
                    .map(|tabs| format!(" ({tabs} tabs)"))
                    .unwrap_or_default(),
                iteration.iteration
            );
            svg.push_str(&format!("<polyline class=\"trace series-{class}\" points=\"{points}\"><title>{}</title></polyline>", escape_html(&label)));
            legend.push(format!(
                "<span class=\"series-{class}\">{}</span>",
                escape_html(&label)
            ));
            series_index += 1;
        }
    }
    if series_index == 0 {
        svg.push_str(&format!(
            "<text x=\"{LEFT}\" y=\"{}\">No available samples</text>",
            TOP + 30.0
        ));
    }
    svg.push_str("</svg>");
    if !legend.is_empty() {
        svg.push_str(&format!("<p class=\"muted\">{}</p>", legend.join(" · ")));
    }
    svg
}

#[cfg(target_os = "macos")]
fn html_stat(statistics: Option<&velocity_core::ResourceMetricSummary>, bytes: bool) -> String {
    let Some(statistics) = statistics else {
        return "N/A".to_owned();
    };
    if bytes {
        format!("{:.1} MiB", statistics.mean / 1_048_576.0)
    } else {
        format!("{:.1}%", statistics.mean)
    }
}

#[cfg(target_os = "macos")]
fn html_bytes(value: Option<f64>) -> String {
    value.map_or_else(
        || "N/A".to_owned(),
        |bytes| format!("{:.1} MiB", bytes / 1_048_576.0),
    )
}

#[cfg(target_os = "macos")]
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(target_os = "macos")]
pub(crate) fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
