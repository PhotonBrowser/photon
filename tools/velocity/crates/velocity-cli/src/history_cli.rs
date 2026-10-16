use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    history::{
        HistoryMetric, HistoryRecord, ImportMetadata, default_storage_directory, delete_baseline,
        export_report, get_baseline, get_record, import_report, list_baselines, list_records,
        set_baseline, write_new_artifact,
    },
    terminal::{print_markdown, print_table},
};

pub(crate) fn run_history(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let Some(subcommand) = arguments.next() else {
        print_history_help();
        return Ok(());
    };
    if matches!(subcommand.as_str(), "--help" | "-h" | "help") {
        print_history_help();
        return Ok(());
    }
    let rest = arguments.collect::<Vec<_>>();
    match subcommand.as_str() {
        "import" => import_command(rest),
        "list" => list_command(rest),
        "show" => show_command(rest),
        "report" => report_command(rest),
        "export" => export_command(rest),
        other => Err(format!(
            "unknown history command '{other}'; available commands: import, list, show, report, export"
        )),
    }
}

pub(crate) fn run_baseline(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let Some(subcommand) = arguments.next() else {
        print_baseline_help();
        return Ok(());
    };
    if matches!(subcommand.as_str(), "--help" | "-h" | "help") {
        print_baseline_help();
        return Ok(());
    }
    let rest = arguments.collect::<Vec<_>>();
    match subcommand.as_str() {
        "set" => baseline_set(rest),
        "list" => baseline_list(rest),
        "show" => baseline_show(rest),
        "delete" => baseline_delete(rest),
        other => Err(format!(
            "unknown baseline command '{other}'; available commands: set, list, show, delete"
        )),
    }
}

pub(crate) fn record_after_success(path: &Path) {
    match import_report(
        path,
        &default_storage_directory(),
        &ImportMetadata {
            capture_hardware: true,
            ..ImportMetadata::default()
        },
    ) {
        Ok(outcome) if !outcome.record_ids.is_empty() => {
            println!("History records: {}", outcome.record_ids.join(", "));
        }
        Ok(outcome) if !outcome.duplicate_record_ids.is_empty() => {
            println!(
                "History already contains: {}",
                outcome.duplicate_record_ids.join(", ")
            );
        }
        Ok(_) => {}
        Err(error) => eprintln!("History storage warning: {error}"),
    }
}

fn import_command(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let report = args.next().ok_or("history import requires <report.json>")?;
    let mut directory = default_storage_directory();
    let mut metadata = ImportMetadata::default();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            "--label" if metadata.label.is_none() => {
                metadata.label = Some(validate_user_metadata(
                    &required(&mut args, &option)?,
                    "label",
                )?);
            }
            "--commit" if metadata.user_commit.is_none() => {
                metadata.user_commit = Some(validate_user_metadata(
                    &required(&mut args, &option)?,
                    "commit",
                )?);
            }
            other => return Err(format!("unexpected history import option '{other}'")),
        }
    }
    let outcome = import_report(Path::new(&report), &directory, &metadata)?;
    for id in &outcome.record_ids {
        println!("Imported {id}");
    }
    for id in &outcome.duplicate_record_ids {
        println!("Already imported {id}");
    }
    if outcome.record_ids.is_empty() && outcome.duplicate_record_ids.is_empty() {
        return Err("report contained no importable browser results".to_owned());
    }
    Ok(())
}

