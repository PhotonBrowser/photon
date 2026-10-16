use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use velocity_browser::{
    BrowserAutomationAdapter, BrowserEngine, BrowserInstallation, JetStreamPageStatus,
};
use velocity_core::{ReportBrowser, unique_run_id};
use velocity_metrics::summarize_metric_values;

use crate::jetstream_source::{
    JETSTREAM_ARCHIVE_SHA256, JETSTREAM_COMMIT, JETSTREAM_VERSION, official_url,
};

const DEFAULT_RUNS: usize = 5;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_TIMEOUT_SECONDS: u64 = 1800;
const MAX_RUNS: usize = 100;
const MAX_WARMUPS: usize = 20;
const MIN_COMPARISON_RUNS: usize = 5;
const MAX_SUBTESTS: usize = 128;

static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Debug)]
enum TargetSelection {
    One(String),
    Many(Vec<String>),
    All,
    Prompt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceKind {
    Local,
    Official,
}

impl SourceKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local" => Ok(Self::Local),
            "official" => Ok(Self::Official),
            other => Err(format!(
                "unknown JetStream source '{other}'; choose local or official"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Local => "official_pinned_local",
            Self::Official => "official_hosted",
        }
    }
}

#[derive(Clone, Debug)]
struct Options {
    target: TargetSelection,
    runs: usize,
    warmups: usize,
    randomized: bool,
    timeout: Duration,
    source: SourceKind,
    output_directory: PathBuf,
    record_history: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SourceMetadata {
    kind: String,
    url: String,
    version: String,
    official_commit: Option<String>,
    archive_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct BenchmarkConfiguration {
    requested_runs: usize,
    warmup_runs: usize,
    timeout_seconds: u64,
    browser_mode: String,
    source_kind: String,
    workload_selection: String,
    prefetch_resources: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EnvironmentMetadata {
    operating_system: String,
    architecture: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExecutionOrder {
    randomized: bool,
    randomization_seed: Option<u64>,
    warmup_order_by_round: Vec<Vec<String>>,
    measured_order_by_round: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CleanupStatus {
    browser_closed: bool,
    profile_removed: bool,
    automation_resources_removed: bool,
    detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JetStreamFailure {
    stage: String,
    message: String,
    progress: Option<Box<ProgressDiagnostics>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProgressDiagnostics {
    state: String,
    initialized: bool,
    started: bool,
    completed_workloads: usize,
    total_workloads: usize,
    current_workload: Option<String>,
    #[serde(default)]
    subtest_scores: BTreeMap<String, f64>,
    #[serde(default)]
    document_visibility_state: Option<String>,
    document_url: Option<String>,
    document_generation: Option<u64>,
    status_text: String,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProcessDiagnostics {
    pid: u32,
    session_id: u32,
    process_start_time: Option<velocity_metrics::ProcessStartTime>,
    executable_path: String,
    session_process_count: usize,
    exit_status_observed: bool,
    exit_code: Option<i32>,
    exit_signal: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JetStreamIteration {
    iteration: usize,
    warmup: bool,
    started_at_unix_ms: u128,
    official_score: Option<f64>,
    score_unit: String,
    benchmark_version: Option<String>,
    document_url: Option<String>,
    document_generation: Option<u64>,
    completed_workloads: usize,
    total_workloads: usize,
    subtest_scores: BTreeMap<String, f64>,
    execution_duration_ms: Option<f64>,
    completion_verified: bool,
    failure: Option<JetStreamFailure>,
    process: Option<ProcessDiagnostics>,
    cleanup: CleanupStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BrowserSummary {
    valid_measured_runs: usize,
    failed_measured_runs: usize,
    median_score: Option<f64>,
    mean_score: Option<f64>,
    minimum_score: Option<f64>,
    maximum_score: Option<f64>,
    sample_standard_deviation: Option<f64>,
    coefficient_of_variation_percent: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BrowserResult {
    browser: ReportBrowser,
    engine_family: Option<String>,
    automation_method: String,
    iterations: Vec<JetStreamIteration>,
    summary: BrowserSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ComparisonAssessment {
    status: String,
    higher_is_better: bool,
    descriptive_ranking_allowed: bool,
    reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct JetStreamReport {
    schema_version: u32,
    benchmark: String,
    benchmark_version: String,
    source: SourceMetadata,
    configuration: BenchmarkConfiguration,
    environment: EnvironmentMetadata,
    execution_order: ExecutionOrder,
    browsers: Vec<BrowserResult>,
    comparison: ComparisonAssessment,
    cancelled: bool,
    generated_at_unix_ms: u128,
}

#[derive(Clone, Debug)]
struct AutomationOutcome {
    score: f64,
    completed_workloads: usize,
    subtest_scores: BTreeMap<String, f64>,
    execution_duration_ms: Option<f64>,
    document_url: Option<String>,
    document_generation: Option<u64>,
}

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|value| matches!(value.as_str(), "--help" | "-h"))
    {
        print_help();
        return Ok(());
    }
    run_benchmark(parse_options(arguments)?)
}

fn parse_options(arguments: Vec<String>) -> Result<Options, String> {
    let mut browser = None;
    let mut browsers = None;
    let mut all = false;
    let mut runs = DEFAULT_RUNS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut randomized = false;
    let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
    let mut source = SourceKind::Local;
    let mut output_directory = default_output_directory();
    let mut record_history = true;
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        if !seen.insert(option.clone()) {
            return Err(format!("{option} may be specified only once"));
        }
        match option.as_str() {
            "--browser" => browser = Some(required_value(&mut arguments, &option)?),
            "--browsers" => {
                let value = required_value(&mut arguments, &option)?;
                let values = value.split(',').map(str::trim).collect::<Vec<_>>();
                if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                    return Err(
                        "--browsers requires comma-separated browser identifiers".to_owned()
                    );
                }
                browsers = Some(values.into_iter().map(str::to_owned).collect());
            }
            "--all" => all = true,
            "--runs" => runs = parse_value(&mut arguments, &option)?,
            "--warmup" => warmups = parse_value(&mut arguments, &option)?,
            "--randomize" => randomized = true,
            "--timeout" => timeout_seconds = parse_value(&mut arguments, &option)?,
            "--benchmark-source" => {
                source = SourceKind::parse(&required_value(&mut arguments, &option)?)?
            }
            "--record-history" => record_history = true,
            "--no-record-history" => record_history = false,
            "--output" => {
                output_directory = PathBuf::from(required_value(&mut arguments, &option)?)
            }
            other => return Err(format!("unknown JetStream option '{other}'")),
        }
    }
    if seen.contains("--record-history") && seen.contains("--no-record-history") {
        return Err("choose only one of --record-history and --no-record-history".to_owned());
    }
    if !(1..=MAX_RUNS).contains(&runs) {
        return Err(format!("--runs must be between 1 and {MAX_RUNS}"));
    }
    if warmups > MAX_WARMUPS {
        return Err(format!("--warmup must be at most {MAX_WARMUPS}"));
    }
    if !(30..=7_200).contains(&timeout_seconds) {
        return Err("--timeout must be between 30 and 7200 seconds".to_owned());
    }
    let target = match (browser, browsers, all) {
        (Some(browser), None, false) => TargetSelection::One(browser),
        (None, Some(browsers), false) => TargetSelection::Many(browsers),
        (None, None, true) => TargetSelection::All,
        (None, None, false) => TargetSelection::Prompt,
        _ => return Err("use exactly one of --browser, --browsers, or --all".to_owned()),
    };
    Ok(Options {
        target,
        runs,
        warmups,
        randomized,
        timeout: Duration::from_secs(timeout_seconds),
        source,
        output_directory,
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

fn print_help() {
    crate::terminal::print_markdown(
        "## `velocity bench jetstream`\n\n**Usage:** `velocity bench jetstream [--browser <id|number> | --browsers <ids> | --all] [OPTIONS]`\n\nRuns the complete official JetStream 3.0 workload set in fresh, isolated, headed sessions. Velocity activates the benchmark's built-in Start Test control with default workload parameters and reads the official overall score from its completed results page.\n\nOptions: `--runs N` (5), `--warmup N` (1), `--randomize`, `--timeout SECONDS` (1800), `--benchmark-source local|official` (pinned local by default), `--output DIRECTORY`, `--record-history` / `--no-record-history` (history enabled by default). Local mode requires `velocity setup jetstream`.\n\nVerified automation adapters: CDP for Chrome and Helium, and the authenticated restricted Photon protocol. A completed score is recorded only after every workload in the complete default suite reports completion. Headless mode is not supported.",
    );
}

#[cfg(not(target_os = "macos"))]
fn run_benchmark(_options: Options) -> Result<(), String> {
    Err(
        "JetStream browser automation currently requires macOS browser lifecycle support"
            .to_owned(),
    )
}

#[cfg(target_os = "macos")]
fn run_benchmark(options: Options) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use velocity_browser::macos::{MacOSBrowserDiscovery, default_session_store_path};

    CANCELLED.store(false, Ordering::SeqCst);
    install_cancellation_handler()?;
    let registry = MacOSBrowserDiscovery
        .discover()
        .map_err(|error| error.to_string())?;
    let installations = resolve_installations(&registry, &options.target)?;
    if installations.is_empty() {
        return Err("no browser installations were selected".to_owned());
    }
    fs::create_dir_all(&options.output_directory).map_err(|error| {
        format!(
            "cannot create result directory {}: {error}",
            options.output_directory.display()
        )
    })?;
    let source_server = if options.source == SourceKind::Local {
        Some(crate::jetstream_source::LocalJetStreamServer::start(
            crate::jetstream_source::require_prepared_source()?,
        )?)
    } else {
        None
    };
    let source_url = source_server
        .as_ref()
        .map(|server| server.url().to_owned())
        .unwrap_or_else(official_url);
    let source = SourceMetadata {
        kind: options.source.name().to_owned(),
        url: source_url.clone(),
        version: JETSTREAM_VERSION.to_owned(),
        official_commit: (options.source == SourceKind::Local).then(|| JETSTREAM_COMMIT.to_owned()),
        archive_sha256: (options.source == SourceKind::Local)
            .then(|| JETSTREAM_ARCHIVE_SHA256.to_owned()),
    };
    let session_store_path = default_session_store_path()?;
    let profile_root = session_store_path
        .parent()
        .ok_or_else(|| "Velocity session store has no parent directory".to_owned())?
        .join("Profiles");
    fs::create_dir_all(&profile_root).map_err(|error| {
        format!(
            "cannot create Velocity profile root {}: {error}",
            profile_root.display()
        )
    })?;

    let run_id = unique_run_id();
    let seed = options.randomized.then(random_seed);
    let mut random = seed.unwrap_or_default();
    let mut warmup_order = Vec::new();
    let mut measured_order = Vec::new();
    let mut iterations_by_installation = installations
        .iter()
        .map(|installation| (installation.installation_id.to_string(), Vec::new()))
        .collect::<BTreeMap<_, _>>();

    println!("Velocity — JetStream 3.0\n");
    println!(
        "Browsers: {}",
        installations
            .iter()
            .map(|installation| installation.metadata.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "Source: {}{}",
        source.kind,
        source
            .official_commit
            .as_ref()
            .map(|commit| format!(" ({commit})"))
            .unwrap_or_default()
    );
    println!(
        "Runs: {} measured, {} warm-up; timeout: {} seconds; mode: headed\n",
        options.runs,
        options.warmups,
        options.timeout.as_secs()
    );

    let rounds = options
        .runs
        .checked_add(options.warmups)
        .ok_or_else(|| "run count overflow".to_owned())?;
    let mut iteration_number_by_installation = BTreeMap::<String, usize>::new();
    for round in 0..rounds {
        if CANCELLED.load(Ordering::Relaxed) {
            break;
        }
        let warmup = round < options.warmups;
        let mut order = installations.clone();
        if options.randomized {
            shuffle(&mut order, &mut random);
        }
        let mut round_order = Vec::new();
        for installation in order {
            if CANCELLED.load(Ordering::Relaxed) {
                break;
            }
            let key = installation.installation_id.to_string();
            let iteration_number = iteration_number_by_installation
                .entry(key.clone())
                .or_default();
            *iteration_number += 1;
            if warmup {
                println!("[warm-up {}] {}", round + 1, installation.metadata.name);
            } else {
                println!(
                    "[{}/{}] {}",
                    round - options.warmups + 1,
                    options.runs,
                    installation.metadata.name
                );
            }
            let iteration = execute_iteration(
                &installation,
                *iteration_number,
                warmup,
                &source_url,
                &profile_root,
                &session_store_path,
                options.timeout,
            );
            match (&iteration.failure, iteration.official_score) {
                (Some(failure), _) => {
                    println!("      Failed ({}): {}", failure.stage, failure.message)
                }
                (None, Some(score)) => println!("      Official score: {score:.2}"),
                (None, None) => println!("      Official score: N/A"),
            }
            if !iteration.cleanup.browser_closed
                || !iteration.cleanup.profile_removed
                || !iteration.cleanup.automation_resources_removed
            {
                println!(
                    "      Cleanup warning: {}",
                    iteration
                        .cleanup
                        .detail
                        .as_deref()
                        .unwrap_or("managed cleanup was incomplete")
                );
            }
            round_order.push(key.clone());
            iterations_by_installation
                .get_mut(&key)
                .expect("iteration map matches selected browsers")
                .push(iteration);
        }
        if !round_order.is_empty() {
            if warmup {
                warmup_order.push(round_order);
            } else {
                measured_order.push(round_order);
            }
        }
    }
    let cancelled = CANCELLED.load(Ordering::Relaxed);
    let browsers = installations
        .iter()
        .map(|installation| {
            browser_result(
                installation,
                iterations_by_installation
                    .remove(&installation.installation_id.to_string())
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    let comparison = assess_comparison(&browsers, options.runs);
    let report = JetStreamReport {
        schema_version: 1,
        benchmark: "jetstream".to_owned(),
        benchmark_version: JETSTREAM_VERSION.to_owned(),
        source,
        configuration: BenchmarkConfiguration {
            requested_runs: options.runs,
            warmup_runs: options.warmups,
            timeout_seconds: options.timeout.as_secs(),
            browser_mode: "headed".to_owned(),
            source_kind: options.source.name().to_owned(),
            workload_selection: "complete_default_suite".to_owned(),
            prefetch_resources: true,
        },
        environment: EnvironmentMetadata {
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
        },
        execution_order: ExecutionOrder {
            randomized: options.randomized,
            randomization_seed: seed,
            warmup_order_by_round: warmup_order,
            measured_order_by_round: measured_order,
        },
        browsers,
        comparison,
        cancelled,
        generated_at_unix_ms: now_unix_ms(),
    };
    print_summary(&report);
    let any_success = report
        .browsers
        .iter()
        .flat_map(|browser| &browser.iterations)
        .any(|iteration| {
            !iteration.warmup && iteration.completion_verified && iteration.official_score.is_some()
        });
    let (json_path, html_path) = write_reports(&options.output_directory, &run_id, &report)?;
    println!("\nSaved JSON: {}", display_path(&json_path));
    println!("Saved HTML: {}", display_path(&html_path));
    if options.record_history && any_success && !cancelled {
        crate::history_cli::record_after_success(&json_path);
    }
    drop(source_server);
    if cancelled {
        return Err(format!(
            "JetStream run was cancelled; diagnostics were saved to {} and {}",
            json_path.display(),
            html_path.display()
        ));
    }
    if !any_success {
        return Err(format!(
            "no JetStream measured run completed; diagnostics were saved to {} and {}",
            json_path.display(),
            html_path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn resolve_installations(
    registry: &velocity_browser::BrowserRegistry,
    target: &TargetSelection,
) -> Result<Vec<BrowserInstallation>, String> {
    let selected = match target {
        TargetSelection::One(selector) => {
            vec![crate::browser_selection::resolve(registry, selector)?.clone()]
        }
        TargetSelection::Many(selectors) => selectors
            .iter()
            .map(|selector| crate::browser_selection::resolve(registry, selector).cloned())
            .collect::<Result<Vec<_>, _>>()?,
        TargetSelection::All => registry.installations().cloned().collect(),
        TargetSelection::Prompt => crate::browser_selection::choose(registry, None, true)?
            .into_iter()
            .cloned()
            .collect(),
    };
    let mut unique = Vec::new();
    for installation in selected {
        if !unique.iter().any(|entry: &BrowserInstallation| {
            entry.installation_id == installation.installation_id
        }) {
            unique.push(installation);
        }
    }
    Ok(unique)
}

fn is_supported(installation: &BrowserInstallation) -> bool {
    matches!(
        installation.metadata.id.as_str(),
        "photon" | "chrome" | "helium"
    )
}

#[cfg(target_os = "macos")]
fn execute_iteration(
    installation: &BrowserInstallation,
    iteration: usize,
    warmup: bool,
    source_url: &str,
    profile_root: &Path,
    session_store_path: &Path,
    timeout: Duration,
) -> JetStreamIteration {
    use velocity_browser::{
        BrowserLifecycleManager, FileSessionStore, ProcessLaunchTiming,
        macos::{MacOSProcessDriver, MacOSWindowObserver},
    };
    use velocity_core::{CancellationToken, StartupMetric, observe_startup_windows};

    let mut result = JetStreamIteration {
        iteration,
        warmup,
        started_at_unix_ms: now_unix_ms(),
        official_score: None,
        score_unit: "official JetStream 3.0 score (higher is better)".to_owned(),
        benchmark_version: None,
        document_url: None,
        document_generation: None,
        completed_workloads: 0,
        total_workloads: 0,
        subtest_scores: BTreeMap::new(),
        execution_duration_ms: None,
        completion_verified: false,
        failure: None,
        process: None,
        cleanup: CleanupStatus {
            browser_closed: true,
            profile_removed: true,
            automation_resources_removed: true,
            detail: None,
        },
    };
    if !is_supported(installation) {
        result.failure = Some(failure(
            "capability",
            format!("{} has no JetStream adapter", installation.metadata.name),
            None,
        ));
        return result;
    }
    if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
        result.failure = Some(failure(
            "cancelled",
            "cancelled before browser launch",
            None,
        ));
        return result;
    }

    let photon = installation.metadata.id.as_str() == "photon";
    let token = if photon {
        match secure_token() {
            Ok(token) => Some(token),
            Err(error) => {
                result.failure = Some(failure("automation_setup", error, None));
                return result;
            }
        }
    } else {
        None
    };
    let socket_path = token.as_ref().map(|token| {
        PathBuf::from("/tmp")
            .join(format!("vlt-{}", &token[..12]))
            .join("p.sock")
    });
    let automation_directory = socket_path
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    let mut driver = MacOSProcessDriver::new(profile_root.to_path_buf());
    if installation.metadata.engine == Some(BrowserEngine::Chromium) {
        driver = driver.with_chromium_devtools();
    }
    if let (Some(socket), Some(token)) = (&socket_path, &token) {
        driver = driver.with_photon_automation(socket.clone(), token.clone());
    }
    let manager = BrowserLifecycleManager::new(
        driver.clone(),
        FileSessionStore::new(session_store_path.to_path_buf()),
    );
    let mut launch_timing: Option<ProcessLaunchTiming> = None;
    let session = match manager.launch_observed(installation, Some("about:blank"), &mut |timing| {
        launch_timing = Some(timing)
    }) {
        Ok(session) => session,
        Err(error) => {
            result.failure = Some(failure("browser_launch", error.to_string(), None));
            cleanup_photon_directory(
                automation_directory.as_deref(),
                token.as_deref(),
                &mut result.cleanup,
            );
            return result;
        }
    };
    result.cleanup.browser_closed = false;
    result.cleanup.profile_removed = session
        .identity
        .profile_path
        .as_deref()
        .is_none_or(|path| !path.exists());
    let benchmark_start = Instant::now();
    let automation_result = (|| -> Result<AutomationOutcome, JetStreamFailure> {
        let timing = launch_timing.as_ref().ok_or_else(|| {
            failure(
                "browser_launch",
                "process driver omitted launch timing",
                None,
            )
        })?;
        let cancellation = CancellationToken::default();
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            cancellation.cancel();
        }
        let window = observe_startup_windows(
            timing,
            &MacOSWindowObserver,
            Duration::from_secs(30),
            &cancellation,
        );
        if !window
            .measurements_ms
            .contains_key(&StartupMetric::WindowReady)
        {
            return Err(failure(
                "window_readiness",
                window
                    .failures
                    .first()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| "browser window did not become ready".to_owned()),
                None,
            ));
        }
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(failure(
                "cancelled",
                "cancelled before benchmark navigation",
                None,
            ));
        }
        if photon {
            run_photon_jetstream(
                socket_path.as_deref().ok_or_else(|| {
                    failure("automation_setup", "Photon socket was not prepared", None)
                })?,
                source_url,
                timeout,
            )
        } else {
            let profile = session.identity.profile_path.as_deref().ok_or_else(|| {
                failure(
                    "automation_setup",
                    "Chromium session has no isolated profile",
                    None,
                )
            })?;
            run_chromium_jetstream(profile, source_url, timeout)
        }
    })();
    match automation_result {
        Ok(outcome) => {
            result.official_score = Some(outcome.score);
            result.benchmark_version = Some(JETSTREAM_VERSION.to_owned());
            result.completed_workloads = outcome.completed_workloads;
            result.total_workloads = outcome.completed_workloads;
            result.subtest_scores = outcome.subtest_scores;
            result.execution_duration_ms = outcome.execution_duration_ms;
            result.document_url = outcome.document_url;
            result.document_generation = outcome.document_generation;
            result.completion_verified = true;
        }
        Err(failure) => result.failure = Some(failure),
    }
    match driver.diagnostics(&session.identity) {
        Ok(diagnostics) => {
            result.process = Some(ProcessDiagnostics {
                pid: session.identity.pid,
                session_id: session.identity.session_id,
                process_start_time: session.identity.start_time,
                executable_path: session
                    .identity
                    .executable_path
                    .to_string_lossy()
                    .into_owned(),
                session_process_count: diagnostics.session_process_count,
                exit_status_observed: diagnostics.exit_status_observed,
                exit_code: diagnostics.exit_code,
                exit_signal: diagnostics.exit_signal,
            });
            if let Some(current_failure) = result.failure.as_mut()
                && diagnostics.exit_status_observed
                && current_failure.stage != "cancelled"
            {
                current_failure.stage = "browser_process_exit".to_owned();
                current_failure.message = format!(
                    "browser process exited before verified JetStream completion (code {:?}, signal {:?}); {}",
                    diagnostics.exit_code, diagnostics.exit_signal, current_failure.message
                );
            }
        }
        Err(error) => {
            result.cleanup.detail = Some(format!("could not collect process diagnostics: {error}"));
        }
    }
    result.cleanup.detail = result.cleanup.detail.take().or_else(|| {
        (!result.completion_verified).then(|| {
            format!(
                "session finished after {:.1} seconds",
                benchmark_start.elapsed().as_secs_f64()
            )
        })
    });
    match manager.close(&session.session_id) {
        Ok(_) => {
            result.cleanup.browser_closed = true;
            result.cleanup.profile_removed = session
                .identity
                .profile_path
                .as_deref()
                .is_none_or(|path| !path.exists());
        }
        Err(error) => {
            result.cleanup.browser_closed = false;
            result.cleanup.detail = Some(format!("managed browser close failed: {error}"));
        }
    }
    cleanup_photon_directory(
        automation_directory.as_deref(),
        token.as_deref(),
        &mut result.cleanup,
    );
    if (!result.cleanup.browser_closed
        || !result.cleanup.profile_removed
        || !result.cleanup.automation_resources_removed)
        && result.failure.is_none()
    {
        result.failure = Some(failure(
            "cleanup",
            result
                .cleanup
                .detail
                .clone()
                .unwrap_or_else(|| "managed browser resources remain after cleanup".to_owned()),
            None,
        ));
        result.completion_verified = false;
        result.official_score = None;
    }
    result
}

#[cfg(not(target_os = "macos"))]
fn execute_iteration(
    installation: &BrowserInstallation,
    iteration: usize,
    warmup: bool,
    _source_url: &str,
    _profile_root: &Path,
    _session_store_path: &Path,
    _timeout: Duration,
) -> JetStreamIteration {
    let mut result = JetStreamIteration {
        iteration,
        warmup,
        started_at_unix_ms: now_unix_ms(),
        official_score: None,
        score_unit: "official JetStream 3.0 score (higher is better)".to_owned(),
        benchmark_version: None,
        document_url: None,
        document_generation: None,
        completed_workloads: 0,
        total_workloads: 0,
        subtest_scores: BTreeMap::new(),
        execution_duration_ms: None,
        completion_verified: false,
        failure: Some(failure(
            "unsupported_platform",
            "managed browser automation is supported only on macOS",
            None,
        )),
        process: None,
        cleanup: CleanupStatus {
            browser_closed: true,
            profile_removed: true,
            automation_resources_removed: true,
            detail: None,
        },
    };
    if !is_supported(installation) {
        result.failure = Some(failure("capability", "no verified JetStream adapter", None));
    }
    result
}

fn failure(
    stage: &str,
    message: impl Into<String>,
    progress: Option<ProgressDiagnostics>,
) -> JetStreamFailure {
    JetStreamFailure {
        stage: stage.to_owned(),
        message: message.into().chars().take(1024).collect(),
        progress: progress.map(Box::new),
    }
}

#[cfg(target_os = "macos")]
fn run_chromium_jetstream(
    profile: &Path,
    source_url: &str,
    timeout: Duration,
) -> Result<AutomationOutcome, JetStreamFailure> {
    use velocity_browser::ChromiumDevTools;

    let mut devtools = ChromiumDevTools::connect(profile, Duration::from_secs(30))
        .map_err(|error| failure("automation_connect", error.to_string(), None))?;
    let target = devtools
        .navigate_initial_page(source_url)
        .map_err(|error| failure("navigation", error, None))?;
    devtools
        .bring_page_to_front(&target)
        .map_err(|error| failure("window_activation", error.to_string(), None))?;
    let started_at = Instant::now();
    let deadline = started_at + timeout;
    let ready = wait_for_chromium_jetstream_state(
        &mut devtools,
        &target,
        source_url,
        deadline,
        "initialization",
        JetStreamPageStatus::ready,
    )?;
    if !ready.started && !ready.completed {
        devtools.start_jetstream(&target).map_err(|error| {
            failure("benchmark_start", error.to_string(), Some(progress(&ready)))
        })?;
    }
    let started = if ready.started || ready.completed {
        ready
    } else {
        wait_for_chromium_jetstream_state(
            &mut devtools,
            &target,
            source_url,
            deadline,
            "start",
            |status| status.started || status.completed,
        )?
    };
    let execution_started = Instant::now();
    let completed = if started.completed {
        started
    } else {
        wait_for_chromium_jetstream_state(
            &mut devtools,
            &target,
            source_url,
            deadline,
            "completion",
            |status| status.completed,
        )?
    };
    outcome_from_status(
        completed,
        Some(execution_started.elapsed().as_secs_f64() * 1000.0),
    )
}

#[cfg(target_os = "macos")]
fn wait_for_chromium_jetstream_state(
    devtools: &mut velocity_browser::ChromiumDevTools,
    target_id: &str,
    expected_url: &str,
    deadline: Instant,
    phase: &str,
    condition: impl Fn(&JetStreamPageStatus) -> bool,
) -> Result<JetStreamPageStatus, JetStreamFailure> {
    let mut last_progress = None;
    let mut last_error = None;
    let mut last_visibility_restore = None;
    loop {
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(failure(
                "cancelled",
                format!("cancelled while waiting for JetStream {phase}"),
                last_progress,
            ));
        }
        if Instant::now() >= deadline {
            let stage = if last_progress
                .as_ref()
                .and_then(|progress: &ProgressDiagnostics| {
                    progress.document_visibility_state.as_deref()
                })
                == Some("hidden")
            {
                "document_hidden"
            } else {
                match phase {
                    "initialization" => "benchmark_initialization",
                    "start" => "benchmark_start",
                    _ => "benchmark_execution",
                }
            };
            return Err(failure(
                stage,
                format!(
                    "JetStream did not reach verified {phase} state before timeout{}",
                    last_error
                        .as_deref()
                        .map(|error| format!("; last observation error: {error}"))
                        .unwrap_or_default()
                ),
                last_progress,
            ));
        }
        match devtools.jetstream_status(target_id) {
            Ok(status) => {
                if document_url(&status.url) != document_url(expected_url) {
                    return Err(failure(
                        "navigation_replaced",
                        format!(
                            "JetStream page navigated to an unexpected document URL: {}",
                            status.url
                        ),
                        Some(progress(&status)),
                    ));
                }
                if status.failed {
                    let browser_diagnostics = devtools.page_diagnostics().join("; ");
                    let detail = if browser_diagnostics.is_empty() {
                        describe_status(&status)
                    } else {
                        format!(
                            "{}; browser diagnostics: {browser_diagnostics}",
                            describe_status(&status)
                        )
                    };
                    return Err(failure(
                        if status.errors.is_empty() {
                            "benchmark_failure"
                        } else {
                            "javascript_exception"
                        },
                        detail,
                        Some(progress(&status)),
                    ));
                }
                if condition(&status) {
                    return Ok(status);
                }
                if status.document_visibility_state.as_deref() == Some("hidden")
                    && visibility_restore_due(last_visibility_restore, Instant::now())
                {
                    devtools.bring_page_to_front(target_id).map_err(|error| {
                        failure(
                            "window_activation",
                            error.to_string(),
                            Some(progress(&status)),
                        )
                    })?;
                    last_visibility_restore = Some(Instant::now());
                }
                last_progress = Some(progress(&status));
                last_error = None;
            }
            Err(error) => {
                let message = error.to_string();
                if is_disconnect(&message) {
                    return Err(failure(
                        "automation_disconnect",
                        format!(
                            "CDP connection was lost while waiting for JetStream {phase}: {message}"
                        ),
                        last_progress,
                    ));
                }
                last_error = Some(message);
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn visibility_restore_due(last_restore: Option<Instant>, now: Instant) -> bool {
    last_restore.is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(1))
}

fn is_disconnect(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "connection reset",
        "connection refused",
        "connection closed",
        "broken pipe",
        "unexpected eof",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

#[cfg(target_os = "macos")]
fn run_photon_jetstream(
    socket_path: &Path,
    source_url: &str,
    timeout: Duration,
) -> Result<AutomationOutcome, JetStreamFailure> {
    use velocity_browser::{PhotonAutomationClient, parse_jetstream_result_title};

    let credential_path = socket_path
        .parent()
        .ok_or_else(|| failure("automation_setup", "Photon socket has no parent", None))?
        .join("token");
    let mut client = PhotonAutomationClient::connect_with_cancel(
        socket_path,
        credential_path,
        Duration::from_secs(30),
        || CANCELLED.load(std::sync::atomic::Ordering::Relaxed),
    )
    .map_err(|error| failure("automation_connect", error.to_string(), None))?;
    if !client.supports("probe_jetstream_result") {
        return Err(failure(
            "unsupported",
            "Photon protocol does not advertise the restricted JetStream 3.0 result observer",
            None,
        ));
    }
    let tabs = client
        .list_tabs(Duration::from_secs(5))
        .map_err(|error| failure("tab_discovery", error.to_string(), None))?;
    let [tab] = tabs.as_slice() else {
        return Err(failure(
            "tab_discovery",
            format!(
                "isolated Photon session expected one initial tab, found {}",
                tabs.len()
            ),
            None,
        ));
    };
    let tab_id = tab.tab_id;
    client
        .activate_tab(tab_id, Duration::from_secs(5))
        .map_err(|error| failure("window_activation", error.to_string(), None))?;
    client
        .navigate(tab_id, source_url, Duration::from_secs(10))
        .map_err(|error| failure("navigation", error.to_string(), None))?;
    let deadline = Instant::now() + timeout;
    let mut last_tab_state = "no Photon tab status received".to_owned();
    let document_generation = loop {
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(failure(
                "cancelled",
                "cancelled during JetStream navigation",
                None,
            ));
        }
        if Instant::now() >= deadline {
            return Err(failure(
                "page_readiness",
                format!(
                    "Photon did not report the JetStream document load before timeout; last tab state: {last_tab_state}"
                ),
                None,
            ));
        }
        let status = client
            .tab_status(tab_id, Duration::from_secs(5))
            .map_err(|error| failure("tab_status", error.to_string(), None))?;
        let url_matches = status.url.as_deref().map(document_url) == Some(document_url(source_url));
        let title_matches = status
            .title
            .as_deref()
            .is_some_and(|title| title.contains("JetStream 3.0"));
        last_tab_state = format!(
            "url_matches={url_matches}, loading={}, engine_load_finished={}, generation={}, title_matches={title_matches}, error_state={}",
            status
                .loading
                .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
            status
                .engine_load_finished
                .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
            status
                .document_generation
                .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
            status.error_state.as_deref().unwrap_or("none")
        );
        if status.error_state.as_deref() == Some("page_crashed") {
            return Err(failure(
                "browser_process_exit",
                format!("Photon reports a crashed controlled tab: {last_tab_state}"),
                None,
            ));
        }
        if url_matches
            && status.loading == Some(false)
            && status.engine_load_finished == Some(true)
            && title_matches
        {
            break status.document_generation.ok_or_else(|| {
                failure(
                    "page_readiness",
                    "Photon omitted the loaded JetStream document generation",
                    None,
                )
            })?;
        }
        thread::sleep(Duration::from_millis(250));
    };
    client
        .install_jetstream_result_observer(tab_id, document_generation, Duration::from_secs(10))
        .map_err(|error| failure("result_observer", error.to_string(), None))?;

    let mut started_at = None;
    let mut start_requested = false;
    let mut last_status = "observer has not published JetStream state".to_owned();
    let mut last_progress = None;
    loop {
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(failure(
                "cancelled",
                "cancelled during JetStream execution",
                last_progress,
            ));
        }
        if Instant::now() >= deadline {
            return Err(failure(
                "benchmark_execution",
                format!(
                    "Photon did not reach verified JetStream completion before timeout; last state: {last_status}"
                ),
                last_progress,
            ));
        }
        let status = client
            .tab_status(tab_id, Duration::from_secs(5))
            .map_err(|error| failure("tab_status", error.to_string(), last_progress.clone()))?;
        let Some(observer) =
            parse_jetstream_result_title(status.title.as_deref()).map_err(|error| {
                failure("score_extraction", error.to_string(), last_progress.clone())
            })?
        else {
            last_status = format!(
                "observer title missing; url={}",
                status.url.as_deref().unwrap_or("unknown")
            );
            thread::sleep(Duration::from_millis(250));
            continue;
        };
        if observer.document_generation != Some(document_generation)
            || observer.document_url.as_deref().map(document_url) != Some(document_url(source_url))
        {
            return Err(failure(
                "navigation_replaced",
                "Photon JetStream observer belongs to a different document generation or URL",
                last_progress,
            ));
        }
        last_progress = Some(ProgressDiagnostics {
            state: observer.state.clone(),
            initialized: observer.initialized,
            started: observer.started,
            completed_workloads: observer.completed_workloads,
            total_workloads: observer.total_workloads,
            current_workload: observer.current_workload.clone(),
            subtest_scores: observer.subtest_scores.clone(),
            document_visibility_state: None,
            document_url: observer.document_url.clone(),
            document_generation: observer.document_generation,
            status_text: observer.status_text.clone(),
            errors: observer.errors.clone(),
        });
        last_status = describe_progress_diagnostics(last_progress.as_ref().expect("progress set"));
        if observer.failed {
            return Err(failure(
                if observer.errors.is_empty() {
                    "benchmark_failure"
                } else {
                    "javascript_exception"
                },
                observer
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| observer.status_text.clone()),
                last_progress,
            ));
        }
        if observer.initialized && !observer.started && !observer.completed && !start_requested {
            client
                .request_jetstream_start(tab_id, document_generation, Duration::from_secs(10))
                .map_err(|error| {
                    failure("benchmark_start", error.to_string(), last_progress.clone())
                })?;
            start_requested = true;
        }
        if observer.started {
            started_at.get_or_insert_with(Instant::now);
        }
        if observer.completed {
            let elapsed = started_at.map(|started| started.elapsed().as_secs_f64() * 1000.0);
            return outcome_from_photon(observer, elapsed);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn outcome_from_status(
    status: JetStreamPageStatus,
    execution_duration_ms: Option<f64>,
) -> Result<AutomationOutcome, JetStreamFailure> {
    if status.failed {
        return Err(failure(
            "benchmark_failure",
            describe_status(&status),
            Some(progress(&status)),
        ));
    }
    if !status.completed {
        return Err(failure(
            "benchmark_completion",
            "JetStream completion was not verified",
            Some(progress(&status)),
        ));
    }
    let score = status
        .official_score
        .filter(|score| score.is_finite() && *score > 0.0)
        .ok_or_else(|| {
            failure(
                "invalid_result",
                "official score is missing or invalid",
                Some(progress(&status)),
            )
        })?;
    if status.total_workloads == 0
        || status.completed_workloads != status.total_workloads
        || status.subtest_scores.len() != status.total_workloads
    {
        return Err(failure(
            "invalid_result",
            format!(
                "official completion omitted workload scores ({}/{}, {} scores)",
                status.completed_workloads,
                status.total_workloads,
                status.subtest_scores.len()
            ),
            Some(progress(&status)),
        ));
    }
    Ok(AutomationOutcome {
        score,
        completed_workloads: status.total_workloads,
        subtest_scores: status.subtest_scores,
        execution_duration_ms,
        document_url: Some(status.url),
        document_generation: None,
    })
}

#[cfg(target_os = "macos")]
fn outcome_from_photon(
    status: velocity_browser::PhotonJetStreamResult,
    execution_duration_ms: Option<f64>,
) -> Result<AutomationOutcome, JetStreamFailure> {
    if !status.completed || status.failed {
        return Err(failure(
            "benchmark_completion",
            "Photon result observer did not verify complete JetStream execution",
            Some(ProgressDiagnostics {
                state: status.state,
                initialized: status.initialized,
                started: status.started,
                completed_workloads: status.completed_workloads,
                total_workloads: status.total_workloads,
                current_workload: status.current_workload,
                subtest_scores: status.subtest_scores,
                document_visibility_state: None,
                document_url: status.document_url,
                document_generation: status.document_generation,
                status_text: status.status_text,
                errors: status.errors,
            }),
        ));
    }
    let score = status
        .official_score
        .filter(|score| score.is_finite() && *score > 0.0)
        .ok_or_else(|| {
            failure(
                "invalid_result",
                "Photon returned a missing or invalid official JetStream score",
                None,
            )
        })?;
    if status.total_workloads == 0
        || status.completed_workloads != status.total_workloads
        || status.subtest_scores.len() != status.total_workloads
    {
        return Err(failure(
            "invalid_result",
            "Photon observer did not return every official workload score",
            None,
        ));
    }
    Ok(AutomationOutcome {
        score,
        completed_workloads: status.total_workloads,
        subtest_scores: status.subtest_scores,
        execution_duration_ms,
        document_url: status.document_url,
        document_generation: status.document_generation,
    })
}

fn progress(status: &JetStreamPageStatus) -> ProgressDiagnostics {
    ProgressDiagnostics {
        state: (if status.completed {
            "complete"
        } else if status.failed {
            "failed"
        } else if status.started {
            "running"
        } else if status.initialized {
            "ready"
        } else {
            "loading"
        })
        .to_owned(),
        initialized: status.initialized,
        started: status.started,
        completed_workloads: status.completed_workloads,
        total_workloads: status.total_workloads,
        current_workload: status.current_workload.clone(),
        subtest_scores: status.subtest_scores.clone(),
        document_visibility_state: status.document_visibility_state.clone(),
        document_url: Some(status.url.clone()),
        document_generation: None,
        status_text: status.status_text.clone(),
        errors: status.errors.clone(),
    }
}

fn describe_status(status: &JetStreamPageStatus) -> String {
    let progress = progress(status);
    let mut description = describe_progress_diagnostics(&progress);
    if let Some(error) = status.errors.first() {
        description.push_str(&format!("; error={error}"));
    }
    description
}

fn describe_progress_diagnostics(progress: &ProgressDiagnostics) -> String {
    format!(
        "state={}, initialized={}, started={}, visibility={}, workloads={}/{}, current={}, status={}",
        progress.state,
        progress.initialized,
        progress.started,
        progress
            .document_visibility_state
            .as_deref()
            .unwrap_or("unknown"),
        progress.completed_workloads,
        progress.total_workloads,
        progress.current_workload.as_deref().unwrap_or("none"),
        progress.status_text
    )
}

fn document_url(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_owned()
}

fn browser_result(
    installation: &BrowserInstallation,
    iterations: Vec<JetStreamIteration>,
) -> BrowserResult {
    let scores = iterations
        .iter()
        .filter(|iteration| !iteration.warmup && iteration.completion_verified)
        .filter_map(|iteration| iteration.official_score)
        .filter(|score| score.is_finite() && *score > 0.0)
        .collect::<Vec<_>>();
    let stats = summarize_metric_values(scores.iter().copied());
    BrowserResult {
        browser: ReportBrowser::from(installation),
        engine_family: installation.metadata.engine.map(engine_name),
        automation_method: match installation.metadata.id.as_str() {
            "photon" => {
                "Photon authenticated Unix socket (restricted JetStream observer)".to_owned()
            }
            "chrome" | "helium" => "Chrome DevTools Protocol (isolated profile)".to_owned(),
            _ => "unsupported".to_owned(),
        },
        summary: BrowserSummary {
            valid_measured_runs: scores.len(),
            failed_measured_runs: iterations
                .iter()
                .filter(|iteration| !iteration.warmup && iteration.failure.is_some())
                .count(),
            median_score: stats.as_ref().map(|value| value.median),
            mean_score: stats.as_ref().map(|value| value.mean),
            minimum_score: stats.as_ref().map(|value| value.minimum),
            maximum_score: stats.as_ref().map(|value| value.maximum),
            sample_standard_deviation: stats
                .as_ref()
                .and_then(|value| value.sample_standard_deviation),
            coefficient_of_variation_percent: stats
                .as_ref()
                .and_then(|value| value.coefficient_of_variation_percent),
        },
        iterations,
    }
}

fn engine_name(engine: BrowserEngine) -> String {
    match engine {
        BrowserEngine::Chromium => "Chromium",
        BrowserEngine::Gecko => "Gecko",
        BrowserEngine::WebKit => "WebKit",
        BrowserEngine::Ladybird => "Ladybird",
    }
    .to_owned()
}

fn assess_comparison(browsers: &[BrowserResult], requested_runs: usize) -> ComparisonAssessment {
    let mut reasons = Vec::new();
    if browsers.len() < 2 {
        reasons.push("comparison requires at least two selected browser installations".to_owned());
    }
    if browsers
        .iter()
        .any(|browser| browser.summary.valid_measured_runs < MIN_COMPARISON_RUNS)
    {
        reasons.push(format!(
            "each browser needs at least {MIN_COMPARISON_RUNS} valid measured runs"
        ));
    }
    if browsers
        .iter()
        .any(|browser| browser.summary.valid_measured_runs < requested_runs)
    {
        reasons.push("one or more browsers have failed or unsupported measured runs".to_owned());
    }
    let comparable = reasons.is_empty();
    ComparisonAssessment {
        status: if comparable {
            "COMPARABLE"
        } else {
            "INCONCLUSIVE"
        }
        .to_owned(),
        higher_is_better: true,
        descriptive_ranking_allowed: comparable,
        reasons,
    }
}

fn print_summary(report: &JetStreamReport) {
    crate::terminal::print_markdown(&format!(
        "## Comparison status: {}\n\n**Higher score is better.** Scores are read from JetStream's own completed results page.",
        report.comparison.status
    ));
    let rows = report
        .browsers
        .iter()
        .map(|browser| {
            vec![
                browser.browser.name.clone(),
                optional_score(browser.summary.median_score),
                optional_score(browser.summary.mean_score),
                optional_score(browser.summary.sample_standard_deviation),
                format!(
                    "{}/{}",
                    browser.summary.valid_measured_runs,
                    browser.summary.valid_measured_runs + browser.summary.failed_measured_runs
                ),
            ]
        })
        .collect::<Vec<_>>();
    crate::terminal::print_table(
        &["Browser", "Median", "Mean", "Std Dev", "Valid/Measured"],
        rows,
        &[],
    );
    for reason in &report.comparison.reasons {
        println!("Warning: {reason}");
    }
    for browser in &report.browsers {
        for iteration in browser
            .iterations
            .iter()
            .filter(|iteration| iteration.failure.is_some())
        {
            let failure = iteration.failure.as_ref().expect("filtered failure");
            println!(
                "{} {} run {} failed at {}: {}",
                browser.browser.name,
                if iteration.warmup {
                    "warm-up"
                } else {
                    "measured"
                },
                iteration.iteration,
                failure.stage,
                failure.message
            );
        }
    }
}

fn optional_score(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.2}"))
}

fn write_reports(
    directory: &Path,
    run_id: &str,
    report: &JetStreamReport,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create result directory {}: {error}",
            directory.display()
        )
    })?;
    let json = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("cannot serialize JetStream report: {error}"))?;
    let html = render_html(report);
    let json_path = directory.join(format!("jetstream-{run_id}.json"));
    let html_path = directory.join(format!("jetstream-{run_id}.html"));
    write_new_file(&json_path, &json)?;
    if let Err(error) = write_new_file(&html_path, html.as_bytes()) {
        return Err(format!(
            "{error}; JSON report was saved to {}",
            json_path.display()
        ));
    }
    Ok((json_path, html_path))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create unique report {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write report {}: {error}", path.display()))
}

fn render_html(report: &JetStreamReport) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Velocity — JetStream 3.0</title><style>:root{color-scheme:light dark;--surface:Canvas;--text:CanvasText;--line:color-mix(in srgb,CanvasText 18%,Canvas);--accent:#5078a8;--danger:#b64d4d}body{font:15px system-ui,sans-serif;max-width:1100px;margin:32px auto;padding:0 20px;color:var(--text);background:var(--surface)}.card{border:1px solid var(--line);border-radius:10px;padding:18px;margin:18px 0;overflow:auto}table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}th,td{text-align:left;padding:9px 10px;border-bottom:1px solid var(--line)}svg{max-width:100%;height:auto}.muted{opacity:.72}code{overflow-wrap:anywhere}.failure{color:var(--danger)}.status{font-weight:700}</style><body>",
    );
    let _ = write!(
        html,
        "<h1>Velocity — JetStream {}</h1><p class=\"status\">Comparison: {}. Higher scores are better.</p>",
        escape_html(&report.benchmark_version),
        escape_html(&report.comparison.status)
    );
    let _ = write!(
        html,
        "<section class=\"card\"><h2>Benchmark source</h2><p>Source: {}<br>Commit: {}<br>Archive SHA-256: {}<br>Workload selection: {}<br>Measured runs: {}<br>Warm-ups: {}<br>Mode: {}<br>OS: {} ({})</p><p><code>{}</code></p>",
        escape_html(&report.source.kind),
        report
            .source
            .official_commit
            .as_deref()
            .map(escape_html)
            .unwrap_or_else(|| "unknown (hosted source)".to_owned()),
        report
            .source
            .archive_sha256
            .as_deref()
            .map(escape_html)
            .unwrap_or_else(|| "unknown".to_owned()),
        escape_html(&report.configuration.workload_selection),
        report.configuration.requested_runs,
        report.configuration.warmup_runs,
        escape_html(&report.configuration.browser_mode),
        escape_html(&report.environment.operating_system),
        escape_html(&report.environment.architecture),
        escape_html(&report.source.url)
    );
    for reason in &report.comparison.reasons {
        let _ = write!(
            html,
            "<p class=\"muted\">Comparison warning: {}</p>",
            escape_html(reason)
        );
    }
    html.push_str("</section><section class=\"card\"><h2>Summary scores</h2><table><thead><tr><th>Browser</th><th>Version</th><th>Median</th><th>Mean</th><th>Std dev</th><th>Valid</th><th>Failed</th></tr></thead><tbody>");
    for browser in &report.browsers {
        let _ = write!(
            html,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&browser.browser.name),
            escape_html(browser.browser.version.as_deref().unwrap_or("unknown")),
            optional_score(browser.summary.median_score),
            optional_score(browser.summary.mean_score),
            optional_score(browser.summary.sample_standard_deviation),
            browser.summary.valid_measured_runs,
            browser.summary.failed_measured_runs
        );
    }
    html.push_str("</tbody></table></section>");
    append_score_chart(&mut html, &report.browsers);
    html.push_str("<section class=\"card\"><h2>Runs and diagnostics</h2>");
    for browser in &report.browsers {
        let _ = write!(
            html,
            "<h3>{}</h3><p class=\"muted\">{}<br>{}</p><table><thead><tr><th>Kind</th><th>Run</th><th>Score</th><th>Workloads</th><th>Duration</th><th>Completion</th><th>Cleanup</th></tr></thead><tbody>",
            escape_html(&browser.browser.name),
            escape_html(&browser.automation_method),
            escape_html(browser.browser.executable_path.to_string_lossy().as_ref())
        );
        for iteration in &browser.iterations {
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                if iteration.warmup {
                    "warm-up"
                } else {
                    "measured"
                },
                iteration.iteration,
                optional_score(iteration.official_score),
                iteration.completed_workloads,
                iteration.total_workloads,
                iteration
                    .execution_duration_ms
                    .map_or_else(|| "N/A".to_owned(), |value| format!("{value:.0} ms")),
                if iteration.completion_verified {
                    "verified"
                } else {
                    "incomplete"
                },
                if iteration.cleanup.browser_closed
                    && iteration.cleanup.profile_removed
                    && iteration.cleanup.automation_resources_removed
                {
                    "clean"
                } else {
                    "warning"
                }
            );
            if let Some(failure) = &iteration.failure {
                let _ = write!(
                    html,
                    "<tr class=\"failure\"><td colspan=\"7\">{}: {}</td></tr>",
                    escape_html(&failure.stage),
                    escape_html(&failure.message)
                );
                if let Some(progress) = &failure.progress {
                    let _ = write!(
                        html,
                        "<tr><td colspan=\"7\">Progress: {}</td></tr>",
                        escape_html(&describe_progress_diagnostics(progress))
                    );
                }
            }
            let subtest_scores = if iteration.subtest_scores.is_empty() {
                iteration
                    .failure
                    .as_ref()
                    .and_then(|failure| failure.progress.as_ref())
                    .map(|progress| &progress.subtest_scores)
            } else {
                Some(&iteration.subtest_scores)
            };
            if let Some(subtest_scores) = subtest_scores.filter(|scores| !scores.is_empty()) {
                let _ = write!(
                    html,
                    "<tr><td colspan=\"7\"><details><summary>{} official subtest scores</summary><table><thead><tr><th>Workload</th><th>Score</th></tr></thead><tbody>",
                    subtest_scores.len()
                );
                for (name, score) in subtest_scores {
                    let _ = write!(
                        html,
                        "<tr><td>{}</td><td>{score:.2}</td></tr>",
                        escape_html(name)
                    );
                }
                html.push_str("</tbody></table></details></td></tr>");
            }
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</section><p class=\"muted\">Scores are read from JetStream 3.0's own result UI. Velocity does not recalculate or replace the official score.</p></body></html>");
    html
}

fn append_score_chart(html: &mut String, browsers: &[BrowserResult]) {
    let values = browsers
        .iter()
        .filter_map(|browser| {
            let scores = browser
                .iterations
                .iter()
                .filter(|iteration| !iteration.warmup && iteration.completion_verified)
                .filter_map(|iteration| {
                    iteration
                        .official_score
                        .map(|score| (iteration.iteration, score))
                })
                .collect::<Vec<_>>();
            (!scores.is_empty()).then_some((browser, scores))
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let maximum = values
        .iter()
        .flat_map(|(_, scores)| scores.iter().map(|(_, score)| *score))
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let height = 60 + values.len() * 58;
    let _ = write!(
        html,
        "<section class=\"card\"><h2>Measured run scores</h2><svg role=\"img\" aria-label=\"JetStream run scores, higher is better\" viewBox=\"0 0 900 {height}\" xmlns=\"http://www.w3.org/2000/svg\"><text x=\"10\" y=\"18\">Score (higher is better)</text>"
    );
    for (index, (browser, scores)) in values.iter().enumerate() {
        let y = 42 + index * 58;
        let _ = write!(
            html,
            "<text x=\"10\" y=\"{}\">{}</text>",
            y + 20,
            escape_html(&browser.browser.name)
        );
        for (point, (run, score)) in scores.iter().enumerate() {
            let x = 230.0 + score / maximum * 620.0;
            let jitter = point % 5 * 4;
            let _ = write!(
                html,
                "<circle cx=\"{x:.1}\" cy=\"{}\" r=\"4\" fill=\"var(--accent)\"><title>{}: measured run {}: {:.2}</title></circle>",
                y + 18 + jitter,
                escape_html(&browser.browser.name),
                run,
                score
            );
        }
        if let Some(median) = browser.summary.median_score {
            let x = 230.0 + median / maximum * 620.0;
            let _ = write!(
                html,
                "<line x1=\"{x:.1}\" y1=\"{}\" x2=\"{x:.1}\" y2=\"{}\" stroke=\"var(--danger)\" stroke-width=\"2\"><title>Median: {median:.2}</title></line>",
                y + 8,
                y + 44
            );
        }
    }
    html.push_str("</svg><p class=\"muted\">Each dot is a completed measured run; warm-ups and failures are excluded.</p></section>");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn required_commit(report: &JetStreamReport) -> Result<&str, String> {
    report.source.official_commit.as_deref().ok_or_else(|| "hosted JetStream reports do not contain a verified immutable source revision and cannot be used for baseline comparison".to_owned())
}

pub(crate) fn parse_history_report(
    bytes: &[u8],
) -> Result<Vec<crate::history::ImportEntry>, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid JetStream report JSON: {error}"))?;
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or("JetStream report has no valid schema_version")?;
    if schema != 1 {
        return Err(format!(
            "unsupported JetStream report schema {schema}; supported schema is 1"
        ));
    }
    let report: JetStreamReport = serde_json::from_value(value)
        .map_err(|error| format!("invalid JetStream report: {error}"))?;
    if report.benchmark != "jetstream"
        || report.benchmark_version != JETSTREAM_VERSION
        || report.source.version != JETSTREAM_VERSION
    {
        return Err(format!(
            "history import supports JetStream {JETSTREAM_VERSION}, found '{}' version '{}'",
            report.benchmark, report.benchmark_version
        ));
    }
    if report.configuration.requested_runs == 0
        || report.configuration.workload_selection != "complete_default_suite"
        || !report.configuration.prefetch_resources
        || report.configuration.browser_mode != "headed"
        || report.configuration.source_kind != report.source.kind
    {
        return Err("JetStream report configuration is unsupported or incomplete".to_owned());
    }
    match report.source.kind.as_str() {
        "official_pinned_local"
            if report.source.official_commit.as_deref() == Some(JETSTREAM_COMMIT)
                && report.source.archive_sha256.as_deref() == Some(JETSTREAM_ARCHIVE_SHA256)
                && report.source.url.starts_with("http://127.0.0.1:")
                && report.source.url.contains("/JetStream3.0/index.html") => {}
        "official_hosted"
            if report.source.official_commit.is_none()
                && report.source.archive_sha256.is_none()
                && report.source.url == official_url() => {}
        _ => {
            return Err(
                "JetStream source metadata does not match a supported 3.0 source".to_owned(),
            );
        }
    }
    if report.generated_at_unix_ms == 0
        || report.environment.operating_system.trim().is_empty()
        || report.environment.architecture.trim().is_empty()
    {
        return Err("JetStream report timestamp or environment metadata is incomplete".to_owned());
    }
    if report.cancelled {
        return Err(
            "cancelled JetStream reports cannot be imported as complete benchmark runs".to_owned(),
        );
    }
    let mut entries = Vec::new();
    for browser in &report.browsers {
        if browser.browser.id.trim().is_empty()
            || browser.browser.installation_id.trim().is_empty()
            || browser.browser.name.trim().is_empty()
            || browser.browser.executable_path.as_os_str().is_empty()
        {
            return Err("JetStream browser metadata is incomplete".to_owned());
        }
        let mut scores = Vec::new();
        let mut completed_workloads = None;
        for iteration in &browser.iterations {
            if iteration.completion_verified {
                if iteration.failure.is_some() {
                    return Err(format!(
                        "{} has a completed JetStream run marked as failed",
                        browser.browser.name
                    ));
                }
                let score = iteration
                    .official_score
                    .filter(|score| score.is_finite() && *score > 0.0)
                    .ok_or_else(|| {
                        format!(
                            "{} has a completed JetStream run without a valid official score",
                            browser.browser.name
                        )
                    })?;
                if iteration.benchmark_version.as_deref() != Some(JETSTREAM_VERSION)
                    || iteration.total_workloads == 0
                    || iteration.completed_workloads != iteration.total_workloads
                    || iteration.subtest_scores.len() != iteration.total_workloads
                    || iteration.subtest_scores.len() > MAX_SUBTESTS
                    || iteration
                        .subtest_scores
                        .values()
                        .any(|score| !score.is_finite() || *score <= 0.0)
                {
                    return Err(format!(
                        "{} JetStream run is missing complete official workload results",
                        browser.browser.name
                    ));
                }
                if completed_workloads
                    .replace(iteration.total_workloads)
                    .is_some_and(|count| count != iteration.total_workloads)
                {
                    return Err(format!(
                        "{} JetStream runs used different workload counts",
                        browser.browser.name
                    ));
                }
                if !iteration.warmup {
                    scores.push(score);
                }
            } else if iteration.official_score.is_some() || iteration.failure.is_none() {
                return Err(format!(
                    "{} has an incomplete JetStream run without a structured failure",
                    browser.browser.name
                ));
            }
        }
        if scores.is_empty() {
            continue;
        }
        let statistics = summarize_metric_values(scores.iter().copied())
            .ok_or_else(|| "cannot summarize JetStream scores".to_owned())?;
        let failed_measured_runs = browser
            .iterations
            .iter()
            .filter(|iteration| !iteration.warmup && iteration.failure.is_some())
            .count();
        if browser.summary.valid_measured_runs != scores.len()
            || browser.summary.failed_measured_runs != failed_measured_runs
            || browser
                .summary
                .median_score
                .is_none_or(|value| !close_enough(value, statistics.median))
            || browser
                .summary
                .mean_score
                .is_none_or(|value| !close_enough(value, statistics.mean))
        {
            return Err(format!(
                "{} JetStream report summary does not match its raw completed runs",
                browser.browser.name
            ));
        }
        let mut warnings = Vec::new();
        if report.source.official_commit.is_none() {
            warnings.push("hosted source has no verified immutable source revision; performance comparisons are disabled".to_owned());
        }
        entries.push(crate::history::ImportEntry {
            benchmark: "jetstream".to_owned(),
            benchmark_version: Some(JETSTREAM_VERSION.to_owned()),
            benchmark_schema_version: schema as u32,
            browser: browser.browser.clone(),
            timestamp_unix_ms: report.generated_at_unix_ms,
            operating_system: report.environment.operating_system.clone(),
            architecture: report.environment.architecture.clone(),
            hardware: None,
            benchmark_source: serde_json::to_value(&report.source).ok(),
            configuration: serde_json::to_value(&report.configuration)
                .map_err(|error| format!("cannot read JetStream configuration: {error}"))?,
            scenario: None,
            summary: vec![crate::history::HistoryMetric {
                name: "score".to_owned(),
                unit: "score".to_owned(),
                median: statistics.median,
                mean: Some(statistics.mean),
                standard_deviation: statistics.sample_standard_deviation,
                sample_count: scores.len(),
                higher_is_better: Some(true),
            }],
            comparability: crate::history::ComparabilityMetadata {
                status: "recorded".to_owned(),
                warnings,
            },
        });
    }
    if entries.is_empty() {
        return Err("JetStream report contains no complete positive measured score".to_owned());
    }
    Ok(entries)
}

fn close_enough(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= right.abs().max(1.0) * 1e-9
}

#[cfg(target_os = "macos")]
pub(crate) fn run_compare(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut baseline = None;
    let mut current = None;
    let mut threshold = 5.0_f64;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--baseline" if baseline.is_none() => {
                baseline = Some(required_value(&mut arguments, &option)?)
            }
            "--current" if current.is_none() => {
                current = Some(required_value(&mut arguments, &option)?)
            }
            "--threshold" => {
                let value = required_value(&mut arguments, &option)?;
                threshold = value
                    .parse()
                    .map_err(|_| format!("invalid threshold '{value}'"))?;
                if !threshold.is_finite() || threshold < 0.0 {
                    return Err("--threshold must be finite and non-negative".to_owned());
                }
            }
            other => return Err(format!("unknown JetStream comparison option '{other}'")),
        }
    }
    let directory = crate::history::default_storage_directory();
    let baseline_selector =
        baseline.ok_or("compare jetstream requires --baseline <report-or-name>")?;
    let current_selector =
        current.ok_or("compare jetstream requires --current <report-or-name>")?;
    let baseline_history = crate::history::record_for_selector(&baseline_selector, &directory)?;
    let current_history = crate::history::record_for_selector(&current_selector, &directory)?;
    if crate::history::hardware_comparability(baseline_history.as_ref(), current_history.as_ref())
        == Some(false)
    {
        return Err("JetStream reports have a known hardware configuration or host identity mismatch and are not comparable".to_owned());
    }
    if crate::history::hardware_comparability(baseline_history.as_ref(), current_history.as_ref())
        .is_none()
    {
        println!(
            "Warning: hardware identity is unavailable; matching OS and architecture does not prove the reports came from the same machine."
        );
    }
    let baseline_path = crate::history::resolve_report_selector(&baseline_selector, &directory)?;
    let current_path = crate::history::resolve_report_selector(&current_selector, &directory)?;
    let baseline_report = read_report(&baseline_path)?;
    let current_report = read_report(&current_path)?;
    if baseline_report.benchmark != "jetstream" || current_report.benchmark != "jetstream" {
        return Err("both reports must contain JetStream".to_owned());
    }
    if required_commit(&baseline_report)? != required_commit(&current_report)?
        || baseline_report.source.archive_sha256 != current_report.source.archive_sha256
        || baseline_report.benchmark_version != current_report.benchmark_version
        || baseline_report.configuration.browser_mode != current_report.configuration.browser_mode
        || baseline_report.configuration.source_kind != current_report.configuration.source_kind
        || baseline_report.configuration.workload_selection
            != current_report.configuration.workload_selection
        || baseline_report.configuration.prefetch_resources
            != current_report.configuration.prefetch_resources
        || baseline_report.environment.operating_system
            != current_report.environment.operating_system
        || baseline_report.environment.architecture != current_report.environment.architecture
    {
        return Err("JetStream source revision, version, workload configuration, execution mode, or environment differs; reports are incompatible".to_owned());
    }
    let mut regressions = 0;
    let mut comparisons = 0;
    for old in &baseline_report.browsers {
        let Some(new) = current_report
            .browsers
            .iter()
            .find(|browser| browser.browser.installation_id == old.browser.installation_id)
        else {
            continue;
        };
        let (Some(old_median), Some(new_median)) =
            (old.summary.median_score, new.summary.median_score)
        else {
            continue;
        };
        let old_scores = measured_scores(old);
        let new_scores = measured_scores(new);
        if old_scores.len() < MIN_COMPARISON_RUNS || new_scores.len() < MIN_COMPARISON_RUNS {
            println!(
                "{}: INCONCLUSIVE — at least {MIN_COMPARISON_RUNS} complete measured runs are required per side",
                old.browser.name
            );
            continue;
        }
        let change = (new_median - old_median) / old_median * 100.0;
        let p_value =
            crate::comparison_statistics::median_permutation_p_value(&old_scores, &new_scores);
        let status = if change.abs() <= threshold {
            "within threshold"
        } else if p_value.is_some_and(|p| p < 0.05) && change < 0.0 {
            regressions += 1;
            "REGRESSION"
        } else if p_value.is_some_and(|p| p < 0.05) {
            "IMPROVEMENT"
        } else {
            "INCONCLUSIVE (threshold crossed without statistical evidence)"
        };
        comparisons += 1;
        println!(
            "{}: baseline {old_median:.2}, current {new_median:.2}, change {change:+.2}%, p={} — {status}",
            old.browser.name,
            p_value.map_or_else(|| "N/A".to_owned(), |p| format!("{p:.3}"))
        );
    }
    if comparisons == 0 {
        return Err(format!(
            "reports have no matching browser with at least {MIN_COMPARISON_RUNS} valid measured runs"
        ));
    }
    if regressions > 0 {
        Err(format!(
            "{regressions} JetStream regression(s) exceed the {threshold:.1}% threshold"
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn run_compare(_arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    Err("JetStream regression comparison currently requires macOS".to_owned())
}

#[cfg(target_os = "macos")]
fn read_report(path: &Path) -> Result<JetStreamReport, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JetStream report {}: {error}", path.display()))
}

fn measured_scores(browser: &BrowserResult) -> Vec<f64> {
    browser
        .iterations
        .iter()
        .filter(|iteration| !iteration.warmup && iteration.completion_verified)
        .filter_map(|iteration| iteration.official_score)
        .filter(|score| score.is_finite() && *score > 0.0)
        .collect()
}

#[cfg(target_os = "macos")]
fn install_cancellation_handler() -> Result<(), String> {
    let previous = unsafe {
        libc::signal(
            libc::SIGINT,
            handle_sigint as *const () as libc::sighandler_t,
        )
    };
    if previous == libc::SIG_ERR {
        Err("cannot install JetStream cancellation handler".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
extern "C" fn handle_sigint(_: libc::c_int) {
    CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
fn secure_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| std::io::Read::read_exact(&mut random, &mut bytes))
        .map_err(|error| format!("cannot generate Photon automation token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(target_os = "macos")]
fn cleanup_photon_directory(
    directory: Option<&Path>,
    token: Option<&str>,
    cleanup: &mut CleanupStatus,
) {
    let Some(directory) = directory else {
        return;
    };
    let Some(token) = token else {
        return;
    };
    let token_path = directory.join("token");
    if fs::read_to_string(&token_path).is_ok_and(|contents| contents.trim() == token) {
        let _ = fs::remove_file(&token_path);
    }
    if fs::remove_dir_all(directory).is_err() && directory.exists() {
        cleanup.automation_resources_removed = false;
        cleanup.detail = Some(format!(
            "could not remove Photon automation directory {}",
            directory.display()
        ));
    } else {
        cleanup.automation_resources_removed = true;
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(target_os = "macos")]
fn random_seed() -> u64 {
    now_unix_ms() as u64 ^ ((std::process::id() as u64) << 32)
}

#[cfg(target_os = "macos")]
fn shuffle<T>(values: &mut [T], seed: &mut u64) {
    for index in (1..values.len()).rev() {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        values.swap(index, *seed as usize % (index + 1));
    }
}

fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_pinned_local_complete_browser_suite() {
        let options = parse_options(Vec::new()).unwrap();
        assert_eq!(options.runs, 5);
        assert_eq!(options.warmups, 1);
        assert_eq!(options.source, SourceKind::Local);
        assert!(options.record_history);
    }

    #[test]
    fn options_reject_unimplemented_and_duplicate_modes() {
        assert!(parse_options(vec!["--headless".to_owned()]).is_err());
        assert!(
            parse_options(vec![
                "--browser".to_owned(),
                "chrome".to_owned(),
                "--all".to_owned()
            ])
            .is_err()
        );
        assert!(parse_options(vec!["--runs".to_owned(), "0".to_owned()]).is_err());
        assert!(
            parse_options(vec![
                "--no-record-history".to_owned(),
                "--record-history".to_owned()
            ])
            .is_err()
        );
    }

    #[test]
    fn hidden_jetstream_page_activation_is_throttled_to_once_per_second() {
        let start = Instant::now();
        assert!(visibility_restore_due(None, start));
        assert!(!visibility_restore_due(
            Some(start),
            start + Duration::from_millis(999)
        ));
        assert!(visibility_restore_due(
            Some(start),
            start + Duration::from_secs(1)
        ));
    }

    #[test]
    fn official_source_is_not_comparable_without_a_verified_revision() {
        let report = JetStreamReport {
            schema_version: 1,
            benchmark: "jetstream".to_owned(),
            benchmark_version: JETSTREAM_VERSION.to_owned(),
            source: SourceMetadata {
                kind: "official_hosted".to_owned(),
                url: official_url(),
                version: JETSTREAM_VERSION.to_owned(),
                official_commit: None,
                archive_sha256: None,
            },
            configuration: BenchmarkConfiguration {
                requested_runs: 5,
                warmup_runs: 1,
                timeout_seconds: 1800,
                browser_mode: "headed".to_owned(),
                source_kind: "official_hosted".to_owned(),
                workload_selection: "complete_default_suite".to_owned(),
                prefetch_resources: true,
            },
            environment: EnvironmentMetadata {
                operating_system: "macos".to_owned(),
                architecture: "aarch64".to_owned(),
            },
            execution_order: ExecutionOrder {
                randomized: false,
                randomization_seed: None,
                warmup_order_by_round: Vec::new(),
                measured_order_by_round: Vec::new(),
            },
            browsers: Vec::new(),
            comparison: ComparisonAssessment {
                status: "INCONCLUSIVE".to_owned(),
                higher_is_better: true,
                descriptive_ranking_allowed: false,
                reasons: Vec::new(),
            },
            cancelled: false,
            generated_at_unix_ms: 1,
        };
        assert!(required_commit(&report).is_err());
    }

    #[test]
    fn higher_is_better_regression_needs_threshold_and_permutation_evidence() {
        let old = [100.0, 101.0, 102.0, 103.0, 104.0];
        let new = [90.0, 91.0, 92.0, 93.0, 94.0];
        let p = crate::comparison_statistics::median_permutation_p_value(&old, &new).unwrap();
        assert!(p < 0.05);
        let change = (92.0 - 102.0) / 102.0 * 100.0;
        assert!(change < -5.0);

        let overlapping = [100.0, 101.0, 102.0, 103.0, 104.0];
        let near = [101.0, 102.0, 103.0, 104.0, 105.0];
        assert!(
            crate::comparison_statistics::median_permutation_p_value(&overlapping, &near).unwrap()
                >= 0.05
        );
    }
}
