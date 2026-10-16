use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use velocity_core::{
    CleanupStatus, FixtureKind, ReportBrowser, ResourceBenchmarkConfig, ResourceBenchmarkFailure,
    ResourceBenchmarkIteration, ResourceBenchmarkReport, ResourceFailureKind, ResourceWorkload,
    RunOrderMetadata, unique_run_id,
};

#[cfg(target_os = "macos")]
use crate::resource_report::{
    display_path, format_mib, format_percent, print_report_summary, title, write_reports,
};

#[cfg(target_os = "macos")]
static RESOURCE_CANCELLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Debug)]
enum TargetSelection {
    One(String),
    Many(Vec<String>),
    All,
    Prompt,
}

#[derive(Clone, Debug)]
struct Options {
    workload: ResourceWorkload,
    target: TargetSelection,
    config: ResourceBenchmarkConfig,
    output_directory: PathBuf,
    record_history: bool,
}

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let Some(workload_name) = arguments.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    if matches!(workload_name, "--help" | "-h" | "help") {
        print_help();
        return Ok(());
    }
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        print_help();
        return Ok(());
    }
    let workload = match workload_name {
        "idle" => ResourceWorkload::Idle,
        "page" => ResourceWorkload::Page,
        "tabs" => ResourceWorkload::Tabs,
        other => return Err(format!("unknown resource benchmark '{other}'")),
    };
    let options = parse_options(workload, arguments.into_iter().skip(1))?;
    run_resource_benchmark(options)
}

fn parse_options(
    workload: ResourceWorkload,
    mut arguments: impl Iterator<Item = String>,
) -> Result<Options, String> {
    let mut browser = None;
    let mut browsers = None;
    let mut all = false;
    let mut config = ResourceBenchmarkConfig::defaults(workload);
    let mut output_directory = default_output_directory();
    let mut seen = BTreeSet::new();
    let mut record_history = true;
    while let Some(option) = arguments.next() {
        if !seen.insert(option.clone()) {
            return Err(format!("{option} may be specified only once"));
        }
        match option.as_str() {
            "--browser" => browser = Some(required_value(&mut arguments, &option)?),
            "--browsers" => {
                let value = required_value(&mut arguments, &option)?;
                let parsed = value.split(',').map(str::trim).collect::<Vec<_>>();
                if parsed.is_empty() || parsed.iter().any(|value| value.is_empty()) {
                    return Err(
                        "--browsers requires comma-separated browser identifiers".to_owned()
                    );
                }
                browsers = Some(parsed.into_iter().map(str::to_owned).collect());
            }
            "--all" => all = true,
            "--runs" => config.runs = parse_value(&mut arguments, &option)?,
            "--warmup" => config.warmup_iterations = parse_value(&mut arguments, &option)?,
            "--randomize" => config.randomized = true,
            "--duration" => {
                config.measurement_duration_ms = parse_seconds_ms(&mut arguments, &option)?
            }
            "--settle" => config.settle_period_ms = parse_seconds_ms(&mut arguments, &option)?,
            "--interval" => config.sampling_interval_ms = parse_value(&mut arguments, &option)?,
            "--timeout" => {
                let timeout = parse_seconds_ms(&mut arguments, &option)?;
                config.navigation_timeout_ms = timeout;
                config.readiness_timeout_ms = timeout;
            }
            "--navigation-timeout" => {
                config.navigation_timeout_ms = parse_seconds_ms(&mut arguments, &option)?
            }
            "--readiness-timeout" => {
                config.readiness_timeout_ms = parse_seconds_ms(&mut arguments, &option)?
            }
            "--fixture" => {
                config.fixture = Some(parse_fixture(&required_value(&mut arguments, &option)?)?)
            }
            "--tabs" => {
                config.tab_counts = parse_tab_counts(&required_value(&mut arguments, &option)?)?
            }
            "--intensity" => config.dynamic_intensity = parse_value(&mut arguments, &option)?,
            "--update-interval" => {
                config.dynamic_update_interval_ms = parse_value(&mut arguments, &option)?
            }
            "--output" => {
                output_directory = PathBuf::from(required_value(&mut arguments, &option)?)
            }
            "--record-history" => record_history = true,
            "--no-record-history" => record_history = false,
            other => {
                return Err(format!(
                    "unknown {workload_name_option} option '{other}'",
                    workload_name_option = workload.id()
                ));
            }
        }
    }
    if seen.contains("--timeout")
        && (seen.contains("--navigation-timeout") || seen.contains("--readiness-timeout"))
    {
        return Err(
            "use --timeout by itself or specify navigation/readiness timeouts separately"
                .to_owned(),
        );
    }
    if seen.contains("--record-history") && seen.contains("--no-record-history") {
        return Err("choose only one of --record-history and --no-record-history".to_owned());
    }
    let target = match (browser, browsers, all) {
        (Some(browser), None, false) => TargetSelection::One(browser),
        (None, Some(browsers), false) => TargetSelection::Many(browsers),
        (None, None, true) => TargetSelection::All,
        (None, None, false) => TargetSelection::Prompt,
        _ => return Err("use exactly one of --browser, --browsers, or --all".to_owned()),
    };
    config.validate(workload)?;
    Ok(Options {
        workload,
        target,
        config,
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

fn parse_seconds_ms(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<u64, String> {
    let seconds: u64 = parse_value(arguments, option)?;
    seconds
        .checked_mul(1_000)
        .ok_or_else(|| format!("{option} is too large"))
}

fn parse_fixture(value: &str) -> Result<FixtureKind, String> {
    match value {
        "simple" => Ok(FixtureKind::Simple),
        "complex" => Ok(FixtureKind::Complex),
        "dynamic" => Ok(FixtureKind::Dynamic),
        other => Err(format!(
            "unknown fixture '{other}'; choose simple, complex, or dynamic"
        )),
    }
}

fn parse_tab_counts(value: &str) -> Result<Vec<usize>, String> {
    value
        .split(',')
        .map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return Err("--tabs requires comma-separated positive counts".to_owned());
            }
            part.parse::<usize>()
                .map_err(|_| format!("invalid tab count '{part}'"))
        })
        .collect()
}

fn default_output_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("tools/velocity"))
        .join("results")
}

fn validate_total_sample_bound(
    config: &ResourceBenchmarkConfig,
    workload: ResourceWorkload,
    browser_count: usize,
) -> Result<usize, String> {
    let scenario_count = if workload == ResourceWorkload::Tabs {
        config.tab_counts.len()
    } else {
        1
    };
    let samples_per_iteration = usize::try_from(
        config
            .measurement_duration_ms
            .div_ceil(config.sampling_interval_ms),
    )
    .unwrap_or(usize::MAX);
    let requested_samples = samples_per_iteration
        .checked_mul(config.runs)
        .and_then(|samples| samples.checked_mul(browser_count))
        .and_then(|samples| samples.checked_mul(scenario_count))
        .unwrap_or(usize::MAX);
    if requested_samples > velocity_core::MAX_RETAINED_RESOURCE_SAMPLES {
        return Err(format!(
            "requested benchmark would retain up to {requested_samples} samples; increase --interval or reduce runs/browser/tab scenarios to stay at or below {} total samples",
            velocity_core::MAX_RETAINED_RESOURCE_SAMPLES
        ));
    }
    Ok(requested_samples)
}