fn list_command(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let mut directory = default_storage_directory();
    let mut browser = None;
    let mut benchmark = None;
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            "--browser" if browser.is_none() => browser = Some(required(&mut args, &option)?),
            "--benchmark" if benchmark.is_none() => benchmark = Some(required(&mut args, &option)?),
            other => return Err(format!("unexpected history list option '{other}'")),
        }
    }
    let records = list_records(&directory, browser.as_deref(), benchmark.as_deref())?;
    print_markdown("## Velocity — Performance History");
    if records.is_empty() {
        println!("No matching historical benchmark records.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = records
        .iter()
        .map(|record| {
            let build = build_identifier(record);
            let metric = primary_metric(record);
            vec![
                format_date(record.timestamp_unix_ms),
                truncate(&build, 18),
                format!(
                    "{} {} {}",
                    record.browser_name,
                    record.benchmark,
                    optional(&record.benchmark_version)
                ),
                metric.map_or_else(|| "N/A".to_owned(), format_metric),
            ]
        })
        .collect();
    print_table(
        &["Date", "Build / label", "Benchmark", "Median"],
        rows,
        &[3],
    );
    println!("{} record(s)", records.len());
    Ok(())
}

fn show_command(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let id = args.next().ok_or("history show requires <record-id>")?;
    let mut directory = default_storage_directory();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            other => return Err(format!("unexpected history show option '{other}'")),
        }
    }
    let record = get_record(&directory, &id)?;
    print_markdown(&format!(
        "## {} — {}",
        record.browser_name, record.benchmark
    ));
    let rows = vec![
        vec!["Record ID".to_owned(), record.record_id.clone()],
        vec![
            "Benchmark version".to_owned(),
            optional(&record.benchmark_version),
        ],
        vec!["Browser ID".to_owned(), record.browser_id.clone()],
        vec!["Installation".to_owned(), record.installation_id.clone()],
        vec![
            "Browser version".to_owned(),
            optional(&record.browser_version),
        ],
        vec![
            "Executable".to_owned(),
            display_path_option(record.executable_path.as_deref()),
        ],
        vec![
            "Timestamp".to_owned(),
            format_timestamp(record.timestamp_unix_ms),
        ],
        vec![
            "Operating system".to_owned(),
            record.operating_system.clone(),
        ],
        vec!["Architecture".to_owned(), record.architecture.clone()],
        vec![
            "Hardware".to_owned(),
            format_hardware(record.hardware.as_ref()),
        ],
        vec![
            "Hardware source".to_owned(),
            record.hardware_observation.clone(),
        ],
        vec![
            "Repository at import".to_owned(),
            format_repository(record.repository_at_import.as_ref()),
        ],
        vec![
            "Executable build revision".to_owned(),
            optional(&record.executable_build.revision),
        ],
        vec![
            "Build identity".to_owned(),
            record.executable_build.provenance.clone(),
        ],
        vec![
            "Build type".to_owned(),
            optional(&record.executable_build.build_type),
        ],
        vec![
            "Build timestamp".to_owned(),
            record
                .executable_build
                .build_timestamp_unix_ms
                .map_or_else(|| "unknown".to_owned(), format_timestamp),
        ],
        vec!["Label".to_owned(), optional(&record.label)],
        vec![
            "User supplied commit".to_owned(),
            optional(&record.user_supplied_commit),
        ],
        vec![
            "Comparability".to_owned(),
            record.comparability.status.clone(),
        ],
        vec![
            "Original report SHA-256".to_owned(),
            record.source_sha256.clone(),
        ],
        vec!["Archived report".to_owned(), record.archived_report.clone()],
    ];
    print_table(&["Field", "Value"], rows, &[]);
    let metrics: Vec<Vec<String>> = record
        .summary
        .iter()
        .map(|metric| {
            vec![
                metric.name.clone(),
                format_metric(metric),
                metric.sample_count.to_string(),
                metric
                    .standard_deviation
                    .map_or_else(|| "N/A".to_owned(), |v| format!("{v:.3} {}", metric.unit)),
            ]
        })
        .collect();
    print_table(
        &["Metric", "Median", "Samples", "Std dev"],
        metrics,
        &[1, 2, 3],
    );
    if !record.comparability.warnings.is_empty() {
        for warning in record.comparability.warnings {
            println!("Warning: {warning}");
        }
    }
    Ok(())
}

fn export_command(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let id = args.next().ok_or("history export requires <record-id>")?;
    let mut directory = default_storage_directory();
    let mut output = None;
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            "--output" if output.is_none() => {
                output = Some(PathBuf::from(required(&mut args, &option)?))
            }
            other => return Err(format!("unexpected history export option '{other}'")),
        }
    }
    let output = output.ok_or("history export requires --output <report.json>")?;
    export_report(&directory, &id, &output)?;
    println!("Exported original report to {}", output.display());
    Ok(())
}

