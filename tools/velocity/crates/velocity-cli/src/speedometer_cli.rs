use std::{
    collections::BTreeSet,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use velocity_browser::{BrowserAutomationAdapter, BrowserEngine, BrowserInstallation};
use velocity_core::{ReportBrowser, unique_run_id};
use velocity_metrics::summarize_metric_values;

use crate::speedometer_source::{
    BenchmarkSourceKind, SPEEDOMETER_COMMIT, SPEEDOMETER_VERSION, official_url,
};

const DEFAULT_RUNS: usize = 5;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_TIMEOUT_SECONDS: u64 = 600;
const MAX_RUNS: usize = 100;
const MAX_WARMUPS: usize = 20;
const MIN_COMPARISON_RUNS: usize = 3;

static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Debug)]
enum TargetSelection {
    One(String),
    Many(Vec<String>),
    All,
    Prompt,
}

#[derive(Clone, Debug)]
struct Options {
    target: TargetSelection,
    runs: usize,
    warmups: usize,
    randomized: bool,
    timeout: Duration,
    source: BenchmarkSourceKind,
    headless: bool,
    output_directory: PathBuf,
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
    official_internal_iterations: usize,
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
struct SpeedometerFailure {
    stage: String,
    message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpeedometerIteration {
    iteration: usize,
    warmup: bool,
    started_at_unix_ms: u128,
    score: Option<f64>,
    score_unit: String,
    benchmark_version: Option<String>,
    official_internal_iteration_scores: Vec<f64>,
    execution_duration_ms: Option<f64>,
    completion_verified: bool,
    warnings: Vec<String>,
    failure: Option<SpeedometerFailure>,
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
    iterations: Vec<SpeedometerIteration>,
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
struct SpeedometerReport {
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

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|value| matches!(value.as_str(), "--help" | "-h"))
    {
        print_help();
        return Ok(());
    }
    let options = parse_options(arguments)?;
    if options.headless {
        return Err(
            "headless Speedometer runs are not supported; use the default headed browser window so the official benchmark conditions remain consistent".to_owned(),
        );
    }
    run_benchmark(options)
}

fn parse_options(arguments: Vec<String>) -> Result<Options, String> {
    let mut browser = None;
    let mut browsers = None;
    let mut all = false;
    let mut runs = DEFAULT_RUNS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut randomized = false;
    let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
    let mut source = BenchmarkSourceKind::Official;
    let mut headless = false;
    let mut output_directory = default_output_directory();
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
                browsers = Some(values.into_iter().map(str::to_owned).collect::<Vec<_>>());
            }
            "--all" => all = true,
            "--runs" => runs = parse_value(&mut arguments, &option)?,
            "--warmup" => warmups = parse_value(&mut arguments, &option)?,
            "--randomize" => randomized = true,
            "--timeout" => timeout_seconds = parse_value(&mut arguments, &option)?,
            "--benchmark-source" => {
                source = BenchmarkSourceKind::parse(&required_value(&mut arguments, &option)?)?
            }
            "--headless" => headless = true,
            "--output" => {
                output_directory = PathBuf::from(required_value(&mut arguments, &option)?)
            }
            other => return Err(format!("unknown Speedometer option '{other}'")),
        }
    }
    if !(1..=MAX_RUNS).contains(&runs) {
        return Err(format!("--runs must be between 1 and {MAX_RUNS}"));
    }
    if warmups > MAX_WARMUPS {
        return Err(format!("--warmup must be at most {MAX_WARMUPS}"));
    }
    if !(30..=3_600).contains(&timeout_seconds) {
        return Err("--timeout must be between 30 and 3600 seconds".to_owned());
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
        headless,
        output_directory,
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
        "## `velocity bench speedometer`\n\n**Usage:** `velocity bench speedometer [--browser <id|number> | --browsers <ids> | --all] [OPTIONS]`\n\nRuns the official Speedometer 3.1 page in fresh, isolated, headed browser sessions. The official benchmark starts itself through its documented `startAutomatically` URL parameter; Velocity reads its own completed score.\n\nOptions: `--runs N` (5), `--warmup N` (1), `--randomize`, `--timeout SECONDS` (600), `--benchmark-source official|local` (official hosted), `--output DIRECTORY`. Local mode uses the checksum-pinned source prepared by `velocity setup speedometer`.\n\nCDP adapters for Chrome and Helium and a restricted Photon observer are implemented. End-to-end support is only established by a completed live run; smoke-test failures remain in reports without scores. Other installations are reported unsupported. Headless mode is not supported.",
    );
}

#[cfg(not(target_os = "macos"))]
fn run_benchmark(_options: Options) -> Result<(), String> {
    Err("Speedometer automation currently requires macOS browser lifecycle support".to_owned())
}