fn print_help() {
    crate::terminal::print_markdown(
        "## Resource benchmarks\n\n`velocity bench idle [--browser <id|number>] [OPTIONS]`\n\n`velocity bench page [--browser <id|number>] --fixture <simple|complex|dynamic> [OPTIONS]`\n\n`velocity bench tabs [--browser <id|number>] --tabs 1,5,10 --fixture <name> [OPTIONS]`\n\nIf no browser target is provided in an interactive terminal, Velocity shows a numbered picker. It accepts comma-separated numbers or `all` for sequential comparisons. Use `--browsers <id|number,...>` or `--all` for scripted comparisons. Options: `--runs N` (5), `--warmup N` (0), `--randomize`, `--duration SECONDS` (10), `--settle SECONDS` (5), `--interval MILLISECONDS` (100), `--navigation-timeout SECONDS` (30), `--readiness-timeout SECONDS` (30), `--timeout SECONDS` sets both, `--intensity N` (25), `--update-interval MILLISECONDS` (100), `--output DIRECTORY`.",
    );
}

#[cfg(not(target_os = "macos"))]
fn run_resource_benchmark(_options: Options) -> Result<(), String> {
    Err("automated resource benchmarks currently require macOS".to_owned())
}

#[cfg(target_os = "macos")]
fn run_resource_benchmark(options: Options) -> Result<(), String> {
    use std::{fs, sync::atomic::Ordering};

    use velocity_browser::macos::default_session_store_path;

    RESOURCE_CANCELLED.store(false, Ordering::SeqCst);
    install_cancellation_handler()?;

    let registry = super::discover_browsers()?;
    let installations = resolve_installations(&registry, &options.target)?;
    if installations.is_empty() {
        return Err("no supported browser installations were found".to_owned());
    }
    validate_total_sample_bound(&options.config, options.workload, installations.len())?;
    fs::create_dir_all(&options.output_directory).map_err(|error| {
        format!(
            "cannot create result directory {}: {error}",
            options.output_directory.display()
        )
    })?;

    let fixture_server = match options.workload {
        ResourceWorkload::Idle => None,
        ResourceWorkload::Page | ResourceWorkload::Tabs => {
            Some(crate::fixture_server::FixtureServer::start()?)
        }
    };
    let session_store_path = default_session_store_path()?;
    let profile_root = session_store_path
        .parent()
        .ok_or_else(|| "Velocity session store has no parent directory".to_owned())?
        .join("Profiles");
    let run_id = unique_run_id();
    let seed = options.config.randomized.then(random_seed);
    let mut random = seed.unwrap_or_default();
    let tab_scenarios = match options.workload {
        ResourceWorkload::Idle => vec![None],
        ResourceWorkload::Page => vec![Some(1)],
        ResourceWorkload::Tabs => options
            .config
            .tab_counts
            .iter()
            .copied()
            .map(Some)
            .collect(),
    };
    let warmup_rounds = make_rounds(
        &installations,
        &tab_scenarios,
        options.config.warmup_iterations,
        options.config.randomized,
        &mut random,
    );
    let measured_rounds = make_rounds(
        &installations,
        &tab_scenarios,
        options.config.runs,
        options.config.randomized,
        &mut random,
    );
    println!(
        "Velocity — {} Resource Benchmark\n",
        title(options.workload)
    );
    println!(
        "Browsers: {}",
        installations
            .iter()
            .map(|installation| installation.metadata.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Some(fixture) = options.config.fixture {
        println!(
            "Fixture: {} v{}",
            fixture.name(),
            velocity_core::FIXTURE_VERSION
        );
    }
    if options.workload == ResourceWorkload::Tabs {
        println!(
            "Tabs: {}",
            options
                .config
                .tab_counts
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!(
        "Iterations: {} measured, {} warm-up",
        options.config.runs, options.config.warmup_iterations
    );
    println!(
        "Duration: {} seconds; settling: {} seconds; sample interval: {} ms\n",
        options.config.measurement_duration_ms / 1_000,
        options.config.settle_period_ms / 1_000,
        options.config.sampling_interval_ms
    );

    let mut iterations = Vec::new();
    let mut per_scenario_count = BTreeMap::<(String, Option<usize>, bool), usize>::new();
    let execution_environment = IterationEnvironment {
        config: &options.config,
        fixture_server: fixture_server.as_ref(),
        profile_root: &profile_root,
        session_store_path: &session_store_path,
        cancelled: &RESOURCE_CANCELLED,
    };
    let rounds = warmup_rounds
        .into_iter()
        .map(|round| (round, true))
        .chain(measured_rounds.into_iter().map(|round| (round, false)));
    let mut measured_progress = 0;
    let measured_total = options.config.runs * installations.len() * tab_scenarios.len();
    let mut actual_warmup_order = Vec::new();
    let mut actual_measured_order = Vec::new();
    for (round, warmup) in rounds {
        let mut completed_order = Vec::new();
        for (installation, tabs) in round {
            if !warmup {
                measured_progress += 1;
                println!(
                    "[{}/{}] {}{}",
                    measured_progress,
                    measured_total,
                    installation.metadata.name,
                    tabs.map(|tabs| format!(" — {tabs} tab(s)"))
                        .unwrap_or_default()
                );
            } else {
                println!(
                    "[warm-up] {}{}",
                    installation.metadata.name,
                    tabs.map(|tabs| format!(" — {tabs} tab(s)"))
                        .unwrap_or_default()
                );
            }
            let key = (installation.installation_id.to_string(), tabs, warmup);
            let iteration_number = per_scenario_count.entry(key).or_default();
            *iteration_number += 1;
            let iteration = execute_iteration(
                &installation,
                options.workload,
                tabs,
                *iteration_number,
                warmup,
                &execution_environment,
            );
            if let Some(failure) = &iteration.failure {
                println!(
                    "      {}: {}",
                    if failure.kind == ResourceFailureKind::Unsupported {
                        "Unsupported"
                    } else {
                        "Failed"
                    },
                    failure.message
                );
            } else if warmup {
                println!("      Workload ready; warm-up excluded from statistics.");
            } else if let Some(resource) = &iteration.resource {
                println!(
                    "      Mean CPU: {}; mean footprint: {}",
                    format_percent(resource.statistics.cpu_mean_percent),
                    format_mib(resource.statistics.physical_footprint_mean_bytes)
                );
            }
            if let Some(detail) = &iteration.cleanup.detail {
                println!("      Cleanup warning: {detail}");
            }
            completed_order.push(scenario_id(&installation, tabs));
            iterations.push(iteration);
            if RESOURCE_CANCELLED.load(Ordering::Relaxed) {
                break;
            }
        }
        if !completed_order.is_empty() {
            if warmup {
                actual_warmup_order.push(completed_order);
            } else {
                actual_measured_order.push(completed_order);
            }
        }
        if RESOURCE_CANCELLED.load(Ordering::Relaxed) {
            break;
        }
    }
    let cancelled = RESOURCE_CANCELLED.load(Ordering::Relaxed);
    let run_order = RunOrderMetadata {
        comparison_run_id: Some(run_id.clone()),
        randomized: options.config.randomized,
        randomization_seed: seed,
        warmup_order_by_round: actual_warmup_order,
        measured_order_by_round: actual_measured_order,
    };
    let report = ResourceBenchmarkReport::new(
        options.workload,
        options.config.clone(),
        run_order,
        iterations,
        cancelled,
    );
    print_report_summary(&report);
    let (json_path, html_path) = write_reports(&options.output_directory, &run_id, &report)?;
    println!("\nSaved JSON: {}", display_path(&json_path));
    println!("Saved HTML: {}", display_path(&html_path));
    if options.record_history
        && report.iterations.iter().any(|iteration| {
            !iteration.warmup && iteration.resource.is_some() && iteration.failure.is_none()
        })
    {
        super::history_cli::record_after_success(&json_path);
    }
    drop(fixture_server);
    if report
        .iterations
        .iter()
        .all(|iteration| iteration.warmup || iteration.resource.is_none())
    {
        return Err(format!(
            "no measured resource iterations succeeded; reports were saved to {} and {}",
            json_path.display(),
            html_path.display()
        ));
    }
    if cancelled {
        println!("Benchmark cancelled after cleaning up the active managed session.");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn resolve_installations(
    registry: &velocity_browser::BrowserRegistry,
    target: &TargetSelection,
) -> Result<Vec<velocity_browser::BrowserInstallation>, String> {
    use velocity_browser::BrowserInstallation;
    let selected = match target {
        TargetSelection::One(selector) => {
            vec![super::browser_selection::resolve(registry, selector)?.clone()]
        }
        TargetSelection::Many(selectors) => selectors
            .iter()
            .map(|selector| super::browser_selection::resolve(registry, selector).cloned())
            .collect::<Result<Vec<BrowserInstallation>, String>>()?,
        TargetSelection::All => registry.installations().cloned().collect(),
        TargetSelection::Prompt => super::browser_selection::choose(registry, None, true)?
            .into_iter()
            .cloned()
            .collect(),
    };
    let mut unique = Vec::new();
    for installation in selected {
        if !unique.iter().any(|existing: &BrowserInstallation| {
            existing.installation_id == installation.installation_id
        }) {
            unique.push(installation);
        }
    }
    Ok(unique)
}

#[cfg(target_os = "macos")]
fn make_rounds(
    installations: &[velocity_browser::BrowserInstallation],
    tab_scenarios: &[Option<usize>],
    count: usize,
    randomized: bool,
    seed: &mut u64,
) -> Vec<Vec<(velocity_browser::BrowserInstallation, Option<usize>)>> {
    (0..count)
        .map(|_| {
            let mut round = Vec::new();
            for tabs in tab_scenarios {
                let mut group = installations.to_vec();
                if randomized {
                    shuffle(&mut group, seed);
                }
                round.extend(group.into_iter().map(|installation| (installation, *tabs)));
            }
            round
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn shuffle<T>(values: &mut [T], state: &mut u64) {
    for index in (1..values.len()).rev() {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        values.swap(index, (*state as usize) % (index + 1));
    }
}

#[cfg(target_os = "macos")]
fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (nanos as u64) ^ ((std::process::id() as u64) << 32)
}

#[cfg(target_os = "macos")]
fn scenario_id(
    installation: &velocity_browser::BrowserInstallation,
    tabs: Option<usize>,
) -> String {
    match tabs {
        Some(tabs) => format!("{}:{tabs}", installation.installation_id),
        None => installation.installation_id.to_string(),
    }
}

#[cfg(target_os = "macos")]
struct IterationEnvironment<'a> {
    config: &'a ResourceBenchmarkConfig,
    fixture_server: Option<&'a crate::fixture_server::FixtureServer>,
    profile_root: &'a Path,
    session_store_path: &'a Path,
    cancelled: &'static std::sync::atomic::AtomicBool,
}

#[cfg(target_os = "macos")]
fn execute_iteration(
    installation: &velocity_browser::BrowserInstallation,
    workload: ResourceWorkload,
    tab_count: Option<usize>,
    iteration: usize,
    warmup: bool,
    environment: &IterationEnvironment<'_>,
) -> ResourceBenchmarkIteration {
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use velocity_browser::{
        BrowserLifecycleManager, FileSessionStore, browser_capabilities,
        macos::{MacOSProcessDriver, MacOSResourceCollector, MacOSWindowObserver},
    };
    use velocity_core::{CancellationToken, StartupMetric, observe_startup_windows};
    use velocity_metrics::{ManagedProcessTarget, ResourceConfig, ResourceMonitor};

    let config = environment.config;
    let fixture_server = environment.fixture_server;
    let profile_root = environment.profile_root;
    let session_store_path = environment.session_store_path;
    let cancelled = environment.cancelled;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut result = ResourceBenchmarkIteration {
        browser: ReportBrowser::from(installation),
        iteration,
        warmup,
        tab_count,
        automation_protocol: None,
        profile_isolation: "isolated Velocity profile required; launch not started".to_owned(),
        started_at_unix_ms: timestamp,
        resource: None,
        failure: None,
        cleanup: CleanupStatus {
            browser_closed: true,
            profile_removed: true,
            detail: None,
        },
    };
    let capabilities = browser_capabilities(installation);
    let is_photon = installation.metadata.id.as_str() == "photon";
    let unsupported = if !capabilities.resource_monitoring {
        Some("resource monitoring is unavailable on this platform".to_owned())
    } else if !capabilities.isolated_profile {
        Some(format!(
            "{} has no verified isolated-profile launch adapter",
            installation.metadata.name
        ))
    } else if workload != ResourceWorkload::Idle
        && (!capabilities.launch_url_navigation || !capabilities.fixture_readiness_beacon)
    {
        Some(format!(
            "{} cannot verify local fixture readiness",
            installation.metadata.name
        ))
    } else if workload == ResourceWorkload::Tabs && !capabilities.tab_creation && !is_photon {
        Some(
            capabilities
                .tab_limitation
                .unwrap_or_else(|| "tab automation is unsupported".to_owned()),
        )
    } else {
        None
    };
    if let Some(message) = unsupported {
        result.profile_isolation = "not used; browser capability is unsupported".to_owned();
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Unsupported,
            message,
        });
        return result;
    }
    if cancelled.load(Ordering::Relaxed) {
        result.profile_isolation = "not used; cancelled before launch".to_owned();
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Cancelled,
            message: "cancelled before iteration launch".to_owned(),
        });
        return result;
    }

    let run_id = unique_run_id();
    let profile_path = profile_root.join(format!(
        "resource-{}-{run_id}",
        installation.installation_id
    ));
    if fs::symlink_metadata(&profile_path).is_ok() {
        result.profile_isolation =
            "not used; refusing to reuse an existing profile path".to_owned();
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Setup,
            message: format!(
                "refusing to reuse existing benchmark profile {}",
                profile_path.display()
            ),
        });
        return result;
    }
    let devtools_enabled = workload == ResourceWorkload::Tabs && !is_photon;
    let photon_token = if is_photon {
        match secure_random_token() {
            Ok(token) => Some(token),
            Err(message) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Setup,
                    message,
                });
                return result;
            }
        }
    } else {
        None
    };
    let photon_socket = photon_token.as_ref().map(|token| {
        PathBuf::from("/tmp")
            .join(format!("vlt-{}", &token[..12]))
            .join("p.sock")
    });
    let automation_directory = photon_socket
        .as_ref()
        .and_then(|socket| socket.parent())
        .map(Path::to_path_buf);
    let mut driver = MacOSProcessDriver::new(profile_root.to_path_buf())
        .with_benchmark_profile(profile_path.clone(), false);
    if devtools_enabled {
        driver = driver.with_chromium_devtools();
    }
    if let (Some(socket), Some(token)) = (&photon_socket, photon_token.as_ref()) {
        driver = driver.with_photon_automation(socket.clone(), token.clone());
    }
    let manager = BrowserLifecycleManager::new(
        driver.clone(),
        FileSessionStore::new(session_store_path.to_path_buf()),
    );

    let token = run_id.replace(
        |character: char| !character.is_ascii_alphanumeric() && character != '-',
        "-",
    );
    let urls = match (workload, fixture_server, config.fixture, tab_count) {
        (ResourceWorkload::Idle, _, _, _) => Vec::new(),
        (ResourceWorkload::Page, Some(server), Some(fixture), _) => vec![server.url(
            fixture,
            &token,
            0,
            config.dynamic_intensity,
            config.dynamic_update_interval_ms,
        )],
        (ResourceWorkload::Tabs, Some(server), Some(fixture), Some(count)) => (0..count)
            .map(|tab| {
                server.url(
                    fixture,
                    &token,
                    tab,
                    config.dynamic_intensity,
                    config.dynamic_update_interval_ms,
                )
            })
            .collect(),
        _ => {
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Setup,
                message: "fixture server or workload configuration is missing".to_owned(),
            });
            return result;
        }
    };
    let launch_url = if is_photon {
        Some("about:blank")
    } else {
        match workload {
            ResourceWorkload::Idle => Some("about:blank"),
            ResourceWorkload::Page => urls.first().map(String::as_str),
            ResourceWorkload::Tabs => Some("about:blank"),
        }
    };
    println!("      Preparing isolated session and launching...");
    let mut launch_timing = None;
    let launch = manager.launch_observed(installation, launch_url, &mut |timing| {
        launch_timing = Some(timing)
    });
    let session = match launch {
        Ok(session) => session,
        Err(error) => {
            result.profile_isolation =
                "isolated-profile launch attempted; managed session was not established".to_owned();
            result.cleanup.profile_removed = !profile_path.exists();
            result.cleanup.detail = (!result.cleanup.profile_removed).then(|| {
                format!(
                    "launch failed and profile remains at {}",
                    profile_path.display()
                )
            });
            if let Err(cleanup_error) = cleanup_automation_directory(
                automation_directory.as_deref(),
                photon_token.as_deref(),
            ) {
                result.cleanup.detail = Some(match result.cleanup.detail.take() {
                    Some(existing) => format!("{existing}; {cleanup_error}"),
                    None => cleanup_error,
                });
            }
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Launch,
                message: error.to_string(),
            });
            return result;
        }
    };
    result.cleanup.browser_closed = false;
    result.cleanup.profile_removed = false;
    result.profile_isolation = "fresh private Velocity profile for this iteration".to_owned();
    let cleanup = |result: &mut ResourceBenchmarkIteration| {
        cleanup_iteration(
            IterationCleanup {
                manager: &manager,
                driver: &driver,
                session_id: &session.session_id,
                identity: &session.identity,
                profile_path: &profile_path,
                automation_directory: automation_directory.as_deref(),
                automation_token: photon_token.as_deref(),
            },
            result,
        );
    };
    result.automation_protocol = Some(if is_photon {
        "Photon Automation Protocol v1 (authenticated Unix domain socket)".to_owned()
    } else {
        match workload {
            ResourceWorkload::Idle => "macOS Quartz verified-window observer".to_owned(),
            ResourceWorkload::Page => {
                "isolated launch URL plus local fixture navigation/readiness signals".to_owned()
            }
            ResourceWorkload::Tabs => "Chrome DevTools Protocol".to_owned(),
        }
    });

    let mut photon_automation = if is_photon {
        let socket = photon_socket.as_ref().expect("Photon socket path prepared");
        let token_path = socket
            .parent()
            .expect("Photon socket has parent")
            .join("token");
        match velocity_browser::PhotonAutomationClient::connect_with_cancel(
            socket,
            &token_path,
            Duration::from_millis(config.readiness_timeout_ms),
            || cancelled.load(Ordering::Relaxed),
        ) {
            Ok(client) => Some(client),
            Err(error) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: if cancelled.load(Ordering::Relaxed) {
                        ResourceFailureKind::Cancelled
                    } else {
                        ResourceFailureKind::Automation
                    },
                    message: error.to_string(),
                });
                cleanup(&mut result);
                return result;
            }
        }
    } else {
        None
    };
    if let Some(client) = photon_automation.as_ref() {
        let required = if workload == ResourceWorkload::Tabs {
            ["list_tabs", "new_tab", "navigate"].as_slice()
        } else if workload == ResourceWorkload::Page {
            ["list_tabs", "navigate"].as_slice()
        } else {
            ["list_tabs"].as_slice()
        };
        if let Some(missing) = required
            .iter()
            .find(|capability| !client.supports(capability))
        {
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Unsupported,
                message: format!(
                    "Photon protocol negotiation succeeded, but the server does not advertise required capability '{missing}'"
                ),
            });
            cleanup(&mut result);
            return result;
        }
    }

    let readiness = match workload {
        ResourceWorkload::Idle => {
            if let Some(client) = photon_automation.as_mut() {
                match client.list_tabs(Duration::from_secs(5)) {
                    Ok(tabs)
                        if tabs.len() == 1 && tabs[0].url.as_deref() == Some("about:blank") => {}
                    Ok(tabs) => {
                        let message = format!(
                            "Photon idle session expected one about:blank tab, observed {} tab(s) with URL {}",
                            tabs.len(),
                            tabs.first()
                                .and_then(|tab| tab.url.as_deref())
                                .unwrap_or("unavailable")
                        );
                        result.failure = Some(ResourceBenchmarkFailure {
                            kind: ResourceFailureKind::Automation,
                            message: message.clone(),
                        });
                        cleanup(&mut result);
                        return result;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        result.failure = Some(ResourceBenchmarkFailure {
                            kind: ResourceFailureKind::Automation,
                            message: message.clone(),
                        });
                        cleanup(&mut result);
                        return result;
                    }
                }
            }
            let timing = launch_timing.as_ref();
            match timing {
                Some(timing) => {
                    let cancellation = CancellationToken::default();
                    let watcher_done = Arc::new(AtomicBool::new(false));
                    let watcher_token = cancellation.clone();
                    let watcher_done_thread = Arc::clone(&watcher_done);
                    let watcher = thread::spawn(move || {
                        while !watcher_done_thread.load(Ordering::Relaxed) {
                            if cancelled.load(Ordering::Relaxed) {
                                watcher_token.cancel();
                                break;
                            }
                            thread::sleep(Duration::from_millis(20));
                        }
                    });
                    let outcome = observe_startup_windows(
                        timing,
                        &MacOSWindowObserver,
                        Duration::from_millis(config.readiness_timeout_ms),
                        &cancellation,
                    );
                    watcher_done.store(true, Ordering::Relaxed);
                    let _ = watcher.join();
                    if outcome
                        .measurements_ms
                        .contains_key(&StartupMetric::WindowReady)
                    {
                        Ok(())
                    } else {
                        let reason = outcome
                            .failures
                            .first()
                            .map(|failure| failure.message.clone())
                            .unwrap_or_else(|| {
                                "verified browser window readiness was not observed".to_owned()
                            });
                        Err(reason)
                    }
                }
                None => Err(
                    "launch adapter did not report process timing for window readiness".to_owned(),
                ),
            }
        }
        ResourceWorkload::Page => {
            let navigation = if let Some(client) = photon_automation.as_mut() {
                client
                    .list_tabs(Duration::from_secs(5))
                    .map_err(|error| error.to_string())
                    .and_then(|tabs| {
                        if tabs.len() != 1 {
                            return Err(format!(
                                "Photon page session expected one initial tab, observed {}",
                                tabs.len()
                            ));
                        }
                        client
                            .navigate(tabs[0].tab_id, &urls[0], Duration::from_secs(10))
                            .map_err(|error| error.to_string())
                    })
            } else {
                Ok(())
            };
            navigation.and_then(|()| {
                fixture_server
                    .ok_or_else(|| "page fixture server was not started".to_owned())
                    .and_then(|server| wait_for_fixture(server, &token, &[0], config, cancelled))
            })
        }
        ResourceWorkload::Tabs => {
            let tabs = urls
                .iter()
                .enumerate()
                .map(|(tab, _)| tab)
                .collect::<Vec<_>>();
            if let Some(client) = photon_automation.as_mut() {
                let mut created_ids = Vec::new();
                (|| {
                    let initial_tabs = client
                        .list_tabs(Duration::from_secs(5))
                        .map_err(|error| error.to_string())?;
                    if initial_tabs.len() != 1 {
                        return Err(format!(
                            "Photon multi-tab session expected one initial tab, observed {}",
                            initial_tabs.len()
                        ));
                    }
                    client
                        .navigate(initial_tabs[0].tab_id, &urls[0], Duration::from_secs(10))
                        .map_err(|error| error.to_string())?;
                    created_ids.push(initial_tabs[0].tab_id);
                    for url in urls.iter().skip(1) {
                        let tab = client
                            .new_tab(url, Duration::from_secs(10))
                            .map_err(|error| error.to_string())?;
                        created_ids.push(tab.tab_id);
                    }
                    let verified = client
                        .list_tabs(Duration::from_secs(5))
                        .map_err(|error| error.to_string())?;
                    if Some(verified.len()) != tab_count || verified.len() != created_ids.len() {
                        return Err(format!(
                            "requested {} Photon tabs but list_tabs verified {}",
                            tab_count.unwrap_or_default(),
                            verified.len()
                        ));
                    }
                    if created_ids
                        .iter()
                        .any(|id| !verified.iter().any(|tab| tab.tab_id == *id))
                    {
                        return Err(
                            "Photon did not retain every newly created benchmark tab".to_owned()
                        );
                    }
                    let server = fixture_server
                        .ok_or_else(|| "tab fixture server was not started".to_owned())?;
                    wait_for_fixture(server, &token, &tabs, config, cancelled)?;
                    for (index, tab_id) in created_ids.iter().enumerate() {
                        let status = client
                            .tab_status(*tab_id, Duration::from_secs(5))
                            .map_err(|error| error.to_string())?;
                        if !status.url.as_deref().is_some_and(|url| {
                            url.contains(&format!("run={token}"))
                                && url.contains(&format!("tab={index}"))
                        }) {
                            return Err(format!(
                                "Photon tab {} did not retain its assigned fixture URL",
                                index + 1
                            ));
                        }
                    }
                    Ok(())
                })()
            } else {
                let automation = velocity_browser::ChromiumDevTools::connect_with_cancel(
                    &profile_path,
                    Duration::from_millis(config.navigation_timeout_ms),
                    || cancelled.load(Ordering::Relaxed),
                )
                .and_then(|mut client| {
                    let server = fixture_server.ok_or_else(|| {
                        velocity_browser::AutomationError(
                            "tab fixture server was not started".to_owned(),
                        )
                    })?;
                    open_tabs_sequentially(&mut client, &urls, |tab, target_id, automation| {
                        server
                            .wait_for_navigation_with_cancel(
                                &token,
                                &[tab],
                                Duration::from_millis(config.navigation_timeout_ms),
                                || cancelled.load(Ordering::Relaxed),
                            )
                            .map_err(velocity_browser::AutomationError)?;
                        automation
                            .wait_for_fixture_ready(
                                target_id,
                                &urls[tab],
                                Duration::from_millis(config.readiness_timeout_ms),
                                || cancelled.load(Ordering::Relaxed),
                            )
                            .map_err(|error| velocity_browser::AutomationError(error.to_string()))
                    })?;
                    Ok(client)
                })
                .map_err(|error| error.to_string());
                match automation {
                    Ok(mut client) => {
                        use velocity_browser::BrowserAutomationAdapter;
                        let verified_count = client.page_count();
                        match verified_count {
                            Ok(count) if Some(count) == tab_count => match fixture_server {
                                Some(_) => Ok(()),
                                None => Err("tab fixture server was not started".to_owned()),
                            },
                            Ok(count) => Err(format!(
                                "requested {} tabs but CDP verified {count}",
                                tab_count.unwrap_or_default()
                            )),
                            Err(error) => Err(error),
                        }
                    }
                    Err(error) => Err(error),
                }
            }
        }
    };
    if let Err(message) = readiness {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: if cancelled.load(Ordering::Relaxed) {
                ResourceFailureKind::Cancelled
            } else if message.contains("timed out") {
                ResourceFailureKind::ReadinessTimeout
            } else {
                ResourceFailureKind::Automation
            },
            message,
        });
        cleanup(&mut result);
        return result;
    }
    println!("      Workload ready.");
    if !wait_cancellable(Duration::from_millis(config.settle_period_ms), cancelled) {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Cancelled,
            message: "benchmark cancelled during settling period".to_owned(),
        });
        cleanup(&mut result);
        return result;
    }
    if warmup {
        cleanup(&mut result);
        return result;
    }
    if cancelled.load(Ordering::Relaxed) {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Cancelled,
            message: "benchmark cancelled before sampling".to_owned(),
        });
        cleanup(&mut result);
        return result;
    }

    let Some(start_time) = session.identity.start_time else {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Setup,
            message: "macOS did not provide a stable process start identity".to_owned(),
        });
        cleanup(&mut result);
        return result;
    };
    let target = ManagedProcessTarget {
        root_pid: session.identity.pid,
        session_id: session.identity.session_id,
        root_start_time: start_time,
        executable_path: session.identity.executable_path.clone(),
        application_path: installation.application_path.clone(),
        isolated_profile_path: session.identity.profile_path.clone(),
        allow_profile_argument_attribution: installation.metadata.engine
            == Some(velocity_browser::BrowserEngine::Chromium),
    };
    let resource_config = ResourceConfig {
        sampling_interval_ms: config.sampling_interval_ms,
    };
    let mut collector = MacOSResourceCollector::new(target.clone());
    if is_photon {
        let (registry, error) =
            fetch_photon_process_registry(photon_automation.as_mut(), Duration::from_millis(250));
        collector.update_photon_process_registry(registry, error);
    }
    let mut monitor = match ResourceMonitor::start(target, resource_config, collector) {
        Ok(monitor) => monitor,
        Err(error) => {
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Sampling,
                message: error.to_string(),
            });
            cleanup(&mut result);
            return result;
        }
    };
    println!("      Measuring resources...");
    let start = Instant::now();
    let deadline = start + Duration::from_millis(config.measurement_duration_ms);
    let interval = Duration::from_millis(config.sampling_interval_ms);
    let mut next_registry_refresh = Instant::now() + Duration::from_millis(500);
    let mut collection_failure = None;
    while Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
        let next = (Instant::now() + interval).min(deadline);
        if !wait_cancellable(next.saturating_duration_since(Instant::now()), cancelled) {
            break;
        }
        if is_photon && Instant::now() >= next_registry_refresh {
            let (registry, error) = fetch_photon_process_registry(
                photon_automation.as_mut(),
                Duration::from_millis(250),
            );
            monitor
                .collector_mut()
                .update_photon_process_registry(registry, error);
            next_registry_refresh = Instant::now() + Duration::from_millis(500);
        }
        let sample = monitor.sample();
        if let Some(error) = &sample.collection_error {
            collection_failure = Some(error.to_string());
            break;
        }
    }
    let resource = monitor.stop();
    result.resource = Some(resource);
    if let Some(message) = collection_failure {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Sampling,
            message,
        });
    } else if cancelled.load(Ordering::Relaxed) {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Cancelled,
            message: "benchmark cancelled during resource sampling".to_owned(),
        });
    }
    cleanup(&mut result);
    result
}