fn report_command(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let mut directory = default_storage_directory();
    let mut browser = None;
    let mut benchmark = None;
    let mut baseline_name = None;
    let mut output = None;
    let mut threshold = 5.0_f64;
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            "--browser" if browser.is_none() => browser = Some(required(&mut args, &option)?),
            "--benchmark" if benchmark.is_none() => benchmark = Some(required(&mut args, &option)?),
            "--baseline" if baseline_name.is_none() => {
                baseline_name = Some(required(&mut args, &option)?)
            }
            "--threshold" => {
                let value = required(&mut args, &option)?;
                threshold = value
                    .parse()
                    .map_err(|_| format!("invalid threshold '{value}'"))?;
                if !threshold.is_finite() || threshold < 0.0 {
                    return Err("--threshold must be finite and non-negative".to_owned());
                }
            }
            "--output" if output.is_none() => {
                output = Some(PathBuf::from(required(&mut args, &option)?))
            }
            other => return Err(format!("unexpected history report option '{other}'")),
        }
    }
    let records = list_records(&directory, browser.as_deref(), benchmark.as_deref())?;
    let baseline = baseline_name
        .as_deref()
        .map(|name| get_baseline(&directory, name))
        .transpose()?
        .map(|baseline| get_record(&directory, &baseline.record_id))
        .transpose()?;
    let output = output.unwrap_or_else(|| {
        directory
            .join("reports")
            .join(format!("performance-history-{}.html", now_suffix()))
    });
    let html = render_history_html(&records, baseline.as_ref(), threshold);
    write_report(&output, html.as_bytes())?;
    println!("Saved history report: {}", output.display());
    println!("Included {} historical record(s).", records.len());
    Ok(())
}

fn baseline_set(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let mut name = None;
    let mut source = None;
    let mut browser = None;
    let mut directory = default_storage_directory();
    let mut replace = false;
    let mut metadata = ImportMetadata::default();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--name" if name.is_none() => name = Some(required(&mut args, &option)?),
            "--from" if source.is_none() => source = Some(required(&mut args, &option)?),
            "--browser" if browser.is_none() => browser = Some(required(&mut args, &option)?),
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            "--label" if metadata.label.is_none() => {
                metadata.label = Some(validate_user_metadata(
                    &required(&mut args, &option)?,
                    "label",
                )?)
            }
            "--commit" if metadata.user_commit.is_none() => {
                metadata.user_commit = Some(validate_user_metadata(
                    &required(&mut args, &option)?,
                    "commit",
                )?)
            }
            "--replace" if !replace => replace = true,
            other => return Err(format!("unexpected baseline set option '{other}'")),
        }
    }
    let name = name.ok_or("baseline set requires --name <name>")?;
    let source = source.ok_or("baseline set requires --from <report.json>")?;
    let source_path = PathBuf::from(&source);
    let outcome = import_report(&source_path, &directory, &metadata)?;
    let ids = outcome
        .record_ids
        .iter()
        .chain(outcome.duplicate_record_ids.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut candidates = ids
        .iter()
        .map(|id| get_record(&directory, id))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(filter) = browser.as_deref() {
        candidates.retain(|record| {
            record.browser_id.eq_ignore_ascii_case(filter)
                || record.browser_name.eq_ignore_ascii_case(filter)
                || record.installation_id.eq_ignore_ascii_case(filter)
        });
    }
    if candidates.len() != 1 {
        return Err(if candidates.is_empty() {
            "no imported report result matches --browser".to_owned()
        } else {
            "report contains multiple browser results; use --browser to select one".to_owned()
        });
    }
    let record = &candidates[0];
    set_baseline(&directory, &name, &record.record_id, replace)?;
    println!(
        "Baseline '{name}' now references {} ({}, {}).",
        record.record_id, record.browser_name, record.benchmark
    );
    Ok(())
}