#[cfg(target_os = "macos")]
fn run_benchmark(options: Options) -> Result<(), String> {
    use std::{fs, sync::atomic::Ordering};
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

    let source_server = if options.source == BenchmarkSourceKind::Local {
        let source_path = super::speedometer_source::require_prepared_source()?;
        Some(super::speedometer_source::LocalSpeedometerServer::start(
            source_path,
        )?)
    } else {
        None
    };
    let source_url = source_server
        .as_ref()
        .map(|server| server.url().to_owned())
        .unwrap_or_else(official_url);
    let source = if options.source == BenchmarkSourceKind::Local {
        SourceMetadata {
            kind: options.source.name().to_owned(),
            url: source_url.clone(),
            version: SPEEDOMETER_VERSION.to_owned(),
            official_commit: Some(SPEEDOMETER_COMMIT.to_owned()),
            archive_sha256: Some(
                "cfefa818d7789ed2f2b9f3f1bf608cc35e4e8241489eef47049296ce92791313".to_owned(),
            ),
        }
    } else {
        SourceMetadata {
            kind: options.source.name().to_owned(),
            url: source_url.clone(),
            version: SPEEDOMETER_VERSION.to_owned(),
            official_commit: None,
            archive_sha256: None,
        }
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
        .collect::<std::collections::BTreeMap<_, _>>();

    println!("Velocity — Speedometer 3.1\n");
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
        .warmups
        .checked_add(options.runs)
        .ok_or_else(|| "run count overflow".to_owned())?;
    let mut run_index_by_installation = std::collections::BTreeMap::<String, usize>::new();
    for round_index in 0..rounds {
        if CANCELLED.load(Ordering::Relaxed) {
            break;
        }
        let warmup = round_index < options.warmups;
        let mut round_installations = installations.clone();
        if options.randomized {
            shuffle(&mut round_installations, &mut random);
        }
        let mut actual_round = Vec::new();
        for installation in round_installations {
            if CANCELLED.load(Ordering::Relaxed) {
                break;
            }
            let key = installation.installation_id.to_string();
            let iteration_number = run_index_by_installation.entry(key.clone()).or_default();
            *iteration_number += 1;
            let label = if warmup {
                format!(
                    "[warm-up {}] {}",
                    round_index + 1,
                    installation.metadata.name
                )
            } else {
                format!(
                    "[{}/{}] {}",
                    round_index - options.warmups + 1,
                    options.runs,
                    installation.metadata.name
                )
            };
            println!("{label}");
            let iteration = execute_iteration(
                &installation,
                *iteration_number,
                warmup,
                &source_url,
                &profile_root,
                &session_store_path,
                options.timeout,
            );
            match (&iteration.failure, iteration.score) {
                (Some(failure), _) => {
                    println!("      Failed ({}): {}", failure.stage, failure.message)
                }
                (None, Some(score)) => println!("      Official score: {score:.3}"),
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
            actual_round.push(key.clone());
            iterations_by_installation
                .get_mut(&key)
                .expect("iteration map matches selected browsers")
                .push(iteration);
        }
        if !actual_round.is_empty() {
            if warmup {
                warmup_order.push(actual_round);
            } else {
                measured_order.push(actual_round);
            }
        }
    }
    let cancelled = CANCELLED.load(Ordering::Relaxed);
    let browser_results = installations
        .iter()
        .map(|installation| {
            let iterations = iterations_by_installation
                .remove(&installation.installation_id.to_string())
                .unwrap_or_default();
            browser_result(installation, iterations)
        })
        .collect::<Vec<_>>();
    let comparison = assess_comparison(&browser_results, &source, options.runs);
    let report = SpeedometerReport {
        schema_version: 1,
        benchmark: "speedometer".to_owned(),
        benchmark_version: SPEEDOMETER_VERSION.to_owned(),
        source,
        configuration: BenchmarkConfiguration {
            requested_runs: options.runs,
            warmup_runs: options.warmups,
            timeout_seconds: options.timeout.as_secs(),
            browser_mode: "headed".to_owned(),
            source_kind: options.source.name().to_owned(),
            official_internal_iterations: 10,
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
        browsers: browser_results,
        comparison,
        cancelled,
        generated_at_unix_ms: now_unix_ms(),
    };

    print_summary(&report);
    let (json_path, html_path) = write_reports(&options.output_directory, &run_id, &report)?;
    println!("\nSaved JSON: {}", display_path(&json_path));
    println!("Saved HTML: {}", display_path(&html_path));
    drop(source_server);

    let any_success = report
        .browsers
        .iter()
        .flat_map(|browser| &browser.iterations)
        .any(|iteration| {
            !iteration.warmup && iteration.completion_verified && iteration.score.is_some()
        });
    if !any_success && !cancelled {
        return Err(format!(
            "no Speedometer measured run completed; diagnostics were saved to {} and {}",
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
    use velocity_browser::BrowserInstallation;
    let selected = match target {
        TargetSelection::One(selector) => {
            vec![crate::browser_selection::resolve(registry, selector)?.clone()]
        }
        TargetSelection::Many(selectors) => selectors
            .iter()
            .map(|selector| crate::browser_selection::resolve(registry, selector).cloned())
            .collect::<Result<Vec<BrowserInstallation>, String>>()?,
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

#[cfg(target_os = "macos")]
fn execute_iteration(
    installation: &BrowserInstallation,
    iteration: usize,
    warmup: bool,
    source_url: &str,
    profile_root: &Path,
    session_store_path: &Path,
    timeout: Duration,
) -> SpeedometerIteration {
    use velocity_browser::{
        BrowserLifecycleManager, FileSessionStore, ProcessLaunchTiming,
        macos::{MacOSProcessDriver, MacOSWindowObserver},
    };
    use velocity_core::{CancellationToken, StartupMetric, observe_startup_windows};

    let mut result = SpeedometerIteration {
        iteration,
        warmup,
        started_at_unix_ms: now_unix_ms(),
        score: None,
        score_unit: "official Speedometer score (higher is better)".to_owned(),
        benchmark_version: None,
        official_internal_iteration_scores: Vec::new(),
        execution_duration_ms: None,
        completion_verified: false,
        warnings: Vec::new(),
        failure: None,
        cleanup: CleanupStatus {
            browser_closed: true,
            profile_removed: true,
            automation_resources_removed: true,
            detail: None,
        },
    };
    if !is_supported_installation(installation) {
        result.failure = Some(SpeedometerFailure {
            stage: "capability".to_owned(),
            message: format!(
                "{} has no verified Speedometer automation adapter; supported adapters are CDP for Chrome/Helium and the negotiated Photon protocol",
                installation.metadata.name
            ),
        });
        return result;
    }
    if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
        result.failure = Some(SpeedometerFailure {
            stage: "cancelled".to_owned(),
            message: "cancelled before browser launch".to_owned(),
        });
        return result;
    }

    let photon = installation.metadata.id.as_str() == "photon";
    let token = if photon {
        match secure_token() {
            Ok(token) => Some(token),
            Err(error) => {
                result.failure = Some(SpeedometerFailure {
                    stage: "setup".to_owned(),
                    message: error,
                });
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
            result.failure = Some(SpeedometerFailure {
                stage: "launch".to_owned(),
                message: error.to_string(),
            });
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
    let started = Instant::now();
    let automation_result = (|| -> Result<AutomationOutcome, SpeedometerFailure> {
        let timing = launch_timing.as_ref().ok_or_else(|| SpeedometerFailure {
            stage: "launch".to_owned(),
            message: "process driver did not report browser launch timing".to_owned(),
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
            let failure = window
                .failures
                .first()
                .map(|failure| failure.message.clone())
                .unwrap_or_else(|| "browser window did not become ready".to_owned());
            return Err(SpeedometerFailure {
                stage: "window_readiness".to_owned(),
                message: failure,
            });
        }
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SpeedometerFailure {
                stage: "cancelled".to_owned(),
                message: "cancelled while waiting for browser window readiness".to_owned(),
            });
        }
        if photon {
            run_photon_speedometer(
                socket_path.as_deref().ok_or_else(|| SpeedometerFailure {
                    stage: "setup".to_owned(),
                    message: "Photon automation socket was not prepared".to_owned(),
                })?,
                source_url,
                timeout,
            )
        } else {
            let profile =
                session
                    .identity
                    .profile_path
                    .as_deref()
                    .ok_or_else(|| SpeedometerFailure {
                        stage: "setup".to_owned(),
                        message: "Chromium session has no isolated profile for CDP".to_owned(),
                    })?;
            run_chromium_speedometer(profile, source_url, timeout)
        }
    })();
    if let Ok(outcome) = &automation_result {
        result.score = Some(outcome.score);
        result.benchmark_version = Some(outcome.version.clone());
        result.official_internal_iteration_scores = outcome.internal_scores.clone();
        result.execution_duration_ms = outcome.execution_duration_ms;
        result.completion_verified = true;
        result.warnings = outcome.warnings.clone();
    } else if let Err(failure) = automation_result {
        result.failure = Some(failure);
    }

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
            result.cleanup.detail = Some(format!(
                "managed browser close failed after {:.1} seconds: {error}",
                started.elapsed().as_secs_f64()
            ));
        }
    }
    cleanup_photon_directory(
        automation_directory.as_deref(),
        token.as_deref(),
        &mut result.cleanup,
    );
    if !result.cleanup.browser_closed && result.failure.is_none() {
        result.failure = Some(SpeedometerFailure {
            stage: "cleanup".to_owned(),
            message: result
                .cleanup
                .detail
                .clone()
                .unwrap_or_else(|| "managed browser could not be closed".to_owned()),
        });
    }
    if !result.cleanup.profile_removed && result.failure.is_none() {
        result.failure = Some(SpeedometerFailure {
            stage: "cleanup".to_owned(),
            message: "managed browser profile remains after cleanup".to_owned(),
        });
    }
    result
}

#[derive(Clone, Debug)]
struct AutomationOutcome {
    version: String,
    score: f64,
    internal_scores: Vec<f64>,
    execution_duration_ms: Option<f64>,
    warnings: Vec<String>,
}

#[cfg(target_os = "macos")]
fn run_chromium_speedometer(
    profile: &Path,
    source_url: &str,
    timeout: Duration,
) -> Result<AutomationOutcome, SpeedometerFailure> {
    use velocity_browser::ChromiumDevTools;
    let mut devtools = ChromiumDevTools::connect(profile, Duration::from_secs(30))
        .map_err(|error| automation_failure("automation_connect", error.to_string()))?;
    let target = devtools
        .navigate_initial_page(source_url)
        .map_err(|error| automation_failure("navigation", error))?;
    let cancel = || CANCELLED.load(std::sync::atomic::Ordering::Relaxed);
    let started_at = Instant::now();
    let initial = devtools
        .wait_for_speedometer_started(&target, source_url, timeout, cancel)
        .map_err(|error| {
            automation_failure(
                if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
                    "cancelled"
                } else {
                    "benchmark_start"
                },
                error.to_string(),
            )
        })?;
    let started = initial.started();
    let started_observed = started.then(Instant::now);
    validate_speedometer_version(&initial.benchmark_version)?;
    if initial.viewport_warning {
        return Err(automation_failure(
            "page_readiness",
            "Speedometer reports that the browser viewport is smaller than its recommended 850 × 650 pixels",
        ));
    }
    let completed = if initial.completed() {
        initial
    } else {
        let remaining = timeout.saturating_sub(started_at.elapsed());
        devtools
            .wait_for_speedometer_completed(&target, source_url, remaining, cancel)
            .map_err(|error| {
                automation_failure(
                    if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
                        "cancelled"
                    } else {
                        "benchmark_completion"
                    },
                    error.to_string(),
                )
            })?
    };
    if let Some(message) = completed.failed_message {
        return Err(automation_failure("score_validation", message));
    }
    validate_speedometer_version(&completed.benchmark_version)?;
    let score = completed
        .score
        .filter(|score| score.is_finite() && *score > 0.0)
        .ok_or_else(|| {
            automation_failure("score_validation", "official score is missing or invalid")
        })?;
    validate_internal_scores(&completed.internal_scores)?;
    let mut warnings = Vec::new();
    if !started {
        warnings.push(
            "benchmark completed before Velocity observed the running state; elapsed time is unavailable".to_owned(),
        );
    }
    Ok(AutomationOutcome {
        version: SPEEDOMETER_VERSION.to_owned(),
        score,
        internal_scores: completed.internal_scores,
        execution_duration_ms: started_observed
            .map(|started| started.elapsed().as_secs_f64() * 1000.0),
        warnings,
    })
}

#[cfg(target_os = "macos")]
fn run_photon_speedometer(
    socket_path: &Path,
    source_url: &str,
    timeout: Duration,
) -> Result<AutomationOutcome, SpeedometerFailure> {
    use velocity_browser::{PhotonAutomationClient, parse_speedometer_result_title};
    let credential_path = socket_path
        .parent()
        .ok_or_else(|| automation_failure("automation_setup", "Photon socket has no parent"))?
        .join("token");
    let mut client = PhotonAutomationClient::connect_with_cancel(
        socket_path,
        credential_path,
        Duration::from_secs(30),
        || CANCELLED.load(std::sync::atomic::Ordering::Relaxed),
    )
    .map_err(|error| automation_failure("automation_connect", error.to_string()))?;
    if !client.supports("probe_speedometer_result") {
        return Err(automation_failure(
            "unsupported",
            "Photon protocol negotiated, but it does not advertise the restricted Speedometer result observer",
        ));
    }
    let tabs = client
        .list_tabs(Duration::from_secs(5))
        .map_err(|error| automation_failure("automation", error.to_string()))?;
    let [tab] = tabs.as_slice() else {
        return Err(automation_failure(
            "automation",
            format!(
                "isolated Photon session expected one initial tab, found {}",
                tabs.len()
            ),
        ));
    };
    let tab_id = tab.tab_id;
    client
        .navigate(tab_id, source_url, Duration::from_secs(10))
        .map_err(|error| automation_failure("navigation", error.to_string()))?;
    let deadline = Instant::now() + timeout;
    loop {
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(automation_failure(
                "cancelled",
                "cancelled during Speedometer navigation",
            ));
        }
        if Instant::now() >= deadline {
            return Err(automation_failure(
                "page_readiness",
                "Photon did not load the Speedometer document before the configured timeout",
            ));
        }
        let status = client
            .tab_status(tab_id, Duration::from_secs(5))
            .map_err(|error| automation_failure("automation", error.to_string()))?;
        if status
            .url
            .as_deref()
            .is_some_and(|url| url == source_url || url.starts_with(&format!("{source_url}#")))
            && status.loading == Some(false)
            && status
                .title
                .as_deref()
                .is_some_and(|title| title.contains("Speedometer 3.1"))
        {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    client
        .install_speedometer_result_observer(tab_id, Duration::from_secs(10))
        .map_err(|error| automation_failure("result_observer", error.to_string()))?;

    let mut started_observed = None;
    let result = loop {
        if CANCELLED.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(automation_failure(
                "cancelled",
                "cancelled while waiting for Speedometer completion",
            ));
        }
        if Instant::now() >= deadline {
            return Err(automation_failure(
                "benchmark_completion",
                "Photon Speedometer did not reach a verified completion state before the configured timeout",
            ));
        }
        let status = client
            .tab_status(tab_id, Duration::from_secs(5))
            .map_err(|error| automation_failure("automation", error.to_string()))?;
        match parse_speedometer_result_title(status.title.as_deref())
            .map_err(|error| automation_failure("score_extraction", error.to_string()))?
        {
            Some(status) if status.state == "running" => {
                if status.viewport_warning {
                    return Err(automation_failure(
                        "page_readiness",
                        "Speedometer reports that Photon viewport is smaller than its recommended 850 × 650 pixels",
                    ));
                }
                started_observed.get_or_insert_with(Instant::now);
            }
            Some(status) if status.state == "failed" => {
                return Err(automation_failure(
                    "benchmark_failure",
                    status.error.unwrap_or_else(|| {
                        "official benchmark reported an invalid score".to_owned()
                    }),
                ));
            }
            Some(status) if status.state == "complete" => {
                if status.viewport_warning {
                    return Err(automation_failure(
                        "page_readiness",
                        "Speedometer reports that Photon viewport is smaller than its recommended 850 × 650 pixels",
                    ));
                }
                break status;
            }
            _ => {}
        }
        thread::sleep(Duration::from_millis(250));
    };
    validate_speedometer_version(&result.version)?;
    let score = result
        .score
        .filter(|score| score.is_finite() && *score > 0.0)
        .ok_or_else(|| {
            automation_failure(
                "score_validation",
                "Photon returned a missing or invalid official score",
            )
        })?;
    validate_internal_scores(&result.internal_scores)?;
    let completed_at = Instant::now();
    let mut warnings = Vec::new();
    if started_observed.is_none() {
        warnings.push(
            "benchmark completed before Velocity observed the running state; elapsed time is unavailable".to_owned(),
        );
    }
    Ok(AutomationOutcome {
        version: SPEEDOMETER_VERSION.to_owned(),
        score,
        internal_scores: result.internal_scores,
        execution_duration_ms: started_observed
            .map(|started| completed_at.duration_since(started).as_secs_f64() * 1000.0),
        warnings,
    })
}

#[cfg(target_os = "macos")]
fn validate_speedometer_version(version: &Option<String>) -> Result<(), SpeedometerFailure> {
    if version.as_deref() == Some(SPEEDOMETER_VERSION) {
        Ok(())
    } else {
        Err(automation_failure(
            "benchmark_version",
            format!(
                "expected official Speedometer {SPEEDOMETER_VERSION}, observed {}",
                version.as_deref().unwrap_or("unknown")
            ),
        ))
    }
}

#[cfg(target_os = "macos")]
fn validate_internal_scores(scores: &[f64]) -> Result<(), SpeedometerFailure> {
    if scores.len() != 10 {
        return Err(automation_failure(
            "score_validation",
            format!(
                "Speedometer 3.1's default run should expose 10 official internal iterations, observed {}",
                scores.len()
            ),
        ));
    }
    if scores
        .iter()
        .any(|score| !score.is_finite() || *score <= 0.0)
    {
        return Err(automation_failure(
            "score_validation",
            "one or more official internal scores were missing or non-finite",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn automation_failure(stage: &str, message: impl Into<String>) -> SpeedometerFailure {
    SpeedometerFailure {
        stage: stage.to_owned(),
        message: message.into(),
    }
}

#[cfg(target_os = "macos")]
fn is_supported_installation(installation: &BrowserInstallation) -> bool {
    match installation.metadata.id.as_str() {
        "photon" => installation.metadata.engine == Some(BrowserEngine::Ladybird),
        "chrome" | "helium" => installation.metadata.engine == Some(BrowserEngine::Chromium),
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn cleanup_photon_directory(
    directory: Option<&Path>,
    token: Option<&str>,
    cleanup: &mut CleanupStatus,
) {
    let (Some(directory), Some(token)) = (directory, token) else {
        return;
    };
    use std::os::unix::fs::MetadataExt;
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            cleanup.automation_resources_removed = true;
            return;
        }
        Err(error) => {
            cleanup.automation_resources_removed = false;
            cleanup.detail = Some(format!(
                "cannot verify Photon automation directory {}: {error}",
                directory.display()
            ));
            return;
        }
    };
    let owned = metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o077 == 0;
    let expected_name = format!("vlt-{}", &token[..12]);
    if !owned
        || directory.parent() != Some(Path::new("/tmp"))
        || directory.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str())
    {
        cleanup.automation_resources_removed = false;
        cleanup.detail = Some(format!(
            "refused to remove Photon automation directory with uncertain ownership: {}",
            directory.display()
        ));
        return;
    }
    match fs::remove_dir_all(directory) {
        Ok(()) => cleanup.automation_resources_removed = true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            cleanup.automation_resources_removed = true;
        }
        Err(error) => {
            cleanup.automation_resources_removed = false;
            cleanup.detail = Some(format!(
                "cannot remove Photon automation directory {}: {error}",
                directory.display()
            ));
        }
    }
}

#[cfg(target_os = "macos")]
fn secure_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| std::io::Read::read_exact(&mut random, &mut bytes))
        .map_err(|error| format!("cannot generate Photon automation token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn browser_result(
    installation: &BrowserInstallation,
    iterations: Vec<SpeedometerIteration>,
) -> BrowserResult {
    let scores = iterations
        .iter()
        .filter(|iteration| !iteration.warmup && iteration.completion_verified)
        .filter_map(|iteration| iteration.score)
        .filter(|score| score.is_finite() && *score > 0.0)
        .collect::<Vec<_>>();
    let statistics = summarize_metric_values(scores.iter().copied());
    let valid_measured_runs = scores.len();
    let failed_measured_runs = iterations
        .iter()
        .filter(|iteration| !iteration.warmup && iteration.failure.is_some())
        .count();
    BrowserResult {
        browser: ReportBrowser::from(installation),
        engine_family: installation.metadata.engine.map(engine_name),
        automation_method: match installation.metadata.id.as_str() {
            "photon" => {
                "Photon authenticated Unix socket (restricted official-result observer)".to_owned()
            }
            "chrome" | "helium" => "Chrome DevTools Protocol (isolated profile)".to_owned(),
            _ => "unsupported".to_owned(),
        },
        iterations,
        summary: BrowserSummary {
            valid_measured_runs,
            failed_measured_runs,
            median_score: statistics.as_ref().map(|stats| stats.median),
            mean_score: statistics.as_ref().map(|stats| stats.mean),
            minimum_score: statistics.as_ref().map(|stats| stats.minimum),
            maximum_score: statistics.as_ref().map(|stats| stats.maximum),
            sample_standard_deviation: statistics
                .as_ref()
                .and_then(|stats| stats.sample_standard_deviation),
            coefficient_of_variation_percent: statistics
                .as_ref()
                .and_then(|stats| stats.coefficient_of_variation_percent),
        },
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

fn assess_comparison(
    browsers: &[BrowserResult],
    source: &SourceMetadata,
    requested_runs: usize,
) -> ComparisonAssessment {
    let mut reasons = Vec::new();
    if browsers.len() < 2 {
        reasons.push("comparison requires at least two selected browser installations".to_owned());
    }
    if source.version != SPEEDOMETER_VERSION {
        reasons.push("Speedometer versions do not match".to_owned());
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

fn print_summary(report: &SpeedometerReport) {
    crate::terminal::print_markdown(&format!(
        "## Comparison status: {}\n\n**Higher score is better.** Results are actual official Speedometer scores.",
        report.comparison.status
    ));
    let rows = report
        .browsers
        .iter()
        .map(|browser| {
            vec![
                browser.browser.name.clone(),
                format_optional(browser.summary.median_score),
                format_optional(browser.summary.mean_score),
                format_optional(browser.summary.sample_standard_deviation),
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
                "{} {} {} run failed at {}: {}",
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

fn format_optional(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.3}"))
}

fn write_reports(
    directory: &Path,
    run_id: &str,
    report: &SpeedometerReport,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(directory).map_err(|error| {
        format!(
            "cannot create report directory {}: {error}",
            directory.display()
        )
    })?;
    let json_bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("cannot serialize Speedometer report: {error}"))?;
    let html = render_html(report);
    let json_path = directory.join(format!("speedometer-{run_id}.json"));
    let html_path = directory.join(format!("speedometer-{run_id}.html"));
    write_new_file(&json_path, &json_bytes)?;
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

fn render_html(report: &SpeedometerReport) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Velocity — Speedometer 3.1</title><style>:root{--chart-series:#5078a8;--chart-median:#913d3d}body{font:15px system-ui,sans-serif;max-width:1100px;margin:32px auto;padding:0 20px;color:#1c2430;background:#f5f7fa}h1,h2{color:#101820}.card{background:white;border:1px solid #d6dde6;border-radius:10px;padding:18px;margin:18px 0;overflow:auto}.status{font-weight:700}.warning{color:#754800}.ok{color:#146444}table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}th,td{text-align:left;padding:9px 10px;border-bottom:1px solid #e2e7ed}th{background:#f1f4f8}svg{max-width:100%;height:auto}.muted{color:#526174}code{overflow-wrap:anywhere}</style><body>",
    );
    let _ = write!(
        html,
        "<h1>Velocity — Speedometer {}</h1><p class=\"status {}\">Comparison: {}. Higher score is better.</p>",
        escape_html(&report.benchmark_version),
        if report.comparison.descriptive_ranking_allowed {
            "ok"
        } else {
            "warning"
        },
        escape_html(&report.comparison.status)
    );
    let _ = write!(
        html,
        "<section class=\"card\"><h2>Configuration</h2><p>Source: {}<br>Commit: {}<br>Browser mode: {}<br>Measured runs: {}<br>Warm-up runs: {}<br>Official internal iterations: {}<br>Timeout: {} seconds<br>OS: {} ({})</p><p><code>{}</code></p>",
        escape_html(&report.source.kind),
        report
            .source
            .official_commit
            .as_deref()
            .map(escape_html)
            .unwrap_or_else(|| "not published for hosted source".to_owned()),
        escape_html(&report.configuration.browser_mode),
        report.configuration.requested_runs,
        report.configuration.warmup_runs,
        report.configuration.official_internal_iterations,
        report.configuration.timeout_seconds,
        escape_html(&report.environment.operating_system),
        escape_html(&report.environment.architecture),
        escape_html(&report.source.url)
    );
    if !report.comparison.reasons.is_empty() {
        html.push_str("<h3>Comparison diagnostics</h3><ul>");
        for reason in &report.comparison.reasons {
            let _ = write!(html, "<li>{}</li>", escape_html(reason));
        }
        html.push_str("</ul>");
    }
    html.push_str("</section><section class=\"card\"><h2>Scores</h2><table><thead><tr><th>Browser</th><th>Version</th><th>Median</th><th>Mean</th><th>Std dev</th><th>Valid</th><th>Failed</th></tr></thead><tbody>");
    for browser in &report.browsers {
        let _ = write!(
            html,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&browser.browser.name),
            escape_html(browser.browser.version.as_deref().unwrap_or("unknown")),
            format_optional(browser.summary.median_score),
            format_optional(browser.summary.mean_score),
            format_optional(browser.summary.sample_standard_deviation),
            browser.summary.valid_measured_runs,
            browser.summary.failed_measured_runs
        );
    }
    html.push_str("</tbody></table></section>");
    append_score_chart(&mut html, &report.browsers);
    html.push_str("<section class=\"card\"><h2>Iterations</h2>");
    for browser in &report.browsers {
        let _ = write!(
            html,
            "<h3>{}</h3><p class=\"muted\">{}</p><table><thead><tr><th>Kind</th><th>Run</th><th>Score</th><th>Duration observed</th><th>Status</th><th>Cleanup</th></tr></thead><tbody>",
            escape_html(&browser.browser.name),
            escape_html(&browser.automation_method)
        );
        for iteration in &browser.iterations {
            let status = iteration
                .failure
                .as_ref()
                .map(|failure| format!("{}: {}", failure.stage, failure.message))
                .unwrap_or_else(|| {
                    if iteration.completion_verified {
                        "complete".to_owned()
                    } else {
                        "not completed".to_owned()
                    }
                });
            let cleanup = if iteration.cleanup.browser_closed
                && iteration.cleanup.profile_removed
                && iteration.cleanup.automation_resources_removed
            {
                "clean"
            } else {
                "cleanup warning"
            };
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                if iteration.warmup {
                    "warm-up"
                } else {
                    "measured"
                },
                iteration.iteration,
                format_optional(iteration.score),
                iteration
                    .execution_duration_ms
                    .map_or_else(|| "N/A".to_owned(), |ms| format!("{ms:.0} ms")),
                escape_html(&status),
                cleanup
            );
            for warning in &iteration.warnings {
                let _ = write!(
                    html,
                    "<tr><td colspan=\"6\" class=\"warning\">{}</td></tr>",
                    escape_html(warning)
                );
            }
            if let Some(detail) = &iteration.cleanup.detail {
                let _ = write!(
                    html,
                    "<tr><td colspan=\"6\" class=\"warning\">Cleanup: {}</td></tr>",
                    escape_html(detail)
                );
            }
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</section><p class=\"muted\">Scores and internal iteration scores are read from Speedometer's own benchmark client. Velocity does not recalculate the official score.</p></body></html>");
    html
}

fn append_score_chart(html: &mut String, browsers: &[BrowserResult]) {
    let values = browsers
        .iter()
        .map(|browser| {
            let scores = browser
                .iterations
                .iter()
                .filter(|iteration| !iteration.warmup && iteration.completion_verified)
                .filter_map(|iteration| iteration.score.map(|score| (iteration.iteration, score)))
                .collect::<Vec<_>>();
            (browser, scores)
        })
        .filter(|(_, scores)| !scores.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let maximum = values
        .iter()
        .flat_map(|(_, scores)| scores.iter().map(|(_, score)| *score))
        .fold(0.0_f64, f64::max);
    let axis_maximum = if maximum > 0.0 { maximum * 1.05 } else { 1.0 };
    let height = 70 + values.len() * 62;
    let _ = write!(
        html,
        "<section class=\"card\"><h2>Measured run score distribution</h2><svg role=\"img\" aria-label=\"Measured Speedometer run scores; higher is better\" viewBox=\"0 0 900 {height}\" xmlns=\"http://www.w3.org/2000/svg\"><text x=\"10\" y=\"20\" font-size=\"13\">Score (axis starts at zero; higher is better)</text>"
    );
    for (index, (browser, scores)) in values.iter().enumerate() {
        let y = 42 + index * 62;
        let median = browser.summary.median_score;
        let median_x = median.map(|score| 250.0 + (score / axis_maximum * 560.0));
        let _ = write!(
            html,
            "<text x=\"10\" y=\"{}\" font-size=\"13\">{}</text>",
            y + 20,
            escape_html(&browser.browser.name),
        );
        for (score_index, (run, score)) in scores.iter().enumerate() {
            let x = 250.0 + (score / axis_maximum * 560.0);
            let jitter = (score_index % 5) * 4;
            let _ = write!(
                html,
                "<circle cx=\"{x:.2}\" cy=\"{}\" r=\"4\" fill=\"var(--chart-series)\"><title>{}: run {} score {:.3}</title></circle>",
                y + 22 + jitter,
                escape_html(&browser.browser.name),
                run,
                score
            );
        }
        if let Some(median_x) = median_x {
            let _ = write!(
                html,
                "<line x1=\"{median_x:.2}\" y1=\"{}\" x2=\"{median_x:.2}\" y2=\"{}\" stroke=\"var(--chart-median)\" stroke-width=\"2\"><title>Median: {:.3}</title></line>",
                y + 14,
                y + 42,
                median.expect("median x exists only when median exists")
            );
        }
        let _ = write!(
            html,
            "<text x=\"{}\" y=\"{}\" font-size=\"12\">median {}</text>",
            median_x.unwrap_or(820.0),
            y + 54,
            format_optional(median)
        );
    }
    html.push_str(
        "<line x1=\"250\" y1=\"35\" x2=\"250\" y2=\"100%\" stroke=\"currentColor\"/><text x=\"250\" y=\"100%\" font-size=\"11\">0</text></svg><p class=\"muted\">Each dot is one completed measured run; the vertical marker is the median. Warm-ups and failed runs are excluded.</p></section>",
    );
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(target_os = "macos")]
fn shuffle<T>(values: &mut [T], seed: &mut u64) {
    for index in (1..values.len()).rev() {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        values.swap(index, (*seed as usize) % (index + 1));
    }
}

#[cfg(target_os = "macos")]
fn random_seed() -> u64 {
    now_unix_ms() as u64 ^ ((std::process::id() as u64) << 32)
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
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
        Err("cannot install Speedometer cancellation handler".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
extern "C" fn handle_sigint(_: libc::c_int) {
    CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
fn compare(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut baseline = None;
    let mut current = None;
    let mut threshold = 5.0_f64;
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--baseline" if baseline.is_none() => {
                baseline = Some(PathBuf::from(required_value(&mut arguments, &option)?));
            }
            "--current" if current.is_none() => {
                current = Some(PathBuf::from(required_value(&mut arguments, &option)?));
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
            other => return Err(format!("unknown Speedometer comparison option '{other}'")),
        }
    }
    let baseline_path = baseline.ok_or("compare speedometer requires --baseline <report.json>")?;
    let current_path = current.ok_or("compare speedometer requires --current <report.json>")?;
    let baseline: SpeedometerReport = read_report(&baseline_path)?;
    let current: SpeedometerReport = read_report(&current_path)?;
    if baseline.benchmark != "speedometer" || current.benchmark != "speedometer" {
        return Err("both reports must contain the Speedometer benchmark".to_owned());
    }
    if baseline.benchmark_version != current.benchmark_version
        || baseline.source.version != current.source.version
        || baseline.configuration != current.configuration
        || baseline.environment.operating_system != current.environment.operating_system
        || baseline.environment.architecture != current.environment.architecture
    {
        return Err("Speedometer versions, run configuration, browser mode, source mode, or environment differ; reports are incompatible".to_owned());
    }
    if !threshold.is_finite() {
        return Err("--threshold must be finite".to_owned());
    }
    let mut regressions = 0;
    let mut comparisons = 0;
    for old in &baseline.browsers {
        let Some(new) = current
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
        if old.summary.valid_measured_runs < MIN_COMPARISON_RUNS
            || new.summary.valid_measured_runs < MIN_COMPARISON_RUNS
        {
            continue;
        }
        let change_percent = (new_median - old_median) / old_median * 100.0;
        let status = if change_percent < -threshold {
            regressions += 1;
            "REGRESSION"
        } else if change_percent > threshold {
            "IMPROVEMENT"
        } else {
            "within threshold"
        };
        comparisons += 1;
        println!(
            "{}: baseline {:.3}, current {:.3}, change {:+.2}% — {status}",
            old.browser.name, old_median, new_median, change_percent
        );
    }
    if comparisons == 0 {
        return Err(format!(
            "reports have no matching browser with at least {MIN_COMPARISON_RUNS} valid measured runs"
        ));
    }
    if regressions > 0 {
        Err(format!(
            "{regressions} Speedometer regression(s) exceed the {threshold:.1}% threshold"
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn read_report(path: &Path) -> Result<SpeedometerReport, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid Speedometer report {}: {error}", path.display()))
}

fn print_compare_help() {
    crate::terminal::print_markdown(
        "## `velocity compare speedometer`\n\n`velocity compare speedometer --baseline <report.json> --current <report.json> [--threshold PERCENT]`\n\nHigher scores are improvements. A decrease greater than the threshold is reported as a regression. Reports must have matching Speedometer versions and configurations, and at least three valid measured runs.",
    );
}

pub fn run_compare(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|value| matches!(value.as_str(), "--help" | "-h"))
    {
        print_compare_help();
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        compare(arguments)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arguments;
        Err("Speedometer regression comparison currently requires macOS".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_defaults_and_rejects_unsafe_values() {
        let options = parse_options(vec!["--browser".to_owned(), "chrome".to_owned()]).unwrap();
        assert_eq!(options.runs, DEFAULT_RUNS);
        assert_eq!(options.warmups, DEFAULT_WARMUPS);
        assert_eq!(options.timeout.as_secs(), DEFAULT_TIMEOUT_SECONDS);
        assert!(parse_options(vec!["--runs".to_owned(), "0".to_owned()]).is_err());
        assert!(parse_options(vec!["--timeout".to_owned(), "10".to_owned()]).is_err());
        assert!(
            parse_options(vec!["--benchmark-source".to_owned(), "modified".to_owned()]).is_err()
        );
        assert!(
            parse_options(vec![
                "--browser".to_owned(),
                "chrome".to_owned(),
                "--all".to_owned()
            ])
            .is_err()
        );
    }

    #[test]
    fn summary_statistics_exclude_warmups_and_failures() {
        let stats = summarize_metric_values([10.0, 12.0]);
        assert_eq!(stats.unwrap().median, 11.0);
    }

    #[test]
    fn html_report_escapes_browser_metadata_and_marks_higher_is_better() {
        let iteration = |iteration, warmup, score| SpeedometerIteration {
            iteration,
            warmup,
            started_at_unix_ms: 0,
            score: Some(score),
            score_unit: "official Speedometer score".to_owned(),
            benchmark_version: Some("3.1".to_owned()),
            official_internal_iteration_scores: Vec::new(),
            execution_duration_ms: None,
            completion_verified: true,
            warnings: Vec::new(),
            failure: None,
            cleanup: CleanupStatus {
                browser_closed: true,
                profile_removed: true,
                automation_resources_removed: true,
                detail: None,
            },
        };
        let browser = BrowserResult {
            browser: ReportBrowser {
                id: "chrome".to_owned(),
                installation_id: "chrome-test".to_owned(),
                name: "Chrome <test>".to_owned(),
                version: Some("1".to_owned()),
                executable_path: PathBuf::from("/browser"),
            },
            engine_family: Some("Chromium".to_owned()),
            automation_method: "CDP".to_owned(),
            iterations: vec![
                iteration(1, true, 99.0),
                iteration(2, false, 11.0),
                iteration(3, false, 13.0),
            ],
            summary: BrowserSummary {
                valid_measured_runs: 3,
                failed_measured_runs: 0,
                median_score: Some(12.0),
                mean_score: Some(12.0),
                minimum_score: Some(11.0),
                maximum_score: Some(13.0),
                sample_standard_deviation: Some(1.0),
                coefficient_of_variation_percent: Some(8.3),
            },
        };
        let report = SpeedometerReport {
            schema_version: 1,
            benchmark: "speedometer".to_owned(),
            benchmark_version: "3.1".to_owned(),
            source: SourceMetadata {
                kind: "official_hosted".to_owned(),
                url: "https://browserbench.org/Speedometer3.1/".to_owned(),
                version: "3.1".to_owned(),
                official_commit: None,
                archive_sha256: None,
            },
            configuration: BenchmarkConfiguration {
                requested_runs: 3,
                warmup_runs: 1,
                timeout_seconds: 600,
                browser_mode: "headed".to_owned(),
                source_kind: "official_hosted".to_owned(),
                official_internal_iterations: 10,
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
            browsers: vec![browser],
            comparison: ComparisonAssessment {
                status: "INCONCLUSIVE".to_owned(),
                higher_is_better: true,
                descriptive_ranking_allowed: false,
                reasons: vec!["single browser".to_owned()],
            },
            cancelled: false,
            generated_at_unix_ms: 0,
        };
        let html = render_html(&report);
        assert!(html.contains("Higher score is better"));
        assert!(html.contains("Chrome &lt;test&gt;"));
        assert!(!html.contains("Chrome <test>"));
        assert!(!html.contains("https://cdn"));
        assert!(html.contains("Measured run score distribution"));
        assert!(html.contains("run 2 score 11.000"));
        assert!(html.contains("run 3 score 13.000"));
        assert!(!html.contains("run 1 score 99.000"));
    }
}