#[cfg(target_os = "macos")]
fn fetch_photon_process_registry(
    client: Option<&mut velocity_browser::PhotonAutomationClient>,
    timeout: std::time::Duration,
) -> (
    Option<velocity_metrics::ProcessRegistrySnapshot>,
    Option<String>,
) {
    let Some(client) = client else {
        return (
            None,
            Some("authenticated Photon automation client is unavailable".to_owned()),
        );
    };
    if !client.supports("list_processes") {
        return (
            None,
            Some("Photon did not advertise the list_processes capability".to_owned()),
        );
    }
    match client.list_processes(timeout) {
        Ok(registry) => {
            let all_running = registry
                .processes
                .iter()
                .all(|process| process.state == "running");
            let processes = registry
                .processes
                .into_iter()
                .filter(|process| process.state == "running")
                .map(|process| velocity_metrics::RegisteredProcess {
                    pid: process.pid,
                    role: process.role,
                    start_time: process.start_time,
                    launch_generation: process.launch_generation,
                    parent_pid: process.parent_identity.as_ref().map(|parent| parent.pid),
                    parent_start_time: process.parent_identity.and_then(|parent| parent.start_time),
                })
                .collect();
            (
                Some(velocity_metrics::ProcessRegistrySnapshot {
                    scope: registry.scope,
                    source: registry.lifecycle_source,
                    complete: registry.complete && all_running,
                    processes,
                }),
                (!all_running).then(|| {
                    "Photon process registry returned a process that was not in the running state"
                        .to_owned()
                }),
            )
        }
        Err(error) => (None, Some(error.to_string())),
    }
}