fn baseline_list(arguments: Vec<String>) -> Result<(), String> {
    let directory = parse_data_dir(arguments)?;
    let baselines = list_baselines(&directory)?;
    print_markdown("## Velocity — Named Baselines");
    if baselines.is_empty() {
        println!("No named baselines.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = baselines
        .iter()
        .map(|baseline| {
            let record = get_record(&directory, &baseline.record_id).ok();
            vec![
                baseline.name.clone(),
                record.as_ref().map_or_else(
                    || "missing".to_owned(),
                    |v| format!("{} {}", v.browser_name, v.benchmark),
                ),
                baseline.record_id.clone(),
            ]
        })
        .collect();
    print_table(&["Name", "Result", "Record ID"], rows, &[]);
    Ok(())
}

fn baseline_show(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let name = args.next().ok_or("baseline show requires <name>")?;
    let directory = parse_data_dir(args.collect())?;
    let baseline = get_baseline(&directory, &name)?;
    let record = get_record(&directory, &baseline.record_id)?;
    println!("Baseline: {}", baseline.name);
    println!("Record: {}", baseline.record_id);
    println!("Browser: {} [{}]", record.browser_name, record.browser_id);
    println!(
        "Benchmark: {} {}",
        record.benchmark,
        optional(&record.benchmark_version)
    );
    println!("Date: {}", format_timestamp(record.timestamp_unix_ms));
    for metric in record.summary {
        println!("{}: {}", metric.name, format_metric(&metric));
    }
    Ok(())
}

fn baseline_delete(arguments: Vec<String>) -> Result<(), String> {
    let mut args = arguments.into_iter();
    let name = args.next().ok_or("baseline delete requires <name>")?;
    let directory = parse_data_dir(args.collect())?;
    delete_baseline(&directory, &name)?;
    println!("Deleted baseline '{name}'. Historical records were retained.");
    Ok(())
}

fn parse_data_dir(arguments: Vec<String>) -> Result<PathBuf, String> {
    let mut args = arguments.into_iter();
    let mut directory = default_storage_directory();
    while let Some(option) = args.next() {
        match option.as_str() {
            "--data-dir" => directory = PathBuf::from(required(&mut args, &option)?),
            other => return Err(format!("unexpected option '{other}'")),
        }
    }
    Ok(directory)
}

fn required(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn validate_user_metadata(value: &str, kind: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err(format!(
            "--{kind} must contain 1 to 256 non-control characters"
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn primary_metric(record: &HistoryRecord) -> Option<&HistoryMetric> {
    if record.benchmark == "speedometer" {
        record.metric("score")
    } else if record.benchmark == "startup" {
        record.metric("window_ready_ms")
    } else {
        record
            .metric("cpu_mean_percent")
            .or_else(|| record.metric("physical_footprint_mean_bytes"))
            .or_else(|| record.summary.first())
    }
}

fn format_metric(metric: &HistoryMetric) -> String {
    if metric.unit == "score" {
        format!("{:.2}", metric.median)
    } else if metric.unit == "ms" {
        format!("{:.1} ms", metric.median)
    } else if metric.unit == "bytes" {
        format!("{:.1} MiB", metric.median / (1024.0 * 1024.0))
    } else {
        format!("{:.2} {}", metric.median, metric.unit)
    }
}

fn display_path_option(path: Option<&Path>) -> String {
    path.map_or_else(|| "unknown".to_owned(), |value| value.display().to_string())
}

fn format_hardware(metadata: Option<&crate::history::HardwareMetadata>) -> String {
    let Some(metadata) = metadata else {
        return "unknown (not present in source report)".to_owned();
    };
    let mut values = Vec::new();
    if let Some(model) = &metadata.machine_model {
        values.push(model.clone());
    }
    if let Some(cpu) = &metadata.cpu_model {
        values.push(cpu.clone());
    }
    if let Some(cores) = metadata.logical_cpu_count {
        values.push(format!("{cores} logical CPUs"));
    }
    if metadata.host_identity.is_some() {
        values.push("hashed host identity available".to_owned());
    }
    if values.is_empty() {
        "unknown".to_owned()
    } else {
        values.join("; ")
    }
}

fn format_repository(metadata: Option<&crate::history::RepositoryMetadata>) -> String {
    let Some(metadata) = metadata else {
        return "unknown".to_owned();
    };
    format!(
        "{} ({}){}; observed at import",
        metadata.revision.as_deref().unwrap_or("unknown revision"),
        metadata
            .branch
            .as_deref()
            .unwrap_or("detached/unknown branch"),
        metadata
            .working_tree_dirty
            .map_or_else(String::new, |dirty| {
                if dirty {
                    "; working tree dirty".to_owned()
                } else {
                    "; clean".to_owned()
                }
            })
    )
}

fn optional(value: &Option<String>) -> String {
    value.as_deref().unwrap_or("unknown").to_owned()
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn format_date(timestamp_ms: u128) -> String {
    let (year, month, day, _, _, _) = civil_time(timestamp_ms);
    format!("{year:04}-{month:02}-{day:02}")
}

fn format_timestamp(timestamp_ms: u128) -> String {
    let (year, month, day, hour, minute, second) = civil_time(timestamp_ms);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

fn civil_time(timestamp_ms: u128) -> (i64, u32, u32, u32, u32, u32) {
    let total_seconds = timestamp_ms / 1_000;
    let days = (total_seconds / 86_400).min(i64::MAX as u128) as i64;
    let seconds = (total_seconds % 86_400) as u32;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = seconds / 3_600;
    let minute = seconds % 3_600 / 60;
    let second = seconds % 60;
    (year, month as u32, day as u32, hour, minute, second)
}

fn now_suffix() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

fn write_report(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_new_artifact(path, bytes)
        .map_err(|error| format!("cannot save history report {}: {error}", path.display()))
}

fn render_history_html(
    records: &[HistoryRecord],
    baseline: Option<&HistoryRecord>,
    threshold_percent: f64,
) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Velocity Performance History</title><style>body{font:16px system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#17202a}h1,h2{margin-bottom:.4rem}.muted,.warning{color:#59636e}.warning{background:#fff4d6;padding:.7rem;border-radius:.4rem}section{border:1px solid #d6dce2;border-radius:.6rem;padding:1rem;margin:1rem 0;overflow:auto}svg{width:100%;min-width:480px;height:260px}table{border-collapse:collapse;width:100%}th,td{text-align:left;border-bottom:1px solid #e1e5e8;padding:.45rem}code{overflow-wrap:anywhere}</style><body><h1>Velocity Performance History</h1>",
    );
    html.push_str(&format!(
        "<p class=\"muted\">{} record(s). Lines connect results only within matching browser installation, benchmark version, OS, architecture, configuration, scenario, and recorded hardware metadata.</p>",
        records.len()
    ));
    html.push_str("<p class=\"warning\">OS and architecture do not identify a unique machine. Missing hardware metadata prevents confirming that results came from the same hardware. A threshold crossing is descriptive and does not by itself establish statistical significance.</p>");
    let mut groups = BTreeMap::<String, Vec<&HistoryRecord>>::new();
    for record in records {
        let host_scope = record
            .hardware
            .as_ref()
            .and_then(|hardware| hardware.host_identity.as_deref())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("unverified-{}", record.record_id));
        let group = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            record.benchmark,
            record.benchmark_version.as_deref().unwrap_or("unknown"),
            record.browser_id,
            record.installation_id,
            record.operating_system,
            record.architecture,
            serde_json::to_string(&record.hardware).unwrap_or_default(),
            host_scope,
            serde_json::to_string(&record.configuration).unwrap_or_default(),
        );
        groups.entry(group).or_default().push(record);
    }
    for group in groups.values_mut() {
        group.sort_by_key(|record| record.timestamp_unix_ms);
        let record = group[0];
        for metric_name in record.summary.iter().map(|metric| metric.name.clone()) {
            let points = group
                .iter()
                .filter_map(|record| record.metric(&metric_name).map(|metric| (*record, metric)))
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            html.push_str(&format!(
                "<section><h2>{} — {} / {}</h2><p class=\"muted\">{} · {} · {} {}</p>",
                escape_html(&record.browser_name),
                escape_html(&record.benchmark),
                escape_html(&metric_name),
                escape_html(
                    record
                        .benchmark_version
                        .as_deref()
                        .unwrap_or("benchmark version unknown")
                ),
                escape_html(&record.operating_system),
                escape_html(&record.architecture),
                points[0].1.unit,
            ));
            render_chart(&mut html, &points, baseline, threshold_percent);
            html.push_str("<table><thead><tr><th>Date</th><th>Build label</th><th>Median</th><th>Variation</th><th>Samples</th><th>Change vs baseline</th></tr></thead><tbody>");
            for (point, metric) in points {
                let label = build_identifier(point);
                let change = baseline
                    .filter(|base| same_comparison_dimensions(point, base))
                    .and_then(|base| base.metric(&metric_name))
                    .filter(|base_metric| base_metric.median > 0.0)
                    .map_or_else(
                        || "N/A".to_owned(),
                        |base_metric| {
                            let percent =
                                (metric.median - base_metric.median) / base_metric.median * 100.0;
                            let direction = metric.higher_is_better.unwrap_or(false);
                            let worsened = if direction {
                                percent < -threshold_percent
                            } else {
                                percent > threshold_percent
                            };
                            let host_note = if point
                                .hardware
                                .as_ref()
                                .and_then(|value| value.host_identity.as_ref())
                                .is_none()
                            {
                                "; host identity unverified"
                            } else {
                                ""
                            };
                            if worsened {
                                format!("{percent:+.2}% (threshold crossed{host_note})")
                            } else {
                                format!("{percent:+.2}%{host_note}")
                            }
                        },
                    );
                let variation = metric.standard_deviation.map_or_else(
                    || "N/A".to_owned(),
                    |value| format!("{value:.3} {}", metric.unit),
                );
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{:.3} {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    format_date(point.timestamp_unix_ms), escape_html(&label), metric.median,
                    escape_html(&metric.unit), variation, metric.sample_count, escape_html(&change)
                ));
            }
            html.push_str("</tbody></table>");
            let warnings = group
                .iter()
                .flat_map(|item| item.comparability.warnings.iter())
                .collect::<std::collections::BTreeSet<_>>();
            for warning in warnings {
                html.push_str(&format!(
                    "<p class=\"warning\">{}</p>",
                    escape_html(warning)
                ));
            }
            html.push_str("</section>");
        }
    }
    if records.is_empty() {
        html.push_str("<p>No matching records are available.</p>");
    }
    html.push_str("</body></html>");
    html
}

fn render_chart(
    html: &mut String,
    points: &[(&HistoryRecord, &HistoryMetric)],
    baseline: Option<&HistoryRecord>,
    threshold_percent: f64,
) {
    let minimum = points
        .iter()
        .map(|(_, metric)| metric.median)
        .fold(f64::INFINITY, f64::min);
    let maximum = points
        .iter()
        .map(|(_, metric)| metric.median)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (maximum - minimum).max(maximum.abs() * 0.02).max(1e-9);
    let height = 190.0;
    let width = 900.0;
    let x = |index: usize| {
        if points.len() < 2 {
            width / 2.0
        } else {
            45.0 + index as f64 * (width - 90.0) / (points.len() - 1) as f64
        }
    };
    let y = |value: f64| 25.0 + (maximum - value) / span * (height - 50.0);
    html.push_str(&format!("<svg role=\"img\" aria-label=\"Historical median trend, higher or lower is better as indicated by metric\" viewBox=\"0 0 {width:.0} {height:.0}\"><line x1=\"45\" y1=\"25\" x2=\"45\" y2=\"165\" stroke=\"#778\"/><line x1=\"45\" y1=\"165\" x2=\"855\" y2=\"165\" stroke=\"#778\"/><text x=\"4\" y=\"30\">{maximum:.3}</text><text x=\"4\" y=\"165\">{minimum:.3}</text>"));
    if points.len() > 1
        && points.first().is_some_and(|(record, _)| {
            record
                .hardware
                .as_ref()
                .and_then(|hardware| hardware.host_identity.as_ref())
                .is_some()
        })
    {
        let coordinates = points
            .iter()
            .enumerate()
            .map(|(index, (_, metric))| format!("{:.1},{:.1}", x(index), y(metric.median)))
            .collect::<Vec<_>>()
            .join(" ");
        html.push_str(&format!("<polyline points=\"{coordinates}\" fill=\"none\" stroke=\"#2474a6\" stroke-width=\"2\"/>"));
    }
    for (index, (record, metric)) in points.iter().enumerate() {
        let crossed = baseline
            .filter(|base| same_comparison_dimensions(record, base))
            .and_then(|base| base.metric(&metric.name))
            .is_some_and(|base| {
                if base.median <= 0.0 {
                    return false;
                }
                let change = (metric.median - base.median) / base.median * 100.0;
                if metric.higher_is_better.unwrap_or(false) {
                    change < -threshold_percent
                } else {
                    change > threshold_percent
                }
            });
        let color = if crossed { "#b33d2e" } else { "#2474a6" };
        html.push_str(&format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"{color}\"><title>{}: {:.3} {}</title></circle>", x(index), y(metric.median), escape_html(&format_date(record.timestamp_unix_ms)), metric.median, escape_html(&metric.unit)));
    }
    html.push_str("</svg>");
}

fn same_comparison_dimensions(left: &HistoryRecord, right: &HistoryRecord) -> bool {
    left.benchmark == right.benchmark
        && left.benchmark_version == right.benchmark_version
        && left.browser_id == right.browser_id
        && left.installation_id == right.installation_id
        && left.operating_system == right.operating_system
        && left.architecture == right.architecture
        && left.hardware == right.hardware
        && left.configuration == right.configuration
        && left.scenario == right.scenario
}

fn build_identifier(record: &HistoryRecord) -> String {
    if let Some(label) = record.label.as_deref() {
        return label.to_owned();
    }
    if let Some(revision) = record.executable_build.revision.as_deref() {
        return revision.to_owned();
    }
    if let Some(revision) = record.user_supplied_commit.as_deref() {
        return format!("user:{revision}");
    }
    "unknown build".to_owned()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn print_history_help() {
    print_markdown(
        "## `velocity history`\n\nCommands: `import <report.json>`, `list`, `show <record-id>`, `report`, and `export <record-id> --output <path>`. Use `--data-dir <path>` on each operation or set `VELOCITY_DATA_DIR`.",
    );
}

fn print_baseline_help() {
    print_markdown(
        "## `velocity baseline`\n\nCommands: `set --name <name> --from <report.json> [--browser <id>] [--replace]`, `list`, `show <name>`, and `delete <name>`. A baseline references an immutable history record.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{ComparabilityMetadata, ExecutableBuildMetadata};

    fn history_record(id: &str, version: &str, score: f64) -> HistoryRecord {
        HistoryRecord {
            schema_version: 1,
            record_id: id.to_owned(),
            source_sha256: format!("sha-{id}"),
            benchmark: "speedometer".to_owned(),
            benchmark_version: Some(version.to_owned()),
            benchmark_schema_version: 2,
            browser_id: "photon".to_owned(),
            installation_id: "photon-development".to_owned(),
            browser_name: "Photon".to_owned(),
            browser_version: Some("0.1.0".to_owned()),
            executable_path: None,
            timestamp_unix_ms: 1_792_080_000_000 + id.len() as u128,
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
            configuration: serde_json::json!({"runs": 3, "warmups": 1}),
            scenario: None,
            summary: vec![HistoryMetric {
                name: "score".to_owned(),
                unit: "score".to_owned(),
                median: score,
                mean: Some(score),
                standard_deviation: Some(0.1),
                sample_count: 3,
                higher_is_better: Some(true),
            }],
            comparability: ComparabilityMetadata::default(),
            original_report_name: "report.json".to_owned(),
            archived_report: format!("reports/{id}.json"),
            label: None,
            user_supplied_commit: None,
        }
    }

    #[test]
    fn date_formatting_handles_known_utc_timestamp() {
        assert_eq!(format_date(1_792_080_000_000), "2026-10-15");
    }

    #[test]
    fn history_chart_is_self_contained_and_reports_empty_data() {
        let html = render_history_html(&[], None, 5.0);
        assert!(html.contains("No matching records"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn history_report_separates_versions_and_marks_hardware_uncertainty() {
        let baseline = history_record("old", "3.1", 12.0);
        let current = history_record("new", "3.1", 11.0);
        let html = render_history_html(&[baseline.clone(), current], Some(&baseline), 5.0);
        assert!(html.contains("host identity unverified"));
        assert!(html.contains("threshold crossed"));
        assert!(!html.contains("<polyline"));
        assert_eq!(html.matches("<svg").count(), 2);

        let other_version = history_record("next", "3.2", 14.0);
        let html = render_history_html(&[baseline, other_version], None, 5.0);
        assert_eq!(html.matches("<section>").count(), 2);
    }
}
