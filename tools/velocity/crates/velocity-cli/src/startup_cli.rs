#[cfg(target_os = "macos")]
use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hash, Hasher},
    time::{SystemTime, UNIX_EPOCH},
};
use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use velocity_browser::BrowserInstallation;
#[cfg(target_os = "macos")]
use velocity_browser::BrowserRegistry;
use velocity_core::{
    StartupBenchmarkConfig, StartupBenchmarkReport, StartupMetric, StartupMode,
    StartupProgressEvent, startup_benchmark_definition, unique_run_id,
};

#[cfg(target_os = "macos")]
use crate::macos_startup::MacOSStartupBackend;
#[cfg(target_os = "macos")]
use velocity_browser::macos::default_session_store_path;
#[cfg(target_os = "macos")]
use velocity_core::StartupBenchmarkExecutor;
#[cfg(target_os = "macos")]
use velocity_core::{
    BenchmarkFailure, BenchmarkFailureKind, BenchmarkResult, BenchmarkRun, CancellationToken,
    RunOrderMetadata, WarmupIterationResult,
};

#[derive(Clone, Debug)]
enum TargetSelection {
    One(String),
    Many(Vec<String>),
    All,
    Prompt,
}

#[derive(Clone, Debug)]
struct StartupOptions {
    target: TargetSelection,
    config: StartupBenchmarkConfig,
    output_directory: PathBuf,
    randomize: bool,
    record_history: bool,
}

#[cfg(target_os = "macos")]
struct BrowserBenchmarkSession {
    installation: BrowserInstallation,
    executor: StartupBenchmarkExecutor<MacOSStartupBackend>,
    warmups: Vec<WarmupIterationResult>,
    iterations: Vec<BenchmarkResult>,
    run_failures: Vec<BenchmarkFailure>,
    timestamp_unix_ms: u128,
}

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let Some(subcommand) = arguments.next() else {
        print_help();
        return Ok(());
    };
    if matches!(subcommand.as_str(), "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }
    if subcommand != "startup" {
        return Err(format!(
            "unknown benchmark '{subcommand}'; available benchmark: startup"
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
    let options = parse_options(arguments.into_iter())?;
    run_startup(options)
}

fn parse_options(mut arguments: impl Iterator<Item = String>) -> Result<StartupOptions, String> {
    let mut browser = None;
    let mut browsers = None;
    let mut all = false;
    let mut runs = 5;
    let mut warmup_iterations = 0;
    let mut mode = StartupMode::Cold;
    let mut timeout_seconds = 30;
    let mut output_directory = None;
    let mut randomize = false;
    let mut record_history = true;
    let mut history_record_option_seen = false;

    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--browser" => {
                if browser.is_some() || browsers.is_some() {
                    return Err("use --browser or --browsers once".to_owned());
                }
                browser = Some(required_value(&mut arguments, "--browser")?);
            }
            "--browsers" => {
                if browser.is_some() || browsers.is_some() {
                    return Err("use --browser or --browsers once".to_owned());
                }
                let value = required_value(&mut arguments, "--browsers")?;
                let values = value.split(',').map(str::trim).collect::<Vec<_>>();
                if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                    return Err(
                        "--browsers requires comma-separated browser identifiers".to_owned()
                    );
                }
                browsers = Some(values.into_iter().map(str::to_owned).collect());
            }
            "--all" => {
                if all {
                    return Err("--all may be specified only once".to_owned());
                }
                all = true;
            }
            "--runs" => {
                runs = parse_value(&mut arguments, "--runs")?;
            }
            "--warmup" => {
                warmup_iterations = parse_value(&mut arguments, "--warmup")?;
            }
            "--timeout" => {
                timeout_seconds = parse_value(&mut arguments, "--timeout")?;
            }
            "--mode" => {
                mode = match required_value(&mut arguments, "--mode")?.as_str() {
                    "cold" => StartupMode::Cold,
                    "warm" => StartupMode::Warm,
                    other => {
                        return Err(format!(
                            "invalid startup mode '{other}'; use 'cold' or 'warm'"
                        ));
                    }
                };
            }
            "--output" => {
                output_directory = Some(PathBuf::from(required_value(&mut arguments, "--output")?));
            }
            "--randomize" => {
                if randomize {
                    return Err("--randomize may be specified only once".to_owned());
                }
                randomize = true;
            }
            "--record-history" | "--no-record-history" => {
                if history_record_option_seen {
                    return Err(
                        "choose only one of --record-history and --no-record-history".to_owned(),
                    );
                }
                history_record_option_seen = true;
                record_history = option == "--record-history";
            }
            other => return Err(format!("unknown startup option '{other}'")),
        }
    }

    let target = match (browser, browsers, all) {
        (Some(browser), None, false) => TargetSelection::One(browser),
        (None, Some(browsers), false) => TargetSelection::Many(browsers),
        (None, None, true) => TargetSelection::All,
        (Some(_), None, true) | (None, Some(_), true) | (Some(_), Some(_), _) => {
            return Err("use either --browser, --browsers, or --all".to_owned());
        }
        (None, None, false) => TargetSelection::Prompt,
    };
    let timeout = std::time::Duration::from_secs(timeout_seconds);
    let config = StartupBenchmarkConfig::new_with_warmups(runs, warmup_iterations, mode, timeout)
        .map_err(|error| error.to_string())?;

    Ok(StartupOptions {
        target,
        config,
        output_directory: output_directory.unwrap_or_else(default_output_directory),
        randomize,
        record_history,
    })
}