#[cfg(target_os = "macos")]
fn open_tabs_sequentially<A, F>(
    automation: &mut A,
    urls: &[String],
    mut wait_for_ready: F,
) -> Result<(), velocity_browser::AutomationError>
where
    A: velocity_browser::BrowserAutomationAdapter,
    F: FnMut(usize, &str, &mut A) -> Result<(), velocity_browser::AutomationError>,
{
    let Some(initial_url) = urls.first() else {
        return Err(velocity_browser::AutomationError(
            "a tab workload requires at least one fixture URL".to_owned(),
        ));
    };
    let initial_count = automation
        .page_count()
        .map_err(velocity_browser::AutomationError)?;
    if initial_count != 1 {
        return Err(velocity_browser::AutomationError(format!(
            "fresh isolated Chromium session expected one initial page, observed {initial_count}"
        )));
    }
    let initial_target = automation
        .navigate_initial_page(initial_url)
        .map_err(velocity_browser::AutomationError)?;
    wait_for_ready(0, &initial_target, automation)?;

    for (index, url) in urls.iter().enumerate().skip(1) {
        let target_id = automation
            .create_tab(url)
            .map_err(velocity_browser::AutomationError)?;
        let expected_count = index + 1;
        let actual_count = automation
            .page_count()
            .map_err(velocity_browser::AutomationError)?;
        if actual_count != expected_count {
            return Err(velocity_browser::AutomationError(format!(
                "requested {expected_count} Chromium tabs so far but CDP verified {actual_count}"
            )));
        }
        wait_for_ready(index, &target_id, automation)?;
    }

    let actual_count = automation
        .page_count()
        .map_err(velocity_browser::AutomationError)?;
    if actual_count != urls.len() {
        return Err(velocity_browser::AutomationError(format!(
            "requested {} Chromium tabs but CDP verified {actual_count}",
            urls.len()
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn wait_for_fixture(
    server: &crate::fixture_server::FixtureServer,
    token: &str,
    tabs: &[usize],
    config: &ResourceBenchmarkConfig,
    cancelled: &'static std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    server.wait_for_navigation_with_cancel(
        token,
        tabs,
        Duration::from_millis(config.navigation_timeout_ms),
        || cancelled.load(Ordering::Relaxed),
    )?;
    server.wait_for_ready_with_cancel(
        token,
        tabs,
        Duration::from_millis(config.readiness_timeout_ms),
        || cancelled.load(Ordering::Relaxed),
    )
}

#[cfg(target_os = "macos")]
struct IterationCleanup<'a> {
    manager: &'a velocity_browser::BrowserLifecycleManager<
        velocity_browser::macos::MacOSProcessDriver,
        velocity_browser::FileSessionStore,
    >,
    driver: &'a velocity_browser::macos::MacOSProcessDriver,
    session_id: &'a str,
    identity: &'a velocity_browser::ProcessIdentity,
    profile_path: &'a Path,
    automation_directory: Option<&'a Path>,
    automation_token: Option<&'a str>,
}

#[cfg(target_os = "macos")]
fn cleanup_iteration(cleanup: IterationCleanup<'_>, result: &mut ResourceBenchmarkIteration) {
    let IterationCleanup {
        manager,
        driver,
        session_id,
        identity,
        profile_path,
        automation_directory,
        automation_token,
    } = cleanup;
    use velocity_browser::CloseOutcome;
    match manager.close(session_id) {
        Ok(CloseOutcome::Closed(_)) | Ok(CloseOutcome::AlreadyExited(_)) => {
            result.cleanup.browser_closed = true;
            if let Err(error) = ensure_closed_profile_removed(driver, session_id, identity) {
                record_cleanup_failure(result, error);
            }
        }
        Err(error) => {
            result.cleanup.browser_closed = false;
            result.cleanup.detail = Some(format!("managed browser cleanup failed: {error}"));
            if result.failure.is_none() {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Cleanup,
                    message: error.to_string(),
                });
            }
        }
    }
    if result.cleanup.browser_closed {
        if let Err(error) = cleanup_automation_directory(automation_directory, automation_token) {
            record_cleanup_failure(result, error);
        }
    } else if let Some(directory) = automation_directory
        && directory.exists()
    {
        record_cleanup_failure(
            result,
            format!(
                "kept Photon automation directory {} because managed process shutdown could not be verified",
                directory.display()
            ),
        );
    }
    result.cleanup.profile_removed = !profile_path.exists();
    if !result.cleanup.profile_removed {
        let detail = format!("isolated profile remains at {}", profile_path.display());
        result.cleanup.detail = Some(match result.cleanup.detail.take() {
            Some(existing) => format!("{existing}; {detail}"),
            None => detail.clone(),
        });
        if result.failure.is_none() {
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Cleanup,
                message: detail,
            });
        }
    }
}

