use std::{
    fs,
    path::{Path, PathBuf},
};

use velocity_core::StartupBenchmarkReport;

use crate::comparison::{
    DEFAULT_MIN_ABSOLUTE_REGRESSION_MS, RegressionThresholds, compare_startup_reports,
    print_regression_comparison, save_regression_html,
};

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let Some(benchmark) = arguments.next() else {
        print_help();
        return Ok(());
    };
    if matches!(benchmark.as_str(), "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }
    if benchmark != "startup" {
        return Err(format!(
            "unknown comparison '{benchmark}'; available comparison: startup"
        ));
    }

    let arguments = arguments.collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        print_help();
        return Ok(());
    }
    let mut arguments = arguments.into_iter();
    let mut baseline = None;
    let mut current = None;
    let mut threshold_percent = 5.0_f64;
    let mut min_absolute_ms = DEFAULT_MIN_ABSOLUTE_REGRESSION_MS;
    let mut output_directory = default_output_directory();
    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--baseline" => {
                if baseline.is_some() {
                    return Err("--baseline may be specified only once".to_owned());
                }
                baseline = Some(required_value(&mut arguments, &option)?);
            }
            "--current" => {
                if current.is_some() {
                    return Err("--current may be specified only once".to_owned());
                }
                current = Some(required_value(&mut arguments, &option)?);
            }
            "--threshold-percent" | "--threshold" => {
                let value = required_value(&mut arguments, &option)?;
                threshold_percent = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid percentage '{value}'"))?;
                if !threshold_percent.is_finite() || threshold_percent < 0.0 {
                    return Err(format!("{option} must be finite and non-negative"));
                }
            }
            "--min-absolute-ms" => {
                let value = required_value(&mut arguments, &option)?;
                min_absolute_ms = value
                    .parse::<f64>()
                    .map_err(|_| format!("invalid millisecond value '{value}'"))?;
                if !min_absolute_ms.is_finite() || min_absolute_ms < 0.0 {
                    return Err("--min-absolute-ms must be finite and non-negative".to_owned());
                }
            }
            "--output" => {
                output_directory = PathBuf::from(required_value(&mut arguments, &option)?);
            }
            other => return Err(format!("unknown compare option '{other}'")),
        }
    }

    let storage_directory = crate::history::default_storage_directory();
    let baseline_selector =
        baseline.ok_or("compare startup requires --baseline <report-or-baseline>")?;
    let current_selector =
        current.ok_or("compare startup requires --current <report-or-baseline>")?;
    let baseline_history =
        crate::history::record_for_selector(&baseline_selector, &storage_directory)?;
    let current_history =
        crate::history::record_for_selector(&current_selector, &storage_directory)?;
    if crate::history::hardware_comparability(baseline_history.as_ref(), current_history.as_ref())
        == Some(false)
    {
        return Err(
            "startup reports have a known hardware configuration or host identity mismatch and are not comparable"
                .to_owned(),
        );
    }
    if crate::history::hardware_comparability(baseline_history.as_ref(), current_history.as_ref())
        .is_none()
    {
        println!(
            "Warning: hardware identity is unavailable; OS and architecture match does not prove both runs used the same machine."
        );
    }
    let baseline_path =
        crate::history::resolve_report_selector(&baseline_selector, &storage_directory)?;
    let current_path =
        crate::history::resolve_report_selector(&current_selector, &storage_directory)?;
    let baseline_report = read_report(&baseline_path)?;
    let current_report = read_report(&current_path)?;
    let thresholds = RegressionThresholds {
        relative_percent: threshold_percent,
        absolute_ms: min_absolute_ms,
    };
    let rows = compare_startup_reports(&baseline_report, &current_report, thresholds)?;
    let has_regressions = print_regression_comparison(&rows, thresholds);
    let html_path = save_regression_html(
        &output_directory,
        &baseline_path,
        &current_path,
        &baseline_report,
        &current_report,
        thresholds,
        &rows,
    )?;
    println!("Saved HTML: {}", display_path(&html_path));
    if has_regressions {
        Err("startup regression detected".to_owned())
    } else {
        Ok(())
    }
}

fn required_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn read_report(path: &Path) -> Result<StartupBenchmarkReport, String> {
    let contents = fs::read(path)
        .map_err(|error| format!("cannot read report {}: {error}", path.display()))?;
    serde_json::from_slice(&contents)
        .map_err(|error| format!("invalid startup report {}: {error}", path.display()))
}

fn default_output_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("tools/velocity"))
        .join("results")
}

fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

fn print_help() {
    crate::terminal::print_markdown(
        "## `velocity compare startup`\n\n**Usage:** `velocity compare startup --baseline <report-or-name> --current <report-or-name> [OPTIONS]`\n\n### Options\n\n- `--threshold <n>` or `--threshold-percent <n>` — Relative regression threshold (default: 5%)\n- `--min-absolute-ms <n>` — Minimum absolute change required (default: 1 ms). A regression must exceed both thresholds and have permutation p < 0.05.\n- `--output <directory>` — HTML report directory (default: `tools/velocity/results`)\n- `-h, --help` — Print this help\n\nSelectors may be JSON report paths, history record IDs, or named baselines. A threshold crossing with insufficient sample evidence is inconclusive.",
    );
}