fn required_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_value<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, String> {
    let value = required_value(arguments, option)?;
    value
        .parse()
        .map_err(|_| format!("invalid value '{value}' for {option}"))
}

fn default_output_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("tools/velocity"))
        .join("results")
}

#[cfg(target_os = "macos")]
fn run_startup(options: StartupOptions) -> Result<(), String> {
    let registry = super::discover_browsers()?;
    let targets = match &options.target {
        TargetSelection::One(selector) => {
            vec![super::browser_selection::resolve(&registry, selector)?.clone()]
        }
        TargetSelection::Many(selectors) => {
            let mut targets = Vec::new();
            for selector in selectors {
                let installation = super::browser_selection::resolve(&registry, selector)?;
                if !targets.iter().any(|existing: &BrowserInstallation| {
                    existing.installation_id == installation.installation_id
                }) {
                    targets.push(installation.clone());
                }
            }
            targets
        }
        TargetSelection::All => registry.installations().cloned().collect::<Vec<_>>(),
        TargetSelection::Prompt => super::browser_selection::choose(&registry, None, true)?
            .into_iter()
            .cloned()
            .collect(),
    };
    if targets.is_empty() {
        return Err("no supported browser installations were found".to_owned());
    }
    fs::create_dir_all(&options.output_directory).map_err(|error| {
        format!(
            "cannot create result directory {}: {error}",
            options.output_directory.display()
        )
    })?;

    let session_store_path = default_session_store_path()?;
    let profile_root = session_store_path
        .parent()
        .ok_or_else(|| "Velocity session store has no parent directory".to_owned())?
        .join("Profiles");

    let comparison_run_id = unique_run_id();
    let randomization_seed = options.randomize.then(randomization_seed);
    let mut random = RoundRandom(randomization_seed.unwrap_or_default());
    let warmup_orders = make_round_orders(
        targets.len(),
        options.config.warmup_iterations,
        options.randomize,
        &mut random,
    );
    let measured_orders = make_round_orders(
        targets.len(),
        options.config.runs.iterations(),
        options.randomize,
        &mut random,
    );

    crate::terminal::print_markdown(&format!(
        "# Velocity — Startup Benchmark\n\n- **Mode:** {}\n- **Measured iterations:** {} per browser\n- **Warm-up iterations:** {} per browser\n- **Randomized round order:** {}\n- **Window readiness timeout:** {} seconds",
        options.config.mode,
        options.config.runs.iterations(),
        options.config.warmup_iterations,
        if options.randomize { "yes" } else { "no" },
        options.config.timeout.as_secs()
    ));

    let cancellation = CancellationToken::default();
    install_cancellation_handlers(cancellation.clone())?;
    let mut sessions = targets
        .into_iter()
        .map(|installation| {
            let backend = MacOSStartupBackend::new(
                profile_root.clone(),
                session_store_path.clone(),
                options.config.mode,
            );
            let executor = StartupBenchmarkExecutor::new(
                installation.clone(),
                options.config,
                backend,
                cancellation.clone(),
            )
            .with_progress(print_progress);
            BrowserBenchmarkSession {
                installation,
                executor,
                warmups: Vec::new(),
                iterations: Vec::new(),
                run_failures: Vec::new(),
                timestamp_unix_ms: unix_time_millis(),
            }
        })
        .collect::<Vec<_>>();

    for session in &mut sessions {
        let run = make_benchmark_run(&session.installation, options.config);
        if let Err(error) = session.executor.prepare_for(&run) {
            session.run_failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::Setup,
                error.to_string(),
            ));
        }
    }

    let mut executed_warmup_orders = Vec::new();
    for (round, order) in warmup_orders.iter().enumerate() {
        if cancellation.is_cancelled() {
            break;
        }
        let mut executed_round = Vec::new();
        for &browser_index in order {
            if cancellation.is_cancelled() {
                break;
            }
            let session = &mut sessions[browser_index];
            println!(
                "\nWarm-up round {}/{} — {}",
                round + 1,
                warmup_orders.len(),
                selected_browser_name(&registry, &session.installation)
            );
            let iteration =
                std::num::NonZeroUsize::new(round + 1).expect("round index is non-zero");
            session
                .warmups
                .push(session.executor.run_warmup_iteration(iteration));
            executed_round.push(browser_index);
        }
        if !executed_round.is_empty() {
            executed_warmup_orders.push(executed_round);
        }
    }

    let mut executed_measured_orders = Vec::new();
    for (round, order) in measured_orders.iter().enumerate() {
        if cancellation.is_cancelled() {
            break;
        }
        let mut executed_round = Vec::new();
        for &browser_index in order {
            if cancellation.is_cancelled() {
                break;
            }
            let session = &mut sessions[browser_index];
            println!(
                "\nMeasured round {}/{} — {}",
                round + 1,
                measured_orders.len(),
                selected_browser_name(&registry, &session.installation)
            );
            let iteration =
                std::num::NonZeroUsize::new(round + 1).expect("round index is non-zero");
            session
                .iterations
                .push(session.executor.run_measured_iteration(iteration));
            executed_round.push(browser_index);
        }
        if !executed_round.is_empty() {
            executed_measured_orders.push(executed_round);
        }
    }

    let warmup_order_by_round = order_metadata(&executed_warmup_orders, &sessions);
    let measured_order_by_round = order_metadata(&executed_measured_orders, &sessions);
    let mut reports = Vec::with_capacity(sessions.len());
    let mut save_errors = Vec::new();
    for mut session in sessions {
        if let Err(message) = session.executor.finish() {
            session.run_failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::Cleanup,
                message,
            ));
        }
        let profile_strategy = session.executor.profile_strategy();
        let mut report = StartupBenchmarkReport::new(
            &session.installation,
            options.config,
            !profile_strategy.starts_with("unsupported:"),
            profile_strategy,
            session.iterations,
        );
        report.timestamp_unix_ms = session.timestamp_unix_ms;
        report.warmup_iterations = session.warmups;
        report.run_failures = session.run_failures;
        report.cancelled |= cancellation.is_cancelled();
        report.cancelled |= report.warmup_iterations.iter().any(|iteration| {
            iteration
                .failures
                .iter()
                .any(|failure| failure.kind == BenchmarkFailureKind::Cancelled)
        });
        report.run_order = RunOrderMetadata {
            comparison_run_id: Some(comparison_run_id.clone()),
            randomized: options.randomize,
            randomization_seed,
            warmup_order_by_round: warmup_order_by_round.clone(),
            measured_order_by_round: measured_order_by_round.clone(),
        };
        match save_report(&options.output_directory, &report) {
            Ok(path) => {
                println!("Saved: {}", display_path(&path));
                if options.record_history && report.summary.values().any(Option::is_some) {
                    super::history_cli::record_after_success(&path);
                }
            }
            Err(error) => {
                eprintln!("Could not save {} report: {error}", report.browser.name);
                save_errors.push(error);
            }
        }
        reports.push(report);
    }

    super::comparison::print_browser_comparison(&reports);
    let html_path = super::comparison::save_browser_comparison_html(
        &options.output_directory,
        &comparison_run_id,
        &reports,
    )?;
    println!("Saved HTML: {}", display_path(&html_path));
    if !save_errors.is_empty() {
        return Err(format!(
            "{} browser JSON report(s) could not be saved",
            save_errors.len()
        ));
    }
    if cancellation.is_cancelled() {
        println!("Benchmark cancelled; partial results were saved.");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_startup(_options: StartupOptions) -> Result<(), String> {
    Err("startup benchmarking and browser lifecycle currently support macOS only".to_owned())
}

#[cfg(target_os = "macos")]
fn selected_browser_name(registry: &BrowserRegistry, installation: &BrowserInstallation) -> String {
    let count = registry.by_browser(&installation.metadata.id).len();
    if count > 1 {
        format!(
            "{} [{}]",
            installation.metadata.name, installation.installation_id
        )
    } else {
        installation.metadata.name.clone()
    }
}

#[cfg(target_os = "macos")]
fn make_benchmark_run(
    installation: &BrowserInstallation,
    config: StartupBenchmarkConfig,
) -> BenchmarkRun {
    BenchmarkRun {
        definition: startup_benchmark_definition(),
        browser_id: installation.metadata.id.clone(),
        config: config.runs,
    }
}

#[cfg(target_os = "macos")]
fn order_metadata(rounds: &[Vec<usize>], sessions: &[BrowserBenchmarkSession]) -> Vec<Vec<String>> {
    rounds
        .iter()
        .map(|order| {
            order
                .iter()
                .map(|&index| sessions[index].installation.installation_id.to_string())
                .collect()
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn randomization_seed() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    hasher.finish()
}

#[cfg(target_os = "macos")]
struct RoundRandom(u64);

#[cfg(target_os = "macos")]
impl RoundRandom {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

#[cfg(target_os = "macos")]
fn make_round_orders(
    browser_count: usize,
    rounds: usize,
    randomize: bool,
    random: &mut RoundRandom,
) -> Vec<Vec<usize>> {
    (0..rounds)
        .map(|_| {
            let mut order = (0..browser_count).collect::<Vec<_>>();
            if randomize {
                for index in (1..order.len()).rev() {
                    let other = (random.next() % (index as u64 + 1)) as usize;
                    order.swap(index, other);
                }
            }
            order
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn print_progress(event: StartupProgressEvent) {
    match event {
        StartupProgressEvent::WarmupStarting { number, total } => {
            println!("      Warm-up {number}/{total} (timings discarded)...");
        }
        StartupProgressEvent::WarmupFinished {
            number,
            ready,
            reason,
            ..
        } => {
            if let Some(reason) = reason {
                println!("      Warm-up {number}: failed ({reason})");
            } else {
                println!(
                    "      Warm-up {number}: {}",
                    if ready { "ready" } else { "failed" }
                );
            }
        }
        StartupProgressEvent::IterationStarting { number, total } => {
            println!("\n[{number}/{total}] Preparing...");
        }
        StartupProgressEvent::Launching { .. } => println!("      Launching browser..."),
        StartupProgressEvent::ProcessStarted {
            number: _,
            elapsed_ms,
        } => {
            println!("      Process started: {elapsed_ms:.1} ms");
        }
        StartupProgressEvent::WindowVisible { elapsed_ms, .. } => {
            println!("      Window visible: {elapsed_ms:.1} ms");
        }
        StartupProgressEvent::WindowReady { elapsed_ms, .. } => {
            println!("      Window ready: {elapsed_ms:.1} ms");
        }
        StartupProgressEvent::MetricUnavailable { metric, reason, .. } => {
            println!("      {}: N/A ({reason})", metric_label(metric))
        }
        StartupProgressEvent::CleaningUp { .. } => println!("      Cleaning up..."),
        StartupProgressEvent::IterationFinished { .. } => println!("      Iteration complete."),
    }
}

fn metric_label(metric: StartupMetric) -> &'static str {
    match metric {
        StartupMetric::ProcessStart => "Process started",
        StartupMetric::WindowVisible => "Window visible",
        StartupMetric::WindowReady => "Window ready",
    }
}

fn save_report(directory: &Path, report: &StartupBenchmarkReport) -> Result<PathBuf, String> {
    let browser_id = &report.browser.id;
    for _ in 0..8 {
        let path = directory.join(format!("startup-{browser_id}-{}.json", unique_run_id()));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create result file {}: {error}",
                    path.display()
                ));
            }
        };
        let write_result = serde_json::to_writer_pretty(&mut file, report)
            .map_err(std::io::Error::other)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all());
        if let Err(error) = write_result {
            let _ = fs::remove_file(&path);
            return Err(format!(
                "cannot write result file {}: {error}",
                path.display()
            ));
        }
        return Ok(path);
    }
    Err("could not allocate a unique startup result filename".to_owned())
}

fn display_path(path: &Path) -> String {
    env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(target_os = "macos")]
static SIGNAL_CANCELLATION: std::sync::OnceLock<CancellationToken> = std::sync::OnceLock::new();

#[cfg(target_os = "macos")]
extern "C" fn cancel_on_signal(_signal: libc::c_int) {
    if let Some(cancellation) = SIGNAL_CANCELLATION.get() {
        cancellation.cancel();
    }
}

#[cfg(target_os = "macos")]
fn install_cancellation_handlers(cancellation: CancellationToken) -> Result<(), String> {
    let _ = SIGNAL_CANCELLATION.set(cancellation);
    // The handler only sets an atomic flag; process cleanup runs on the normal
    // benchmark thread, where the managed-session manager can verify ownership.
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::signal(signal, cancel_on_signal as *const () as libc::sighandler_t)
                == libc::SIG_ERR
            {
                return Err(format!(
                    "cannot install cancellation handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

fn print_help() {
    crate::terminal::print_markdown(
        "## `velocity bench startup`\n\n**Usage:** `velocity bench startup [--browser <id|number> | --browsers <id|number,...> | --all] [OPTIONS]`\n\nIf no target is given in an interactive terminal, Velocity shows a numbered browser picker.\n\n### Options\n\n- `--runs <count>` — Measured iterations per browser (default: 5)\n- `--warmup <count>` — Unmeasured launches before results (default: 0)\n- `--randomize` — Shuffle browser order independently each round\n- `--mode <cold|warm>` — Isolated profile policy (default: cold)\n- `--timeout <seconds>` — Window readiness timeout (default: 30)\n- `--output <directory>` — Result directory (default: `tools/velocity/results`)\n- `--record-history` / `--no-record-history` — Store successful reports in local history (default: enabled)\n- `-h, --help` — Print this help",
    );
}

#[cfg(test)]
fn run_targets_sequentially<T, R>(
    targets: impl IntoIterator<Item = T>,
    mut run: impl FnMut(T) -> Result<R, String>,
    mut report_error: impl FnMut(String),
    mut should_stop: impl FnMut() -> bool,
) -> Vec<R> {
    let mut results = Vec::new();
    for target in targets {
        if should_stop() {
            break;
        }
        match run(target) {
            Ok(result) => results.push(result),
            Err(error) => report_error(error),
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_browser_cold_and_default_iteration_count() {
        let options = parse_options(args(&["--browser", "photon"]).into_iter()).unwrap();
        assert!(matches!(options.target, TargetSelection::One(ref id) if id == "photon"));
        assert_eq!(options.config.mode, StartupMode::Cold);
        assert_eq!(options.config.runs.iterations(), 5);
        assert_eq!(options.config.timeout.as_secs(), 30);
        assert!(options.record_history);
    }

    #[test]
    fn parses_all_warm_and_custom_runs() {
        let options = parse_options(
            args(&["--all", "--runs", "10", "--mode", "warm", "--timeout", "5"]).into_iter(),
        )
        .unwrap();
        assert!(matches!(options.target, TargetSelection::All));
        assert_eq!(options.config.mode, StartupMode::Warm);
        assert_eq!(options.config.runs.iterations(), 10);
        assert_eq!(options.config.timeout.as_secs(), 5);
    }

    #[test]
    fn rejects_conflicting_selectors_and_zero_runs() {
        assert!(parse_options(args(&["--all", "--browser", "chrome"]).into_iter()).is_err());
        assert!(parse_options(args(&["--browser", "chrome", "--runs", "0"]).into_iter()).is_err());
        assert!(
            !parse_options(args(&["--browser", "chrome", "--no-record-history"]).into_iter())
                .unwrap()
                .record_history
        );
        assert!(
            parse_options(
                args(&[
                    "--browser",
                    "chrome",
                    "--record-history",
                    "--no-record-history"
                ])
                .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn all_targets_run_sequentially_after_an_individual_failure() {
        let mut visited = Vec::new();
        let mut errors = Vec::new();
        let results = run_targets_sequentially(
            ["photon", "chrome", "safari"],
            |browser| {
                visited.push(browser);
                if browser == "chrome" {
                    Err("fixture launch failure".to_owned())
                } else {
                    Ok(browser.to_owned())
                }
            },
            |error| errors.push(error),
            || false,
        );
        assert_eq!(visited, ["photon", "chrome", "safari"]);
        assert_eq!(results, ["photon", "safari"]);
        assert_eq!(errors, ["fixture launch failure"]);
    }
}