#[cfg(target_os = "macos")]
fn ensure_closed_profile_removed(
    driver: &velocity_browser::macos::MacOSProcessDriver,
    session_id: &str,
    identity: &velocity_browser::ProcessIdentity,
) -> Result<(), String> {
    use velocity_browser::{ProcessDriver, ProcessState};

    let Some(profile_path) = identity.profile_path.as_deref() else {
        return Ok(());
    };
    let mut last_error = None;
    for attempt in 0..3 {
        if !profile_path.exists() {
            return Ok(());
        }
        match driver.state(identity).map_err(|error| error.to_string())? {
            ProcessState::Exited => {}
            ProcessState::Running => {
                return Err(format!(
                    "refusing to remove isolated profile {} while its managed process session is still running",
                    profile_path.display()
                ));
            }
            ProcessState::OwnershipUncertain => {
                return Err(format!(
                    "refusing to remove isolated profile {} because managed process ownership is uncertain",
                    profile_path.display()
                ));
            }
        }
        match driver.cleanup(session_id, identity) {
            Ok(()) if !profile_path.exists() => return Ok(()),
            Ok(()) => {
                last_error = Some(format!(
                    "isolated profile reappeared after cleanup: {}",
                    profile_path.display()
                ));
            }
            Err(error) => last_error = Some(error.to_string()),
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Err(last_error
        .unwrap_or_else(|| format!("isolated profile remains at {}", profile_path.display())))
}

#[cfg(target_os = "macos")]
fn cleanup_automation_directory(
    directory: Option<&Path>,
    expected_token: Option<&str>,
) -> Result<(), String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    let Some(directory) = directory else {
        return Ok(());
    };
    let metadata = match std::fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "cannot inspect Photon automation directory: {error}"
            ));
        }
    };
    let expected_name = expected_token
        .filter(|token| token.len() >= 12)
        .map(|token| format!("vlt-{}", &token[..12]))
        .ok_or_else(|| "cannot verify the Photon automation cleanup token".to_owned())?;
    if directory.parent() != Some(Path::new("/tmp"))
        || directory.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str())
        || !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "refusing to remove unverified Photon automation directory {}",
            directory.display()
        ));
    }
    let credential = directory.join("token");
    let credential_metadata = std::fs::symlink_metadata(&credential)
        .map_err(|error| format!("cannot verify Photon credential before cleanup: {error}"))?;
    if !credential_metadata.file_type().is_file()
        || credential_metadata.uid() != unsafe { libc::geteuid() }
        || credential_metadata.mode() & 0o077 != 0
        || credential_metadata.len() > 128
    {
        return Err(format!(
            "refusing to remove Photon automation directory with an unverified credential: {}",
            directory.display()
        ));
    }
    let token = std::fs::read_to_string(&credential)
        .map_err(|error| format!("cannot read Photon credential before cleanup: {error}"))?;
    if token.trim_end_matches(['\r', '\n']) != expected_token.unwrap_or_default() {
        return Err(format!(
            "refusing to remove Photon automation directory whose credential changed: {}",
            directory.display()
        ));
    }
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("cannot inspect Photon automation directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("cannot inspect automation resource: {error}"))?;
        match entry.file_name().to_str() {
            Some("token") => {}
            Some("p.sock") => {
                let socket_metadata = std::fs::symlink_metadata(entry.path()).map_err(|error| {
                    format!("cannot inspect Photon socket before cleanup: {error}")
                })?;
                if !socket_metadata.file_type().is_socket()
                    || socket_metadata.uid() != unsafe { libc::geteuid() }
                    || socket_metadata.mode() & 0o077 != 0
                {
                    return Err(format!(
                        "refusing to remove Photon automation directory with an unverified socket: {}",
                        directory.display()
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "refusing to remove Photon automation directory containing an unexpected file: {}",
                    directory.display()
                ));
            }
        }
    }
    std::fs::remove_dir_all(directory).map_err(|error| {
        format!(
            "cannot remove Photon automation directory {}: {error}",
            directory.display()
        )
    })
}

#[cfg(target_os = "macos")]
fn record_cleanup_failure(result: &mut ResourceBenchmarkIteration, message: String) {
    result.cleanup.detail = Some(match result.cleanup.detail.take() {
        Some(existing) => format!("{existing}; {message}"),
        None => message.clone(),
    });
    if result.failure.is_none() {
        result.failure = Some(ResourceBenchmarkFailure {
            kind: ResourceFailureKind::Cleanup,
            message,
        });
    }
}

#[cfg(target_os = "macos")]
fn wait_cancellable(duration: Duration, cancelled: &std::sync::atomic::AtomicBool) -> bool {
    use std::{thread, time::Instant};
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        thread::sleep((deadline - Instant::now()).min(Duration::from_millis(25)));
    }
    !cancelled.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(target_os = "macos")]
fn secure_random_token() -> Result<String, String> {
    use std::{fmt::Write as _, fs::File, io::Read};

    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| format!("cannot generate Photon automation credential: {error}"))?;
    let mut token = String::with_capacity(64);
    for byte in bytes {
        write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(token)
}

#[cfg(target_os = "macos")]
fn install_cancellation_handler() -> Result<(), String> {
    unsafe extern "C" fn cancel_on_signal(signal: libc::c_int) {
        if signal == libc::SIGINT || signal == libc::SIGTERM {
            RESOURCE_CANCELLED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_page_benchmark_and_defaults() {
        let options = parse_options(
            ResourceWorkload::Page,
            args(&["--browser", "chrome", "--fixture", "dynamic"]).into_iter(),
        )
        .unwrap();
        assert!(matches!(options.target, TargetSelection::One(ref browser) if browser == "chrome"));
        assert_eq!(options.config.runs, 5);
        assert_eq!(options.config.navigation_timeout_ms, 30_000);
        assert_eq!(options.config.readiness_timeout_ms, 30_000);
        assert_eq!(options.config.fixture, Some(FixtureKind::Dynamic));
        assert!(options.record_history);
        let without_history = parse_options(
            ResourceWorkload::Page,
            args(&["--browser", "chrome", "--no-record-history"]).into_iter(),
        )
        .unwrap();
        assert!(!without_history.record_history);
        assert!(
            parse_options(
                ResourceWorkload::Page,
                args(&[
                    "--browser",
                    "chrome",
                    "--record-history",
                    "--no-record-history"
                ])
                .into_iter(),
            )
            .is_err()
        );
    }

    #[test]
    fn navigation_and_readiness_deadlines_are_independently_configurable() {
        let options = parse_options(
            ResourceWorkload::Page,
            args(&[
                "--browser",
                "chrome",
                "--navigation-timeout",
                "12",
                "--readiness-timeout",
                "20",
            ])
            .into_iter(),
        )
        .unwrap();
        assert_eq!(options.config.navigation_timeout_ms, 12_000);
        assert_eq!(options.config.readiness_timeout_ms, 20_000);
        assert!(
            parse_options(
                ResourceWorkload::Page,
                args(&[
                    "--browser",
                    "chrome",
                    "--timeout",
                    "12",
                    "--readiness-timeout",
                    "20"
                ])
                .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_incompatible_or_malformed_options_before_launch() {
        assert!(
            parse_options(
                ResourceWorkload::Idle,
                args(&["--browser", "chrome", "--fixture", "simple"]).into_iter()
            )
            .is_err()
        );
        assert!(
            parse_options(
                ResourceWorkload::Tabs,
                args(&["--browser", "chrome", "--tabs", "1,1"]).into_iter()
            )
            .is_err()
        );
        assert!(
            parse_options(
                ResourceWorkload::Page,
                args(&["--browser", "chrome", "--all"]).into_iter()
            )
            .is_err()
        );
        assert!(parse_tab_counts("1,broken").is_err());
    }

    #[test]
    fn accepts_valid_tab_counts_and_rejects_unsafe_limits() {
        assert_eq!(parse_tab_counts("1, 5,10").unwrap(), [1, 5, 10]);
        let mut config = ResourceBenchmarkConfig::defaults(ResourceWorkload::Tabs);
        config.tab_counts = vec![velocity_core::MAX_TAB_COUNT + 1];
        assert!(config.validate(ResourceWorkload::Tabs).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn chromium_tabs_are_verified_ready_before_the_next_tab_is_created() {
        use std::sync::{Arc, Mutex};

        use velocity_browser::{AutomationProtocol, BrowserAutomationAdapter};

        struct MockAutomation {
            events: Arc<Mutex<Vec<String>>>,
            page_count: usize,
        }

        impl BrowserAutomationAdapter for MockAutomation {
            fn protocol(&self) -> AutomationProtocol {
                AutomationProtocol::ChromeDevToolsProtocol
            }

            fn navigate_initial_page(&mut self, url: &str) -> Result<String, String> {
                self.page_count = 1;
                self.events.lock().unwrap().push(format!("navigate:{url}"));
                Ok(url.to_owned())
            }

            fn create_tab(&mut self, url: &str) -> Result<String, String> {
                self.page_count += 1;
                self.events.lock().unwrap().push(format!("create:{url}"));
                Ok(url.to_owned())
            }

            fn page_count(&mut self) -> Result<usize, String> {
                self.events
                    .lock()
                    .unwrap()
                    .push(format!("count:{}", self.page_count));
                Ok(self.page_count)
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut automation = MockAutomation {
            events: Arc::clone(&events),
            page_count: 1,
        };
        let urls = ["fixture-0", "fixture-1", "fixture-2"].map(str::to_owned);
        let ready_events = Arc::clone(&events);
        open_tabs_sequentially(&mut automation, &urls, move |tab, target, _automation| {
            ready_events
                .lock()
                .unwrap()
                .push(format!("ready:{tab}:{target}"));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            *events.lock().unwrap(),
            [
                "count:1",
                "navigate:fixture-0",
                "ready:0:fixture-0",
                "create:fixture-1",
                "count:2",
                "ready:1:fixture-1",
                "create:fixture-2",
                "count:3",
                "ready:2:fixture-2",
                "count:3",
            ]
        );
    }

    #[test]
    fn total_retained_samples_are_bounded_across_iterations_and_scenarios() {
        let mut config = ResourceBenchmarkConfig::defaults(ResourceWorkload::Page);
        config.measurement_duration_ms = 10_000;
        config.sampling_interval_ms = 10;
        config.runs = 100;
        assert!(config.validate(ResourceWorkload::Page).is_ok());
        assert!(validate_total_sample_bound(&config, ResourceWorkload::Page, 2).is_err());
        config.runs = 5;
        assert_eq!(
            validate_total_sample_bound(&config, ResourceWorkload::Page, 2).unwrap(),
            10_000
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn settling_wait_stops_promptly_when_cancelled() {
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        assert!(!wait_cancellable(Duration::from_secs(30), &cancelled));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn randomized_scenarios_preserve_browser_rounds_and_are_replayable() {
        use std::path::PathBuf;

        use velocity_browser::{
            BrowserEngine, BrowserId, BrowserInstallation, BrowserMetadata, InstallationId,
        };

        let installations = ["alpha", "beta", "gamma"]
            .into_iter()
            .map(|id| BrowserInstallation {
                installation_id: InstallationId::new(id).unwrap(),
                metadata: BrowserMetadata {
                    id: BrowserId::new(id).unwrap(),
                    name: id.to_owned(),
                    bundle_identifier: None,
                    engine: Some(BrowserEngine::Chromium),
                },
                version: Some("1".to_owned()),
                application_path: PathBuf::from("/fixture/browser.app"),
                executable_path: PathBuf::from("/fixture/browser"),
            })
            .collect::<Vec<_>>();
        let mut seed_a = 0x1234_5678_u64;
        let mut seed_b = seed_a;
        let first = make_rounds(&installations, &[Some(1), Some(5)], 3, true, &mut seed_a);
        let replay = make_rounds(&installations, &[Some(1), Some(5)], 3, true, &mut seed_b);
        assert_eq!(first, replay);
        for round in first {
            assert_eq!(round.len(), 6);
            for (group, tabs) in [(&round[..3], Some(1)), (&round[3..], Some(5))] {
                assert!(group.iter().all(|(_, count)| *count == tabs));
                let ids = group
                    .iter()
                    .map(|(installation, _)| installation.installation_id.to_string())
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    ids,
                    BTreeSet::from(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()])
                );
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn html_report_is_offline_and_contains_real_sample_series() {
        use std::path::PathBuf;

        use crate::resource_report::render_html;
        use velocity_core::{
            CleanupStatus, ResourceBenchmarkIteration, ResourceBenchmarkReport, RunOrderMetadata,
        };
        use velocity_metrics::{
            ResourceCoverage, ResourceReport, ResourceSample, ResourceStatistics,
        };

        let browser = ReportBrowser {
            id: "chrome".to_owned(),
            installation_id: "chrome-test".to_owned(),
            name: "Chrome <test>".to_owned(),
            version: Some("1".to_owned()),
            executable_path: PathBuf::from("/fixture/chrome"),
        };
        let iteration = ResourceBenchmarkIteration {
            browser,
            iteration: 1,
            warmup: false,
            tab_count: Some(1),
            automation_protocol: Some("fixture CDP".to_owned()),
            profile_isolation: "fresh profile".to_owned(),
            started_at_unix_ms: 1,
            resource: Some(ResourceReport {
                schema_version: 1,
                started_at_unix_ms: 1,
                sampling_interval_ms: 100,
                samples: vec![ResourceSample {
                    elapsed_ms: 500.0,
                    interval_ms: Some(100.0),
                    wall_clock_unix_ms: Some(501),
                    cpu_percent: Some(3.25),
                    physical_footprint_bytes: Some(10 * 1024 * 1024),
                    resident_bytes: Some(12 * 1024 * 1024),
                    process_count: 2,
                    coverage: ResourceCoverage::SessionScoped,
                    attribution: velocity_metrics::ProcessAttributionDiagnostics::default(),
                    processes: Vec::new(),
                    warnings: Vec::new(),
                    collection_error: None,
                }],
                statistics: ResourceStatistics {
                    cpu_mean_percent: Some(3.25),
                    physical_footprint_mean_bytes: Some((10 * 1024 * 1024) as f64),
                    ..ResourceStatistics::default()
                },
            }),
            failure: None,
            cleanup: CleanupStatus {
                browser_closed: true,
                profile_removed: true,
                detail: None,
            },
        };
        let mut config = ResourceBenchmarkConfig::defaults(ResourceWorkload::Page);
        config.runs = 1;
        config.measurement_duration_ms = 1_000;
        let report = ResourceBenchmarkReport::new(
            ResourceWorkload::Page,
            config,
            RunOrderMetadata::default(),
            vec![iteration],
            false,
        );
        let html = render_html(&report);
        assert!(html.contains("<svg"));
        assert!(html.contains("3.2%"));
        assert!(html.contains("Chrome &lt;test&gt;"));
        assert!(html.contains("engine-registry-and-private-session-time-weighted-cpu-v3"));
        assert!(html.contains("UNKNOWN (verified 0)"));
        assert!(html.contains("Process attribution diagnostics"));
        assert!(!html.contains("<script src="));
        assert!(!html.contains("https://"));

        let mut tab_report = report.clone();
        tab_report.benchmark = ResourceWorkload::Tabs.id().to_owned();
        let mut second_tab_summary = tab_report.summaries[0].clone();
        second_tab_summary.tab_count = Some(2);
        second_tab_summary
            .across_iteration_statistics
            .get_mut("physical_footprint_mean_bytes")
            .unwrap()
            .mean = (20 * 1024 * 1024) as f64;
        tab_report.summaries.push(second_tab_summary);
        let tab_html = render_html(&tab_report);
        assert!(tab_html.contains("Memory by tab count"));
        assert!(tab_html.contains("aria-label=\"Mean physical footprint by browser tab count\""));
        assert!(tab_html.contains("Chrome &lt;test&gt; — 2 tab(s): 20.0 MiB"));
    }
}
