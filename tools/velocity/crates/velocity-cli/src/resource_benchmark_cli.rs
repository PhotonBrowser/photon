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

pub fn run_responsiveness(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut iterations = 10_usize;
    let mut warmup = 1_usize;
    let mut timeout_seconds = 60_u64;
    let mut output = None;
    let mut record_history = true;
    let mut diagnostics_enabled = true;
    let mut balanced_overhead = false;
    let mut seed = None;
    let mut browser = "photon".to_owned();
    let mut workload = "task-backlog".to_owned();
    let mut seen = BTreeSet::new();
    let mut arguments = arguments.into_iter();
    while let Some(option) = arguments.next() {
        if !seen.insert(option.clone()) {
            return Err(format!("{option} may be specified only once"));
        }
        match option.as_str() {
            "--browser" => browser = required_value(&mut arguments, &option)?,
            "--workload" => workload = required_value(&mut arguments, &option)?,
            "--iterations" => iterations = parse_value(&mut arguments, &option)?,
            "--warmup" => warmup = parse_value(&mut arguments, &option)?,
            "--timeout" => timeout_seconds = parse_value(&mut arguments, &option)?,
            "--output" => output = Some(PathBuf::from(required_value(&mut arguments, &option)?)),
            "--diagnostics" => match required_value(&mut arguments, &option)?.as_str() {
                "enabled" => diagnostics_enabled = true,
                "disabled" => diagnostics_enabled = false,
                "balanced" => balanced_overhead = true,
                value => {
                    return Err(format!(
                        "{option} must be enabled, disabled, or balanced, got '{value}'"
                    ));
                }
            },
            "--seed" => seed = Some(parse_value(&mut arguments, &option)?),
            "--no-record-history" => record_history = false,
            "--record-history" => record_history = true,
            "--help" | "-h" => {
                print_responsiveness_help();
                return Ok(());
            }
            other => return Err(format!("unknown responsiveness option '{other}'")),
        }
    }
    let fixture = parse_responsiveness_fixture(&workload)?;
    if seen.contains("--record-history") && seen.contains("--no-record-history") {
        return Err("choose only one of --record-history and --no-record-history".to_owned());
    }
    if !(1..=100).contains(&iterations) {
        return Err("--iterations must be between 1 and 100".to_owned());
    }
    if warmup > 20 {
        return Err("--warmup must be between 0 and 20".to_owned());
    }
    if !(1..=600).contains(&timeout_seconds) {
        return Err("--timeout must be between 1 and 600 seconds".to_owned());
    }
    if seed.is_some() && !balanced_overhead {
        return Err("--seed is available only with --diagnostics balanced".to_owned());
    }

    let mut config = ResourceBenchmarkConfig::defaults(ResourceWorkload::Page);
    config.runs = iterations;
    config.warmup_iterations = warmup;
    config.fixture = Some(fixture);
    config.settle_period_ms = 0;
    config.navigation_timeout_ms = timeout_seconds * 1_000;
    config.readiness_timeout_ms = timeout_seconds * 1_000;
    config.validate(ResourceWorkload::Page)?;
    if output
        .as_ref()
        .is_some_and(|path| path.extension().is_none_or(|ext| ext != "json"))
    {
        return Err("--output must name a .json report file".to_owned());
    }
    let output_directory = output
        .as_ref()
        .and_then(|path| path.parent())
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(default_output_directory);
    run_resource_benchmark_mode(
        Options {
            workload: ResourceWorkload::Page,
            target: TargetSelection::One(browser),
            config,
            output_directory,
            record_history,
        },
        true,
        output,
        diagnostics_enabled,
        balanced_overhead.then_some(seed.unwrap_or(0x5048_4f54_4f4e)),
    )
}

fn parse_responsiveness_fixture(workload: &str) -> Result<FixtureKind, String> {
    match workload {
        "task-backlog" => Ok(FixtureKind::TaskBacklog),
        "gc-pressure" => Ok(FixtureKind::GcPressure),
        "dom-style" => Ok(FixtureKind::DomStyle),
        "style-localized" => Ok(FixtureKind::StyleLocalized),
        "style-descendant" => Ok(FixtureKind::StyleDescendant),
        "style-sibling" => Ok(FixtureKind::StyleSibling),
        "style-relational" => Ok(FixtureKind::StyleRelational),
        "layout-updates" => Ok(FixtureKind::LayoutUpdates),
        "animation" => Ok(FixtureKind::Animation),
        _ => Err(
            "supported responsiveness workloads: task-backlog, dom-style, style-localized, style-descendant, style-sibling, style-relational, layout-updates, gc-pressure, animation"
                .to_owned(),
        ),
    }
}

fn print_responsiveness_help() {
    crate::terminal::print_markdown(
        "## `velocity bench responsiveness`\n\n`velocity bench responsiveness --browser <photon-id|number> [--workload task-backlog|dom-style|style-localized|style-descendant|style-sibling|style-relational|layout-updates|gc-pressure|animation] [OPTIONS]`\n\n`task-backlog` queues 512 zero-delay timer callbacks. `dom-style` mutates classes and attributes in a fixed nested tree with descendant, sibling, state, and relational selectors. The focused `style-localized`, `style-descendant`, `style-sibling`, and `style-relational` workloads isolate one class of selector dependency in the same fixed 16-by-32 tree and run 1,024 mutations each. `layout-updates` repeatedly changes normal-flow and absolutely positioned geometry while reading element bounds. `gc-pressure` allocates a fixed graph of temporary and retained JavaScript objects plus temporary DOM nodes, then releases references and allows normal sweeping; it never forces collection. `animation` runs five fixed 900 ms phases: CSS transform, CSS opacity, JavaScript requestAnimationFrame, DOM mutations during animation, and visible transform animation under temporary allocation pressure. Its requestAnimationFrame intervals are browser-observable callback intervals, separate from Metal drawable presentation timestamps. All use a fresh Photon profile and authenticated Ladybird diagnostics. Select one Photon installation by the number or installation ID shown by `velocity list`; the family selector `photon` works when it resolves to a single installation. Options: `--iterations N` (10), `--warmup N` (1), `--timeout SECONDS` (60), `--diagnostics enabled|disabled|balanced` (enabled), `--seed N` (balanced mode only; default 88271498792782), `--output FILE.json`, and `--no-record-history`. Balanced mode runs diagnostics-off and diagnostics-on sessions in randomized pairs with fresh profiles and retains pair IDs; it reports paired overhead effects and uncertainty rather than treating sessions as independent browser comparisons. Only Photon is supported by this instrumentation version.",
    );
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
    run_resource_benchmark_mode(options, false, None, false, None)
}

#[cfg(not(target_os = "macos"))]
fn run_resource_benchmark_mode(
    _options: Options,
    _responsiveness: bool,
    _responsiveness_output: Option<PathBuf>,
    _diagnostics_enabled: bool,
    _balanced_seed: Option<u64>,
) -> Result<(), String> {
    Err("automated resource benchmarks currently require macOS".to_owned())
}

#[cfg(target_os = "macos")]
fn run_resource_benchmark_mode(
    options: Options,
    responsiveness: bool,
    responsiveness_output: Option<PathBuf>,
    diagnostics_enabled: bool,
    balanced_seed: Option<u64>,
) -> Result<(), String> {
    use std::{
        fs,
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    use velocity_browser::macos::default_session_store_path;

    RESOURCE_CANCELLED.store(false, Ordering::SeqCst);
    install_cancellation_handler()?;

    let registry = super::discover_browsers()?;
    let installations = resolve_installations(&registry, &options.target)?;
    if installations.is_empty() {
        return Err("no supported browser installations were found".to_owned());
    }
    if responsiveness {
        validate_responsiveness_installations(&installations)?;
    }
    validate_total_sample_bound(
        &options.config,
        options.workload,
        installations.len() * if balanced_seed.is_some() { 2 } else { 1 },
    )?;
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
        "Velocity — {} Benchmark\n",
        if responsiveness {
            "Responsiveness"
        } else {
            title(options.workload)
        }
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
        let version = match fixture {
            FixtureKind::TaskBacklog => velocity_core::TASK_BACKLOG_FIXTURE_REVISION,
            FixtureKind::GcPressure => velocity_core::GC_PRESSURE_FIXTURE_REVISION,
            FixtureKind::DomStyle => velocity_core::DOM_STYLE_FIXTURE_REVISION,
            FixtureKind::StyleLocalized
            | FixtureKind::StyleDescendant
            | FixtureKind::StyleSibling
            | FixtureKind::StyleRelational => velocity_core::STYLE_INVALIDATION_FIXTURE_REVISION,
            FixtureKind::LayoutUpdates => velocity_core::LAYOUT_UPDATES_FIXTURE_REVISION,
            FixtureKind::Animation => velocity_core::ANIMATION_FIXTURE_REVISION,
            _ => velocity_core::FIXTURE_VERSION,
        };
        println!("Fixture: {} v{}", fixture.name(), version);
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
    if responsiveness {
        println!(
            "Fixture deadline: {} seconds; process-resource sample interval: {} ms\n",
            options.config.readiness_timeout_ms / 1_000,
            options.config.sampling_interval_ms
        );
    } else {
        println!(
            "Duration: {} seconds; settling: {} seconds; sample interval: {} ms\n",
            options.config.measurement_duration_ms / 1_000,
            options.config.settle_period_ms / 1_000,
            options.config.sampling_interval_ms
        );
    }

    let mut iterations = Vec::new();
    let mut per_scenario_count = BTreeMap::<(String, Option<usize>, bool), usize>::new();
    let rounds = warmup_rounds
        .into_iter()
        .map(|round| (round, true))
        .chain(measured_rounds.into_iter().map(|round| (round, false)));
    let mut measured_progress = 0;
    let diagnostic_mode_count = if balanced_seed.is_some() { 2 } else { 1 };
    let measured_total =
        options.config.runs * installations.len() * tab_scenarios.len() * diagnostic_mode_count;
    let mut balanced_order_state = balanced_seed.unwrap_or_default();
    let mut actual_warmup_order = Vec::new();
    let mut actual_measured_order = Vec::new();
    for (round_index, (round, warmup)) in rounds.enumerate() {
        let mut completed_order = Vec::new();
        for (installation, tabs) in round {
            if warmup {
                println!(
                    "[warm-up] {}{}",
                    installation.metadata.name,
                    tabs.map(|tabs| format!(" — {tabs} tab(s)"))
                        .unwrap_or_default()
                );
            }
            let mut diagnostic_modes = if balanced_seed.is_some() {
                diagnostics_pair_order(&mut balanced_order_state).to_vec()
            } else {
                vec![diagnostics_enabled]
            };
            for diagnostic_enabled_for_run in diagnostic_modes.drain(..) {
                if !warmup {
                    measured_progress += 1;
                    println!(
                        "[{}/{}] {}{} (diagnostics {})",
                        measured_progress,
                        measured_total,
                        installation.metadata.name,
                        tabs.map(|tabs| format!(" — {tabs} tab(s)"))
                            .unwrap_or_default(),
                        if diagnostic_enabled_for_run {
                            "on"
                        } else {
                            "off"
                        },
                    );
                }
                let key = (installation.installation_id.to_string(), tabs, warmup);
                let iteration_number = per_scenario_count.entry(key).or_default();
                *iteration_number += 1;
                let execution_environment = IterationEnvironment {
                    config: &options.config,
                    fixture_server: fixture_server.as_ref(),
                    profile_root: &profile_root,
                    session_store_path: &session_store_path,
                    cancelled: &RESOURCE_CANCELLED,
                    responsiveness,
                    diagnostics_enabled: responsiveness && diagnostic_enabled_for_run,
                };
                let diagnostic_pair_id = (balanced_seed.is_some() && !warmup)
                    .then_some(round_index.saturating_sub(options.config.warmup_iterations) + 1);
                let iteration = execute_iteration(
                    &installation,
                    options.workload,
                    tabs,
                    *iteration_number,
                    warmup,
                    diagnostic_pair_id,
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
                completed_order.push(format!(
                    "{}:diagnostics-{}",
                    scenario_id(&installation, tabs),
                    if diagnostic_enabled_for_run {
                        "on"
                    } else {
                        "off"
                    }
                ));
                iterations.push(iteration);
                if RESOURCE_CANCELLED.load(Ordering::Relaxed) {
                    break;
                }
            }
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
        randomized: options.config.randomized || balanced_seed.is_some(),
        randomization_seed: seed.or(balanced_seed),
        warmup_order_by_round: actual_warmup_order,
        measured_order_by_round: actual_measured_order,
    };
    if responsiveness {
        let responsiveness_fixture = options.config.fixture.unwrap_or(FixtureKind::TaskBacklog);
        let report = build_responsiveness_report(
            &installations[0],
            iterations,
            options.config.runs * diagnostic_mode_count,
            options.config.warmup_iterations * diagnostic_mode_count,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            run_id.clone(),
            cancelled,
            diagnostics_enabled,
            balanced_seed,
            responsiveness_fixture,
        );
        print_responsiveness_summary(&report);
        let json_path = responsiveness_output.unwrap_or_else(|| {
            options
                .output_directory
                .join(format!("responsiveness-{run_id}.json"))
        });
        let html_path = json_path.with_extension("html");
        write_responsiveness_reports(&json_path, &html_path, &report)?;
        println!("\nSaved JSON: {}", display_path(&json_path));
        println!("Saved HTML: {}", display_path(&html_path));
        if options.record_history && report.coverage.successful_iterations > 0 {
            super::history_cli::record_after_success(&json_path);
        }
        drop(fixture_server);
        if report.coverage.successful_iterations == 0 {
            return Err(format!(
                "no responsiveness iterations succeeded; report saved to {}",
                json_path.display()
            ));
        }
        if cancelled {
            println!("Benchmark cancelled after cleaning up the active managed session.");
        }
        return Ok(());
    }

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

fn validate_responsiveness_installations(
    installations: &[velocity_browser::BrowserInstallation],
) -> Result<(), String> {
    if installations.len() != 1
        || installations
            .iter()
            .any(|installation| installation.metadata.id.as_str() != "photon")
    {
        return Err(
            "the task-backlog workload currently supports one Photon installation".to_owned(),
        );
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

fn diagnostics_pair_order(seed: &mut u64) -> [bool; 2] {
    if *seed == 0 {
        *seed = 0x9e37_79b9_7f4a_7c15;
    }
    let mut modes = [false, true];
    shuffle(&mut modes, seed);
    modes
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
    responsiveness: bool,
    diagnostics_enabled: bool,
}

#[cfg(target_os = "macos")]
fn execute_iteration(
    installation: &velocity_browser::BrowserInstallation,
    workload: ResourceWorkload,
    tab_count: Option<usize>,
    iteration: usize,
    warmup: bool,
    diagnostic_pair_id: Option<usize>,
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
        diagnostic_pair_id,
        diagnostics_enabled: environment
            .responsiveness
            .then_some(environment.diagnostics_enabled),
        tab_count,
        automation_protocol: None,
        profile_isolation: "isolated Velocity profile required; launch not started".to_owned(),
        started_at_unix_ms: timestamp,
        resource: None,
        responsiveness: None,
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
    if environment.responsiveness {
        let responsiveness_fixture = config.fixture.unwrap_or(FixtureKind::TaskBacklog);
        let Some(client) = photon_automation.as_mut() else {
            result.failure = Some(ResourceBenchmarkFailure {
                kind: ResourceFailureKind::Unsupported,
                message: "responsiveness workloads require authenticated Photon automation"
                    .to_owned(),
            });
            cleanup(&mut result);
            return result;
        };
        let tab_id = match client.list_tabs(Duration::from_secs(5)) {
            Ok(tabs) if tabs.len() == 1 => tabs[0].tab_id,
            Ok(tabs) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Automation,
                    message: format!(
                        "{} fixture expected one tab, found {}",
                        responsiveness_fixture.name(),
                        tabs.len()
                    ),
                });
                cleanup(&mut result);
                return result;
            }
            Err(error) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Automation,
                    message: error.to_string(),
                });
                cleanup(&mut result);
                return result;
            }
        };
        let timeout = Duration::from_millis(config.readiness_timeout_ms);
        if warmup {
            if let Err(message) = wait_for_responsiveness_completion(
                client,
                tab_id,
                timeout,
                cancelled,
                responsiveness_fixture,
                || {},
            ) {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::ReadinessTimeout,
                    message,
                });
            }
            cleanup(&mut result);
            return result;
        }

        if environment.diagnostics_enabled {
            if let Err(error) = client.performance_start(tab_id, Duration::from_secs(5)) {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Unsupported,
                    message: error.to_string(),
                });
                cleanup(&mut result);
                return result;
            }
        }

        let mut process_resource_error = None;
        let mut process_resource_monitor = None;
        if let Some(start_time) = session.identity.start_time {
            let target = ManagedProcessTarget {
                root_pid: session.identity.pid,
                session_id: session.identity.session_id,
                root_start_time: start_time,
                executable_path: session.identity.executable_path.clone(),
                application_path: installation.application_path.clone(),
                isolated_profile_path: session.identity.profile_path.clone(),
                allow_profile_argument_attribution: false,
            };
            let mut collector = MacOSResourceCollector::new(target.clone());
            let (registry, registry_error) =
                fetch_photon_process_registry(Some(&mut *client), Duration::from_millis(250));
            collector.update_photon_process_registry(registry, registry_error);
            match ResourceMonitor::start(
                target,
                ResourceConfig {
                    sampling_interval_ms: config.sampling_interval_ms,
                },
                collector,
            ) {
                Ok(mut monitor) => {
                    let _ = monitor.sample();
                    process_resource_monitor = Some(monitor);
                }
                Err(error) => process_resource_error = Some(error.to_string()),
            }
        } else {
            process_resource_error =
                Some("macOS did not provide a stable process start identity".to_owned());
        }

        let mut next_resource_sample =
            Instant::now() + Duration::from_millis(config.sampling_interval_ms);
        let completion = wait_for_responsiveness_completion(
            client,
            tab_id,
            timeout,
            cancelled,
            responsiveness_fixture,
            || {
                if Instant::now() >= next_resource_sample {
                    if let Some(monitor) = process_resource_monitor.as_mut() {
                        let sample = monitor.sample();
                        if let Some(error) = &sample.collection_error {
                            process_resource_error = Some(error.to_string());
                        }
                    }
                    next_resource_sample =
                        Instant::now() + Duration::from_millis(config.sampling_interval_ms);
                }
            },
        );
        if responsiveness_fixture == FixtureKind::Animation && completion.is_ok() {
            // Allow in-flight GPU work and Metal's presented handler to finish
            // before freezing the per-frame diagnostics session.
            std::thread::sleep(Duration::from_millis(150));
        }
        let process_resources = process_resource_monitor.map(|monitor| monitor.stop());
        let diagnostics = if environment.diagnostics_enabled {
            client
                .performance_stop(tab_id, Duration::from_secs(5))
                .map(|mut diagnostics| {
                    if client.supports("performance_frame_timing_v1") {
                        diagnostics["photon_drawable_presentation_events_supported"] =
                            serde_json::Value::Bool(
                                client.supports("performance_drawable_presentation_events_v1"),
                            );
                        diagnostics["photon_gpu_sampling_correlation_supported"] =
                            serde_json::Value::Bool(
                                client.supports("performance_gpu_sampling_correlation_v1"),
                            );
                        match client.performance_frame_snapshot(tab_id, Duration::from_secs(5)) {
                            Ok(snapshot) => {
                                diagnostics["photon_frame_timing"] = snapshot["metrics"].clone();
                            }
                            Err(error) => {
                                diagnostics["photon_frame_timing_error"] =
                                    serde_json::Value::String(error.to_string());
                            }
                        }
                    } else {
                        diagnostics["photon_frame_timing_error"] = serde_json::Value::String(
                            "Photon did not advertise performance_frame_timing_v1".to_owned(),
                        );
                    }
                    diagnostics
                })
        } else {
            Ok(serde_json::Value::Null)
        };
        match (completion, diagnostics) {
            (Ok(completion), Ok(mut metrics)) => {
                if environment.diagnostics_enabled
                    && responsiveness_fixture == FixtureKind::TaskBacklog
                    && !has_timer_task_samples(&metrics, 512)
                {
                    result.failure = Some(ResourceBenchmarkFailure {
                        kind: ResourceFailureKind::Sampling,
                        message: "Engine returned fewer than 512 timer-source task samples for the fixed task-backlog workload".to_owned(),
                    });
                } else {
                    if !completion.fixture_metrics.is_null() {
                        metrics["fixture_metrics"] = completion.fixture_metrics;
                    }
                    result.responsiveness = Some(velocity_core::ResponsivenessIterationData {
                        workload_duration_ms: completion.duration_ms,
                        metrics,
                        process_resources,
                        process_resource_error,
                    });
                }
            }
            (Err(message), _) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: if cancelled.load(Ordering::Relaxed) {
                        ResourceFailureKind::Cancelled
                    } else {
                        ResourceFailureKind::ReadinessTimeout
                    },
                    message,
                });
            }
            (_, Err(error)) => {
                result.failure = Some(ResourceBenchmarkFailure {
                    kind: ResourceFailureKind::Sampling,
                    message: error.to_string(),
                });
            }
        }
        cleanup(&mut result);
        return result;
    }
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
fn wait_for_responsiveness_completion(
    client: &mut velocity_browser::PhotonAutomationClient,
    tab_id: u64,
    timeout: std::time::Duration,
    cancelled: &std::sync::atomic::AtomicBool,
    fixture: FixtureKind,
    mut sample_resources: impl FnMut(),
) -> Result<ResponsivenessCompletion, String> {
    use std::{sync::atomic::Ordering, thread, time::Instant};

    let deadline = Instant::now() + timeout;
    let (completion_prefix, expected_count) = match fixture {
        FixtureKind::TaskBacklog => ("VelocityTaskBacklog:", 512_u64),
        FixtureKind::GcPressure => (
            "VelocityGCPressure:",
            velocity_core::GC_PRESSURE_RECORD_COUNT,
        ),
        FixtureKind::DomStyle => ("VelocityDOMStyle:", velocity_core::DOM_STYLE_MUTATION_COUNT),
        FixtureKind::StyleLocalized
        | FixtureKind::StyleDescendant
        | FixtureKind::StyleSibling
        | FixtureKind::StyleRelational => (
            "VelocityStyleInvalidation:",
            velocity_core::STYLE_INVALIDATION_MUTATION_COUNT,
        ),
        FixtureKind::LayoutUpdates => {
            ("VelocityLayoutUpdates:", velocity_core::LAYOUT_UPDATE_COUNT)
        }
        FixtureKind::Animation => ("VelocityAnimation:", velocity_core::ANIMATION_PHASE_COUNT),
        _ => return Err("unsupported responsiveness fixture".to_owned()),
    };
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(format!("{} workload was cancelled", fixture.name()));
        }
        let tab = client
            .tab_status(tab_id, std::time::Duration::from_secs(2))
            .map_err(|error| error.to_string())?;
        sample_resources();
        if let Some(title) = tab.title.as_deref()
            && let Some(result) = title.strip_prefix(completion_prefix)
        {
            let fields = result.split(':').collect::<Vec<_>>();
            let completed = fields
                .first()
                .and_then(|field| field.parse::<u64>().ok())
                .ok_or_else(|| "fixture returned an invalid completion count".to_owned())?;
            let duration_ms = fields
                .get(1)
                .copied()
                .and_then(|field| field.parse::<f64>().ok())
                .filter(|duration| duration.is_finite() && *duration > 0.0)
                .ok_or_else(|| "fixture returned an invalid duration".to_owned())?;
            let correct = fields
                .get(if fixture == FixtureKind::Animation {
                    7
                } else {
                    2
                })
                .copied()
                == Some("true");
            if completed != expected_count || !correct {
                return Err(format!(
                    "{} fixture correctness check failed (completed {completed}/{expected_count}, correct={correct})",
                    fixture.name()
                ));
            }
            let fixture_metrics = if fixture == FixtureKind::Animation {
                let parse_number = |index: usize| {
                    fields
                        .get(index)
                        .and_then(|field| field.parse::<f64>().ok())
                        .filter(|value| value.is_finite() && *value >= 0.0)
                };
                let callback_count = fields
                    .get(2)
                    .and_then(|field| field.parse::<u64>().ok())
                    .ok_or_else(|| "animation fixture returned an invalid rAF count".to_owned())?;
                serde_json::json!({
                    "scope": "browser_observable_request_animation_frame",
                    "active_window_duration_ms": duration_ms,
                    "raf_callback_count": callback_count,
                    "raf_interval_p50_ms": parse_number(3),
                    "raf_interval_p95_ms": parse_number(4),
                    "raf_interval_p99_ms": parse_number(5),
                    "raf_interval_max_ms": parse_number(6),
                    "phase_count": completed,
                    "phase_duration_ms": velocity_core::ANIMATION_PHASE_DURATION_MS,
                })
            } else {
                serde_json::Value::Null
            };
            return Ok(ResponsivenessCompletion {
                duration_ms,
                fixture_metrics,
            });
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{} fixture did not finish within {:.1} seconds",
                fixture.name(),
                timeout.as_secs_f64()
            ));
        }
        thread::sleep(std::time::Duration::from_millis(30));
    }
}

struct ResponsivenessCompletion {
    duration_ms: f64,
    fixture_metrics: serde_json::Value,
}

fn has_timer_task_samples(snapshot: &serde_json::Value, minimum_count: u64) -> bool {
    if snapshot
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return false;
    }
    let Some(metrics) = snapshot.get("metrics") else {
        return false;
    };
    if metrics
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
        || metrics.get("scope").and_then(serde_json::Value::as_str) != Some("webcontent_processes")
    {
        return false;
    }
    snapshot
        .pointer("/metrics/processes")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|processes| {
            processes.iter().any(|process| {
                let Some(diagnostics) = process.get("diagnostics") else {
                    return false;
                };
                diagnostics
                    .get("schema_version")
                    .and_then(serde_json::Value::as_u64)
                    == Some(1)
                    && diagnostics.get("scope").and_then(serde_json::Value::as_str)
                        == Some("webcontent_process")
                    && diagnostics
                        .get("queue_wait_unit")
                        .and_then(serde_json::Value::as_str)
                        == Some("ms")
                    && process
                        .get("available")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    && process
                        .pointer("/diagnostics/by_source")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|sources| {
                            sources.iter().any(|source| {
                                source.get("source").and_then(serde_json::Value::as_str)
                                    == Some("timer")
                                    && source
                                        .get("sample_count")
                                        .and_then(serde_json::Value::as_u64)
                                        .is_some_and(|count| count >= minimum_count)
                            })
                        })
            })
        })
}

#[cfg(target_os = "macos")]
fn build_responsiveness_report(
    installation: &velocity_browser::BrowserInstallation,
    iterations: Vec<ResourceBenchmarkIteration>,
    requested_iterations: usize,
    warmup_count: usize,
    timestamp_unix_ms: u128,
    run_id: String,
    cancelled: bool,
    diagnostics_enabled: bool,
    balanced_seed: Option<u64>,
    fixture: FixtureKind,
) -> velocity_core::ResponsivenessBenchmarkReport {
    use std::collections::BTreeMap;
    use velocity_core::{
        ANIMATION_FIXTURE_REVISION, DOM_STYLE_FIXTURE_REVISION, DOM_STYLE_MUTATION_COUNT,
        GC_PRESSURE_FIXTURE_REVISION, GC_PRESSURE_RECORD_COUNT, LAYOUT_UPDATE_COUNT,
        LAYOUT_UPDATES_FIXTURE_REVISION, RESPONSIVENESS_SCHEMA_VERSION,
        ResponsivenessBenchmarkReport, ResponsivenessCoverage, ResponsivenessMachineInformation,
        ResponsivenessSample, STYLE_INVALIDATION_FIXTURE_REVISION, TASK_BACKLOG_FIXTURE_REVISION,
    };
    use velocity_metrics::{ResourceCoverage, summarize_metric_values};

    let mut coverage = ResponsivenessCoverage {
        requested_iterations,
        warmup_iterations: warmup_count,
        ..ResponsivenessCoverage::default()
    };
    let mut metric_values = BTreeMap::<String, Vec<f64>>::new();
    let mut display_refresh_rates = Vec::<f64>::new();
    let workload = fixture.name();
    let mut warnings = vec![
        "The workload does not inject native user input or measure end-to-end input-to-presentation latency.".to_owned(),
        "Engine diagnostics expose elapsed style-update timing, style-reaction counters, and full/partial-layout timing. Ladybird does not expose a reliable reason or a counter for full-document style recomputation, and broad matching is not equivalent to full-document traversal.".to_owned(),
        "Engine and executable revisions are unknown because the current Photon automation metadata does not expose verified build revisions.".to_owned(),
    ];
    if fixture == FixtureKind::TaskBacklog {
        warnings.push("The task-backlog fixture schedules timer-source tasks; it does not create a GC-pressure workload.".to_owned());
    } else if fixture == FixtureKind::GcPressure {
        warnings.push("The GC-pressure fixture allocates and releases a deterministic object graph, but never forces collection. GC timing reports only collections that occurred naturally during the measured session.".to_owned());
    }
    if diagnostics_enabled || balanced_seed.is_some() {
        warnings.push("Task queue measurements are scoped to each WebContent process and may include work from every document hosted by that process.".to_owned());
    }
    if !diagnostics_enabled && balanced_seed.is_none() {
        warnings.push("Engine diagnostics were disabled. This report contains fixture wall duration, throughput, and independently collected process-tree resources only, for instrumentation-overhead control runs.".to_owned());
    }
    if let Some(seed) = balanced_seed {
        warnings.push(format!("Diagnostics overhead was measured in randomized on/off pairs using seed {seed}; these paired differences are a dedicated collection-overhead assessment and must not be interpreted as optimization results."));
    }
    warnings.push("Process-tree resource samples use verified process identity and avoid duplicate process identities. They are process-session scoped, can include Photon and WebContent processes, and are not per-tab memory.".to_owned());
    let samples = iterations
        .into_iter()
        .map(|iteration| {
            let responsiveness = iteration.responsiveness;
            let sample_diagnostics_enabled = iteration.diagnostics_enabled;
            let diagnostic_pair_id = iteration.diagnostic_pair_id;
            let mut metrics = responsiveness.as_ref().map(|sample| sample.metrics.clone());
            let process_resources = responsiveness
                .as_ref()
                .and_then(|sample| sample.process_resources.clone());
            let process_resource_error = responsiveness
                .as_ref()
                .and_then(|sample| sample.process_resource_error.clone());
            if iteration.warmup {
                return ResponsivenessSample {
                    iteration: iteration.iteration,
                    warmup: true,
                    diagnostic_pair_id,
                    diagnostics_enabled: sample_diagnostics_enabled,
                    workload_duration_ms: responsiveness.map(|sample| sample.workload_duration_ms),
                    metrics,
                    process_resources,
                    process_resource_error,
                    failure: iteration.failure.map(|failure| failure.message),
                };
            }

            if let Some(responsiveness) = responsiveness.as_ref() {
                coverage.successful_iterations += 1;
                match fixture {
                    FixtureKind::TaskBacklog => {
                        insert_metric(
                            &mut metric_values,
                            "task_backlog_duration_ms",
                            responsiveness.workload_duration_ms,
                        );
                        if responsiveness.workload_duration_ms > 0.0 {
                            insert_metric(
                                &mut metric_values,
                                "task_backlog_throughput_tasks_per_second",
                                512_000.0 / responsiveness.workload_duration_ms,
                            );
                        }
                    }
                    FixtureKind::GcPressure => {
                        insert_metric(
                            &mut metric_values,
                            "gc_pressure_duration_ms",
                            responsiveness.workload_duration_ms,
                        );
                        if responsiveness.workload_duration_ms > 0.0 {
                            insert_metric(
                                &mut metric_values,
                                "gc_pressure_records_per_second",
                                GC_PRESSURE_RECORD_COUNT as f64 * 1000.0
                                    / responsiveness.workload_duration_ms,
                            );
                        }
                    }
                    FixtureKind::DomStyle => {
                        insert_metric(
                            &mut metric_values,
                            "dom_style_duration_ms",
                            responsiveness.workload_duration_ms,
                        );
                        if responsiveness.workload_duration_ms > 0.0 {
                            insert_metric(
                                &mut metric_values,
                                "dom_style_mutations_per_second",
                                DOM_STYLE_MUTATION_COUNT as f64 * 1000.0
                                    / responsiveness.workload_duration_ms,
                            );
                        }
                    }
                    FixtureKind::StyleLocalized
                    | FixtureKind::StyleDescendant
                    | FixtureKind::StyleSibling
                    | FixtureKind::StyleRelational => {
                        let metric = format!("{}_duration_ms", fixture.name().replace('-', "_"));
                        insert_metric(
                            &mut metric_values,
                            &metric,
                            responsiveness.workload_duration_ms,
                        );
                        if responsiveness.workload_duration_ms > 0.0 {
                            let metric = format!(
                                "{}_mutations_per_second",
                                fixture.name().replace('-', "_")
                            );
                            insert_metric(
                                &mut metric_values,
                                &metric,
                                velocity_core::STYLE_INVALIDATION_MUTATION_COUNT as f64 * 1000.0
                                    / responsiveness.workload_duration_ms,
                            );
                        }
                    }
                    FixtureKind::LayoutUpdates => {
                        insert_metric(
                            &mut metric_values,
                            "layout_update_duration_ms",
                            responsiveness.workload_duration_ms,
                        );
                        if responsiveness.workload_duration_ms > 0.0 {
                            insert_metric(
                                &mut metric_values,
                                "layout_updates_per_second",
                                LAYOUT_UPDATE_COUNT as f64 * 1000.0
                                    / responsiveness.workload_duration_ms,
                            );
                        }
                    }
                    FixtureKind::Animation => {
                        insert_metric(
                            &mut metric_values,
                            "animation_active_window_ms",
                            responsiveness.workload_duration_ms,
                        );
                        if let Some(fixture_metrics) = responsiveness
                            .metrics
                            .get("fixture_metrics")
                            .filter(|value| !value.is_null())
                        {
                            for (name, key) in [
                                ("animation_raf_callback_count", "raf_callback_count"),
                                ("raf_interval_p50_ms", "raf_interval_p50_ms"),
                                ("raf_interval_p95_ms", "raf_interval_p95_ms"),
                                ("raf_interval_p99_ms", "raf_interval_p99_ms"),
                                ("raf_interval_max_ms", "raf_interval_max_ms"),
                            ] {
                                if let Some(value) =
                                    fixture_metrics.get(key).and_then(serde_json::Value::as_f64)
                                {
                                    insert_metric(&mut metric_values, name, value);
                                }
                            }
                        }
                    }
                    _ => unreachable!("responsiveness supports only its dedicated fixtures"),
                }
                if let Some(resources) = process_resources.as_ref() {
                    coverage.process_resource_iterations += 1;
                    coverage.process_resource_sample_count += resources.samples.len();
                    let complete = !resources.samples.is_empty()
                        && resources
                            .samples
                            .iter()
                            .all(|sample| sample.coverage == ResourceCoverage::SessionScoped);
                    coverage.process_resource_incomplete_sample_count += resources
                        .samples
                        .iter()
                        .filter(|sample| sample.coverage != ResourceCoverage::SessionScoped)
                        .count();
                    if complete {
                        coverage.process_resource_complete_iterations += 1;
                        add_process_resource_metrics(resources, &mut metric_values);
                    }
                }
                if let Some(error) = process_resource_error.as_deref() {
                    warnings.push(format!(
                        "Process-tree resource sampling was incomplete: {error}"
                    ));
                }
                if sample_diagnostics_enabled == Some(true) {
                    add_iteration_metrics(
                        &responsiveness.metrics,
                        &mut coverage,
                        &mut metric_values,
                    );
                    if let Some(metrics) = metrics.as_mut() {
                        add_frame_timing_metrics(
                            metrics,
                            &mut coverage,
                            &mut metric_values,
                            &mut display_refresh_rates,
                            &mut warnings,
                            (fixture == FixtureKind::Animation)
                                .then_some(velocity_core::ANIMATION_PRE_ROLL_MS),
                        );
                    }
                }
            } else {
                coverage.failed_iterations += 1;
            }

            ResponsivenessSample {
                iteration: iteration.iteration,
                warmup: false,
                diagnostic_pair_id,
                diagnostics_enabled: sample_diagnostics_enabled,
                workload_duration_ms: responsiveness.map(|sample| sample.workload_duration_ms),
                metrics,
                process_resources,
                process_resource_error,
                failure: iteration.failure.map(|failure| failure.message),
            }
        })
        .collect::<Vec<_>>();

    let diagnostics_successful_iterations = samples
        .iter()
        .filter(|sample| {
            !sample.warmup && sample.failure.is_none() && sample.diagnostics_enabled == Some(true)
        })
        .count();
    if (diagnostics_enabled || balanced_seed.is_some())
        && coverage.complete_single_process_samples < diagnostics_successful_iterations
    {
        warnings.push(format!(
            "{} of {} diagnostics-enabled iterations had exactly one available WebContent process diagnostics snapshot; cross-process metric distributions are not merged.",
            coverage.complete_single_process_samples, diagnostics_successful_iterations
        ));
    }
    if coverage.successful_iterations == 0 {
        warnings.push("No measured iteration produced a usable responsiveness sample.".to_owned());
    }
    if coverage.process_resource_complete_iterations < coverage.successful_iterations {
        warnings.push(format!(
            "Only {} of {} successful iterations had complete process-tree resource samples; incomplete attribution is retained in raw samples and excluded from resource summaries.",
            coverage.process_resource_complete_iterations, coverage.successful_iterations
        ));
    }
    if (diagnostics_enabled || balanced_seed.is_some())
        && fixture == FixtureKind::GcPressure
        && coverage.gc_timing_available_snapshots == 0
    {
        warnings.push("No main-thread JS heap collection occurred during the measured workload; GC timing distributions are unavailable.".to_owned());
    }
    display_refresh_rates.sort_by(f64::total_cmp);
    display_refresh_rates.dedup_by(|left, right| (*left - *right).abs() < 0.1);
    if display_refresh_rates.len() > 1 {
        warnings.push("The window's nominal display refresh rate changed during this report; intervals remain raw Metal presentation timestamps and are not classified as dropped frames against one fixed refresh target.".to_owned());
    }

    let metrics = metric_values
        .into_iter()
        .filter_map(|(name, values)| summarize_metric_values(values).map(|summary| (name, summary)))
        .collect();

    let mut configuration = serde_json::json!({
        "iterations": requested_iterations,
        "warmup_count": warmup_count,
        "requested_iteration_pairs": balanced_seed.map(|_| requested_iterations / 2),
        "requested_warmup_pairs": balanced_seed.map(|_| warmup_count / 2),
        "task_count": (fixture == FixtureKind::TaskBacklog).then_some(512),
        "task_source": (fixture == FixtureKind::TaskBacklog).then_some("timer"),
        "dom_style_mutation_count": (fixture == FixtureKind::DomStyle).then_some(DOM_STYLE_MUTATION_COUNT),
        "dom_style_branch_count": (fixture == FixtureKind::DomStyle).then_some(16),
        "dom_style_items_per_branch": (fixture == FixtureKind::DomStyle).then_some(32),
        "dom_style_selector_kinds": (fixture == FixtureKind::DomStyle).then_some(vec!["descendant", "sibling", "state", "relational"]),
        "style_invalidation_mutation_count": matches!(fixture, FixtureKind::StyleLocalized | FixtureKind::StyleDescendant | FixtureKind::StyleSibling | FixtureKind::StyleRelational).then_some(velocity_core::STYLE_INVALIDATION_MUTATION_COUNT),
        "style_invalidation_tree_shape": matches!(fixture, FixtureKind::StyleLocalized | FixtureKind::StyleDescendant | FixtureKind::StyleSibling | FixtureKind::StyleRelational).then_some("16 branches x 32 items with one leaf per item"),
        "style_invalidation_selector": match fixture {
            FixtureKind::StyleLocalized => Some("isolated class selector"),
            FixtureKind::StyleDescendant => Some("ancestor class descendant selector"),
            FixtureKind::StyleSibling => Some("adjacent and general sibling selectors"),
            FixtureKind::StyleRelational => Some(":has() descendant dependency"),
            _ => None,
        },
        "layout_update_count": (fixture == FixtureKind::LayoutUpdates).then_some(LAYOUT_UPDATE_COUNT),
        "layout_normal_flow_count": (fixture == FixtureKind::LayoutUpdates).then_some(256),
        "layout_absolute_count": (fixture == FixtureKind::LayoutUpdates).then_some(128),
        "animation_phase_count": (fixture == FixtureKind::Animation).then_some(velocity_core::ANIMATION_PHASE_COUNT),
        "animation_phase_duration_ms": (fixture == FixtureKind::Animation).then_some(velocity_core::ANIMATION_PHASE_DURATION_MS),
        "animation_pre_roll_ms": (fixture == FixtureKind::Animation).then_some(velocity_core::ANIMATION_PRE_ROLL_MS),
        "frame_interval_pre_roll_rule_version": (fixture == FixtureKind::Animation).then_some(velocity_core::FRAME_INTERVAL_PRE_ROLL_RULE_VERSION),
        "allocation_record_count": (fixture == FixtureKind::GcPressure).then_some(GC_PRESSURE_RECORD_COUNT),
        "allocation_cycles": (fixture == FixtureKind::GcPressure).then_some(32),
        "records_per_cycle": (fixture == FixtureKind::GcPressure).then_some(25_000),
        "retained_graph_sample_period": (fixture == FixtureKind::GcPressure).then_some(128),
        "temporary_dom_nodes_per_cycle": (fixture == FixtureKind::GcPressure).then_some(16),
        "sample_capacity_per_webcontent_process": 1024,
        "diagnostic_schema_version": 1,
        "process_resource_sampling_interval_ms": 100,
        "process_resource_scope": "verified_managed_process_session_tree",
        "diagnostics_enabled": if balanced_seed.is_some() { serde_json::Value::Null } else { serde_json::json!(diagnostics_enabled) },
        "diagnostics_mode": if balanced_seed.is_some() { "balanced_interleaved" } else if diagnostics_enabled { "enabled" } else { "disabled" },
        "diagnostics_order_seed": balanced_seed,
        "profile_isolation": "fresh_private_profile_per_iteration",
    });
    let hardware = super::history::host_hardware_metadata();
    configuration["machine_hardware"] = serde_json::json!({
        "machine_model": hardware.machine_model,
        "cpu_model": hardware.cpu_model,
        "logical_cpu_count": hardware.logical_cpu_count,
    });
    if let Some(seed) = balanced_seed {
        configuration["diagnostic_overhead_assessment"] =
            super::responsiveness_cli::paired_overhead_assessment(&samples, seed);
    }

    ResponsivenessBenchmarkReport {
        benchmark: "responsiveness".to_owned(),
        benchmark_type: "browser_responsiveness".to_owned(),
        schema_version: RESPONSIVENESS_SCHEMA_VERSION,
        run_id,
        timestamp_unix_ms,
        browser: ReportBrowser::from(installation),
        browser_version: installation.version.clone(),
        engine_revision: None,
        executable_revision: None,
        workload: workload.to_owned(),
        fixture_revision: match fixture {
            FixtureKind::TaskBacklog => TASK_BACKLOG_FIXTURE_REVISION,
            FixtureKind::GcPressure => GC_PRESSURE_FIXTURE_REVISION,
            FixtureKind::DomStyle => DOM_STYLE_FIXTURE_REVISION,
            FixtureKind::StyleLocalized
            | FixtureKind::StyleDescendant
            | FixtureKind::StyleSibling
            | FixtureKind::StyleRelational => STYLE_INVALIDATION_FIXTURE_REVISION,
            FixtureKind::LayoutUpdates => LAYOUT_UPDATES_FIXTURE_REVISION,
            FixtureKind::Animation => ANIMATION_FIXTURE_REVISION,
            _ => unreachable!("responsiveness supports only its dedicated fixtures"),
        }
        .to_owned(),
        configuration,
        machine_information: ResponsivenessMachineInformation {
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
        },
        display_refresh_rate_hz: (display_refresh_rates.len() == 1)
            .then(|| display_refresh_rates[0]),
        iterations: requested_iterations,
        warmup_count,
        metrics,
        samples,
        coverage,
        warnings,
        cancelled,
    }
}

#[cfg(target_os = "macos")]
fn add_process_resource_metrics(
    resources: &velocity_metrics::ResourceReport,
    metric_values: &mut std::collections::BTreeMap<String, Vec<f64>>,
) {
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
            insert_metric(metric_values, name, value);
        }
    }
}

#[cfg(target_os = "macos")]
fn add_iteration_metrics(
    snapshot: &serde_json::Value,
    coverage: &mut velocity_core::ResponsivenessCoverage,
    metric_values: &mut std::collections::BTreeMap<String, Vec<f64>>,
) {
    let Some(processes) = snapshot
        .pointer("/metrics/processes")
        .and_then(serde_json::Value::as_array)
    else {
        return;
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
    coverage.process_scoped_snapshots += available.len();
    for process in &available {
        coverage.retained_task_samples += process
            .pointer("/diagnostics/sample_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as usize;
    }
    if available.len() != 1 {
        return;
    }
    let Some(diagnostics) = available[0].get("diagnostics") else {
        return;
    };
    let sample_count = diagnostics
        .get("sample_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    coverage.complete_single_process_samples += 1;
    if diagnostics
        .pointer("/style_and_layout/available")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        coverage.style_layout_available_snapshots += 1;
    } else {
        coverage.style_layout_incomplete_snapshots += 1;
    }
    if sample_count > 0 {
        for (metric, path) in [
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
                insert_metric(metric_values, metric, value);
            }
        }
        if let Some(timer) = diagnostics
            .pointer("/by_source")
            .and_then(serde_json::Value::as_array)
            .and_then(|sources| {
                sources.iter().find(|source| {
                    source.get("source").and_then(serde_json::Value::as_str) == Some("timer")
                })
            })
        {
            if let Some(value) = timer
                .pointer("/queue_wait/p95_ms")
                .and_then(serde_json::Value::as_f64)
            {
                insert_metric(metric_values, "timer_task_queue_wait_p95_ms", value);
            }
        }
    }
    if diagnostics
        .pointer("/style_and_layout/available")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        for (metric, path) in [
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
                insert_metric(metric_values, metric, value);
            }
        }
    }

    if let Some(collection_count) = diagnostics
        .pointer("/gc/collection_count")
        .and_then(serde_json::Value::as_f64)
    {
        insert_metric(metric_values, "gc_collection_count", collection_count);
    }
    let gc_timing_sample_count = diagnostics
        .pointer("/gc/timing_sample_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    if gc_timing_sample_count > 0 {
        coverage.gc_timing_available_snapshots += 1;
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
                    insert_metric(metric_values, &name, value);
                }
            }
        }
    }
    if diagnostics
        .pointer("/gc/gc_live_heap_available")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        coverage.gc_live_heap_available_snapshots += 1;
        if let Some(value) = diagnostics
            .pointer("/gc/gc_live_heap_bytes")
            .and_then(serde_json::Value::as_f64)
        {
            insert_metric(metric_values, "gc_live_heap_bytes", value);
        }
    }
}

#[cfg(target_os = "macos")]
fn add_frame_timing_metrics(
    diagnostics: &mut serde_json::Value,
    coverage: &mut velocity_core::ResponsivenessCoverage,
    metric_values: &mut std::collections::BTreeMap<String, Vec<f64>>,
    display_refresh_rates: &mut Vec<f64>,
    warnings: &mut Vec<String>,
    known_pre_roll_ms: Option<u64>,
) {
    use serde_json::{Value, json};

    let Some(frame_timing) = diagnostics.get("photon_frame_timing") else {
        diagnostics["frame_timing_summary"] = json!({
            "classification": "missing_presentation_evidence",
            "reason": diagnostics
                .get("photon_frame_timing_error")
                .and_then(Value::as_str)
                .unwrap_or("frame timing diagnostics were not returned"),
        });
        return;
    };
    if frame_timing.get("schema_version").and_then(Value::as_u64) != Some(1)
        || frame_timing.get("scope").and_then(Value::as_str)
            != Some("photon_tab_engine_surface_frames")
    {
        diagnostics["frame_timing_summary"] = json!({
            "classification": "missing_presentation_evidence",
            "reason": "unsupported Photon frame timing schema",
        });
        return;
    }
    coverage.frame_timing_available_iterations += 1;
    if let Some(rate) = frame_timing
        .get("display_refresh_rate_hz")
        .and_then(Value::as_f64)
        .filter(|rate| rate.is_finite() && *rate > 0.0)
    {
        display_refresh_rates.push(rate);
    }
    let Some(frames) = frame_timing.get("frames").and_then(Value::as_array) else {
        diagnostics["frame_timing_summary"] = json!({
            "classification": "missing_presentation_evidence",
            "reason": "Photon frame timing response omitted its frame samples",
        });
        return;
    };

    let mut installed_count = 0_u64;
    let mut feedback_frame_count = 0_u64;
    let mut installed_feedback_frame_count = 0_u64;
    let mut feedback_event_count = 0_u64;
    let mut frame_keys = BTreeSet::new();
    let mut coalesced_frame_keys = BTreeSet::new();
    let mut install_by_surface_frame = std::collections::BTreeMap::new();
    let mut gpu_submission_count = 0_u64;
    let mut gpu_completion_callback_count = 0_u64;
    let mut stage_durations_ms = Vec::<(&'static str, f64)>::new();
    let gpu_correlation_supported = diagnostics
        .get("photon_gpu_sampling_correlation_supported")
        .and_then(Value::as_bool)
        == Some(true);

    for frame in frames {
        let Some(values) = frame.as_array() else {
            warnings.push("Photon frame timing included a malformed compact frame sample; that row was excluded.".to_owned());
            continue;
        };
        let generation = values.first().and_then(Value::as_u64).unwrap_or_default();
        let frame_id = values.get(1).and_then(Value::as_u64).unwrap_or_default();
        if generation != 0 && frame_id != 0 {
            frame_keys.insert((generation, frame_id));
        }
        let handoff_ns = values.get(2).and_then(Value::as_u64);
        let install_ns = values.get(3).and_then(Value::as_u64);
        let submission_ns = values.get(4).and_then(Value::as_u64);
        let completion_ns = values.get(5).and_then(Value::as_u64);
        let feedback_received = values.get(8).and_then(Value::as_bool).unwrap_or(false);
        let presentation_count = values.get(10).and_then(Value::as_u64).unwrap_or_default();
        let coalesced_before_install = values.get(12).and_then(Value::as_bool).unwrap_or(false);

        installed_count += u64::from(install_ns.is_some());
        gpu_submission_count += u64::from(submission_ns.is_some());
        gpu_completion_callback_count += u64::from(completion_ns.is_some());
        feedback_frame_count += u64::from(feedback_received);
        installed_feedback_frame_count += u64::from(install_ns.is_some() && feedback_received);
        feedback_event_count += presentation_count;
        if generation != 0 && frame_id != 0 {
            if let Some(install_ns) = install_ns {
                install_by_surface_frame.insert((generation, frame_id), install_ns);
            }
            if coalesced_before_install {
                coalesced_frame_keys.insert((generation, frame_id));
            }
        }
        if let (Some(handoff), Some(install)) = (handoff_ns, install_ns)
            && install >= handoff
        {
            stage_durations_ms.push((
                "surface_handoff_to_install_ms",
                (install - handoff) as f64 / 1_000_000.0,
            ));
        }
        if !gpu_correlation_supported
            && let (Some(install), Some(submission)) = (install_ns, submission_ns)
            && submission >= install
        {
            stage_durations_ms.push((
                "surface_install_to_gpu_submission_ms",
                (submission - install) as f64 / 1_000_000.0,
            ));
        }
        if generation == 0 || frame_id == 0 {
            warnings.push("Photon frame timing contained a zero generation or engine frame ID; verify frame ID consistency.".to_owned());
        }
    }

    let event_capability_supported = diagnostics
        .get("photon_drawable_presentation_events_supported")
        .and_then(Value::as_bool)
        == Some(true);
    let presentation_events = event_capability_supported
        .then(|| {
            frame_timing
                .get("presentation_events")
                .and_then(Value::as_array)
        })
        .flatten();
    let mut actual_frames = Vec::<(f64, u64, u64, u64)>::new();
    let mut zero_presented_time_feedback_count = 0_u64;
    let mut seen_surface_frames = BTreeSet::new();
    let mut feedback_surface_frames = BTreeSet::new();
    let mut presented_drawable_keys = BTreeSet::new();
    let mut reused_count = 0_u64;
    let mut callback_order_is_monotonic = true;
    let mut frame_ids_consistent = true;
    let mut previous_callback_time = None;
    let mut presentation_event_count = 0_u64;
    if let Some(events) = presentation_events {
        for event in events {
            let Some(values) = event.as_array() else {
                warnings.push("Photon drawable presentation timing included a malformed event; that event was excluded.".to_owned());
                continue;
            };
            let generation = values.first().and_then(Value::as_u64).unwrap_or_default();
            let frame_id = values.get(1).and_then(Value::as_u64).unwrap_or_default();
            let drawable_id = values.get(2).and_then(Value::as_u64).unwrap_or_default();
            let timestamp = values
                .get(3)
                .and_then(Value::as_f64)
                .filter(|timestamp| timestamp.is_finite() && *timestamp > 0.0);
            presentation_event_count += 1;
            if generation == 0 || frame_id == 0 || drawable_id == 0 {
                frame_ids_consistent = false;
                warnings.push("Photon drawable presentation timing contained a zero generation, engine frame ID, or drawable ID.".to_owned());
            }
            if generation != 0 && frame_id != 0 {
                feedback_surface_frames.insert((generation, frame_id));
            }
            let frame_samples_were_evicted = frame_timing
                .get("evicted_frame_sample_count")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                > 0;
            if !frame_keys.contains(&(generation, frame_id)) && !frame_samples_were_evicted {
                frame_ids_consistent = false;
            }
            let Some(timestamp) = timestamp else {
                zero_presented_time_feedback_count += 1;
                continue;
            };
            presented_drawable_keys.insert((generation, frame_id, drawable_id));
            if !seen_surface_frames.insert((generation, frame_id)) {
                reused_count += 1;
            }
            if previous_callback_time.is_some_and(|previous| timestamp < previous) {
                callback_order_is_monotonic = false;
            }
            previous_callback_time = Some(timestamp);
            actual_frames.push((timestamp, generation, frame_id, drawable_id));
        }
    } else {
        warnings.push("Photon did not provide the negotiated drawable presentation event stream; no actual display frame intervals were calculated.".to_owned());
    }
    if !gpu_correlation_supported {
        warnings.push("Photon did not advertise per-drawable GPU sampling correlation; GPU submission-to-completion callback latency is unavailable.".to_owned());
    }
    actual_frames.sort_by(|left, right| left.0.total_cmp(&right.0));
    if !callback_order_is_monotonic {
        warnings.push("Metal presented-handler callback order differed from presentedTime order; interval samples were ordered by the Metal timestamps.".to_owned());
    }
    if !frame_ids_consistent {
        warnings.push("At least one drawable presentation event did not match a retained Engine surface frame ID; frame samples may have been evicted.".to_owned());
    }

    let mut gpu_sampling_event_count = 0_u64;
    let mut gpu_sampling_completed_event_count = 0_u64;
    let mut gpu_sampling_presentation_match_count = 0_u64;
    let mut gpu_sampling_completion_durations_ms = Vec::new();
    let mut gpu_sampling_event_ids_consistent = true;
    let mut first_sampling_by_surface_frame = BTreeSet::new();
    if gpu_correlation_supported {
        if let Some(events) = frame_timing
            .get("gpu_sampling_events")
            .and_then(Value::as_array)
        {
            for event in events {
                let Some(values) = event.as_array() else {
                    warnings.push("Photon GPU sampling timing included a malformed event; that event was excluded.".to_owned());
                    continue;
                };
                let generation = values.first().and_then(Value::as_u64).unwrap_or_default();
                let frame_id = values.get(1).and_then(Value::as_u64).unwrap_or_default();
                let drawable_id = values.get(2).and_then(Value::as_u64).unwrap_or_default();
                let submitted_ns = values.get(3).and_then(Value::as_u64);
                let completed_ns = values.get(4).and_then(Value::as_u64);
                gpu_sampling_event_count += 1;
                if generation == 0 || frame_id == 0 || drawable_id == 0 || submitted_ns.is_none() {
                    gpu_sampling_event_ids_consistent = false;
                    warnings.push("Photon GPU sampling timing included an event with a zero identifier or missing submission timestamp.".to_owned());
                    continue;
                }
                if !frame_keys.contains(&(generation, frame_id))
                    && frame_timing
                        .get("evicted_frame_sample_count")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                        == 0
                {
                    gpu_sampling_event_ids_consistent = false;
                }
                if presented_drawable_keys.contains(&(generation, frame_id, drawable_id)) {
                    gpu_sampling_presentation_match_count += 1;
                }
                if let (Some(submitted_ns), Some(completed_ns)) = (submitted_ns, completed_ns) {
                    gpu_sampling_completed_event_count += 1;
                    if completed_ns < submitted_ns {
                        warnings.push("Photon GPU sampling completion callback preceded its matching drawable submission timestamp; that duration was excluded.".to_owned());
                        continue;
                    }
                    gpu_sampling_completion_durations_ms
                        .push((completed_ns - submitted_ns) as f64 / 1_000_000.0);
                    if first_sampling_by_surface_frame.insert((generation, frame_id))
                        && let Some(install_ns) =
                            install_by_surface_frame.get(&(generation, frame_id))
                        && submitted_ns >= *install_ns
                    {
                        stage_durations_ms.push((
                            "surface_install_to_gpu_submission_ms",
                            (submitted_ns - install_ns) as f64 / 1_000_000.0,
                        ));
                    }
                }
            }
        } else {
            warnings.push("Photon advertised GPU sampling correlation but omitted its bounded sampling events.".to_owned());
        }
    }
    if !gpu_sampling_event_ids_consistent {
        warnings.push("Photon GPU sampling events did not consistently match retained Engine surface frame IDs.".to_owned());
    }
    let evicted_gpu_sampling_event_count = frame_timing
        .get("evicted_gpu_sampling_event_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let gpu_sampling_retention_percent =
        (gpu_sampling_event_count + evicted_gpu_sampling_event_count > 0).then(|| {
            gpu_sampling_event_count as f64 * 100.0
                / (gpu_sampling_event_count + evicted_gpu_sampling_event_count) as f64
        });

    let mut intervals_ms = actual_frames
        .windows(2)
        .filter_map(|pair| {
            let interval = (pair[1].0 - pair[0].0) * 1000.0;
            (interval.is_finite() && interval > 0.0).then_some(interval)
        })
        .collect::<Vec<_>>();
    let mut excluded_pre_roll_interval_ms = None;
    let mut excluded_pre_roll_interval_index = None;
    if let Some(pre_roll_ms) = known_pre_roll_ms
        && let Some((interval_index, _interval)) = intervals_ms
            .iter()
            .copied()
            .enumerate()
            .take(7)
            .find(|(_, interval)| {
                *interval >= pre_roll_ms as f64 * 0.75 && *interval <= pre_roll_ms as f64 * 1.5
            })
    {
        excluded_pre_roll_interval_ms = Some(intervals_ms.remove(interval_index));
        excluded_pre_roll_interval_index = Some(interval_index);
    }
    let evicted_presentation_event_count = frame_timing
        .get("evicted_presentation_event_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let presentation_event_retention_percent =
        (presentation_event_count + evicted_presentation_event_count > 0).then(|| {
            presentation_event_count as f64 * 100.0
                / (presentation_event_count + evicted_presentation_event_count) as f64
        });
    let median_interval = nearest_rank(&intervals_ms, 0.5);
    let long_interval_count = if median_interval > 0.0 {
        intervals_ms
            .iter()
            .filter(|interval| **interval > median_interval * 2.0)
            .count()
    } else {
        0
    };
    let classification = if actual_frames.is_empty() {
        "missing_presentation_evidence"
    } else if intervals_ms.len() < 5 {
        "insufficient_frame_samples"
    } else if long_interval_count > 0 {
        "irregular_frame_delivery"
    } else {
        "consistent_frame_delivery"
    };

    for (name, fraction) in [
        ("frame_interval_p50_ms", 0.5),
        ("frame_interval_p95_ms", 0.95),
        ("frame_interval_p99_ms", 0.99),
    ] {
        if !intervals_ms.is_empty() {
            insert_metric(metric_values, name, nearest_rank(&intervals_ms, fraction));
        }
    }
    if let Some(maximum) = intervals_ms.iter().copied().max_by(f64::total_cmp) {
        insert_metric(metric_values, "frame_interval_max_ms", maximum);
    }
    for (name, value) in [
        (
            "frame_gpu_completion_callback_count",
            gpu_completion_callback_count as f64,
        ),
        ("frame_gpu_submission_count", gpu_submission_count as f64),
        (
            "frame_actual_drawable_presentation_count",
            actual_frames.len() as f64,
        ),
        ("frame_reused_count", reused_count as f64),
        ("engine_surface_update_count", frame_keys.len() as f64),
        (
            "presented_engine_surface_update_count",
            seen_surface_frames.len() as f64,
        ),
        ("repeated_surface_presentation_count", reused_count as f64),
        (
            "coalesced_surface_update_count",
            frame_timing
                .get("coalesced_before_install_count")
                .and_then(Value::as_u64)
                .unwrap_or_default() as f64,
        ),
        (
            "retained_coalesced_surface_update_count",
            coalesced_frame_keys.len() as f64,
        ),
        (
            "gpu_sampling_submission_event_count",
            gpu_sampling_event_count as f64,
        ),
        (
            "gpu_sampling_completed_event_count",
            gpu_sampling_completed_event_count as f64,
        ),
        (
            "gpu_sampling_presented_drawable_match_count",
            gpu_sampling_presentation_match_count as f64,
        ),
        (
            "frame_zero_presented_time_event_count",
            zero_presented_time_feedback_count as f64,
        ),
        (
            "frame_coalesced_before_install_count",
            frame_timing
                .get("coalesced_before_install_count")
                .and_then(Value::as_u64)
                .unwrap_or_default() as f64,
        ),
        ("frame_interval_outlier_count", long_interval_count as f64),
        ("frame_interval_sample_count", intervals_ms.len() as f64),
    ] {
        insert_metric(metric_values, name, value);
    }
    let installed_feedback_coverage_percent = (installed_count > 0)
        .then(|| installed_feedback_frame_count as f64 * 100.0 / installed_count as f64);
    if let Some(coverage_percent) = installed_feedback_coverage_percent {
        insert_metric(
            metric_values,
            "frame_presentation_feedback_coverage_percent",
            coverage_percent,
        );
    }
    if let Some(retention_percent) = presentation_event_retention_percent {
        insert_metric(
            metric_values,
            "frame_presentation_event_retention_percent",
            retention_percent,
        );
    }
    if let Some(retention_percent) = gpu_sampling_retention_percent {
        insert_metric(
            metric_values,
            "gpu_sampling_event_retention_percent",
            retention_percent,
        );
    }
    let evicted_frame_sample_count = frame_timing
        .get("evicted_frame_sample_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if evicted_frame_sample_count == 0 && evicted_presentation_event_count == 0 {
        let without_valid_presentation = frame_keys.difference(&seen_surface_frames).count() as f64;
        let without_drawable_feedback =
            frame_keys.difference(&feedback_surface_frames).count() as f64;
        insert_metric(
            metric_values,
            "surface_updates_without_valid_presentation_evidence_count",
            without_valid_presentation,
        );
        insert_metric(
            metric_values,
            "surface_updates_without_drawable_feedback_count",
            without_drawable_feedback,
        );
        insert_metric(
            metric_values,
            "surface_updates_with_feedback_but_no_valid_presented_time_count",
            feedback_surface_frames
                .difference(&seen_surface_frames)
                .count() as f64,
        );
    } else {
        warnings.push("The bounded Engine frame or drawable event buffer evicted samples, so the session-wide count of surface updates without presentation evidence is incomplete.".to_owned());
    }
    if gpu_correlation_supported && evicted_gpu_sampling_event_count > 0 {
        warnings.push("The bounded GPU sampling event buffer evicted callbacks; GPU sampling latency coverage is incomplete.".to_owned());
    }
    if !actual_frames.is_empty() {
        coverage.actual_presentation_iterations += 1;
    }
    coverage.frame_presentation_feedback_samples += presentation_event_count as usize;
    coverage.frame_interval_samples += intervals_ms.len();
    coverage.evicted_frame_timing_samples += frame_timing
        .get("evicted_frame_sample_count")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    for name in [
        "surface_handoff_to_install",
        "surface_install_to_gpu_submission",
    ] {
        let values = stage_durations_ms
            .iter()
            .filter_map(|(stage, duration)| (*stage == format!("{name}_ms")).then_some(*duration))
            .collect::<Vec<_>>();
        if !values.is_empty() {
            insert_metric(
                metric_values,
                &format!("{name}_p50_ms"),
                nearest_rank(&values, 0.5),
            );
            insert_metric(
                metric_values,
                &format!("{name}_p95_ms"),
                nearest_rank(&values, 0.95),
            );
        }
    }
    for (name, fraction) in [
        ("gpu_sampling_submission_to_completion_callback_p50_ms", 0.5),
        (
            "gpu_sampling_submission_to_completion_callback_p95_ms",
            0.95,
        ),
        (
            "gpu_sampling_submission_to_completion_callback_p99_ms",
            0.99,
        ),
    ] {
        if !gpu_sampling_completion_durations_ms.is_empty() {
            insert_metric(
                metric_values,
                name,
                nearest_rank(&gpu_sampling_completion_durations_ms, fraction),
            );
        }
    }
    if let Some(maximum) = gpu_sampling_completion_durations_ms
        .iter()
        .copied()
        .max_by(f64::total_cmp)
    {
        insert_metric(
            metric_values,
            "gpu_sampling_submission_to_completion_callback_max_ms",
            maximum,
        );
    }

    let measurement_window_ms = frame_timing
        .get("measurement_window_ms")
        .and_then(Value::as_f64);
    let mut frame_timing_summary = json!({
        "classification": classification,
        "classification_basis": "intervals longer than twice this sample's median are counted as long intervals; this is not a refresh-rate dropped-frame claim",
        "measurement_window_ms": measurement_window_ms,
        "known_fixture_pre_roll_ms": known_pre_roll_ms,
        "excluded_pre_roll_interval_ms": excluded_pre_roll_interval_ms,
        "excluded_pre_roll_interval_index": excluded_pre_roll_interval_index,
        "display_refresh_rate_hz": frame_timing.get("display_refresh_rate_hz"),
        "display_refresh_rate_change_count": frame_timing.get("display_refresh_rate_change_count"),
        "frame_sample_count": frames.len(),
        "installed_frame_count": installed_count,
        "gpu_submission_count": gpu_submission_count,
        "gpu_completion_callback_count": gpu_completion_callback_count,
        "frame_actual_drawable_presentation_count": actual_frames.len(),
        "presentation_feedback_frame_count": feedback_frame_count,
        "presentation_feedback_event_count": feedback_event_count,
        "drawable_presentation_event_count": presentation_event_count,
        "drawable_presentation_event_evicted_count": evicted_presentation_event_count,
        "frame_presentation_event_retention_percent": presentation_event_retention_percent,
        "drawable_presentation_event_stream_supported": event_capability_supported,
        "drawable_presentation_callback_order_is_monotonic": callback_order_is_monotonic,
        "frame_ids_consistent": frame_ids_consistent,
        "installed_frame_presentation_feedback_coverage_percent": installed_feedback_coverage_percent,
        "reused_frame_presentation_count": reused_count,
        "zero_presented_time_feedback_event_count": zero_presented_time_feedback_count,
        "coalesced_before_install_count": frame_timing.get("coalesced_before_install_count"),
        "frame_interval_sample_count": intervals_ms.len(),
        "frame_interval_p50_ms": (!intervals_ms.is_empty())
            .then(|| nearest_rank(&intervals_ms, 0.5)),
        "frame_interval_p95_ms": (!intervals_ms.is_empty())
            .then(|| nearest_rank(&intervals_ms, 0.95)),
        "frame_interval_p99_ms": (!intervals_ms.is_empty())
            .then(|| nearest_rank(&intervals_ms, 0.99)),
        "frame_interval_max_ms": intervals_ms
            .iter()
            .copied()
            .max_by(f64::total_cmp),
        "frame_interval_outlier_count": long_interval_count,
    });
    let additional_frame_timing_summary = json!({
        "evicted_frame_timing_sample_count": frame_timing.get("evicted_frame_sample_count"),
        "unique_presented_engine_surface_update_count": seen_surface_frames.len(),
        "unique_presented_engine_frame_count": seen_surface_frames.len(),
        "engine_surface_update_count": frame_keys.len(),
        "retained_coalesced_engine_surface_update_count": coalesced_frame_keys.len(),
        "coalesced_engine_surface_update_count": frame_timing.get("coalesced_before_install_count"),
        "surface_updates_without_drawable_feedback_count": (evicted_frame_sample_count == 0 && evicted_presentation_event_count == 0)
            .then(|| frame_keys.difference(&feedback_surface_frames).count()),
        "surface_updates_without_valid_presentation_evidence_count": (evicted_frame_sample_count == 0 && evicted_presentation_event_count == 0)
            .then(|| frame_keys.difference(&seen_surface_frames).count()),
        "surface_updates_with_feedback_but_no_valid_presented_time_count": (evicted_frame_sample_count == 0 && evicted_presentation_event_count == 0)
            .then(|| feedback_surface_frames.difference(&seen_surface_frames).count()),
        "visibility_and_occlusion_context_available": false,
        "engine_surface_to_presentation_latency_ms": null,
        "engine_surface_to_presentation_latency_limitation": "Engine surface publication has no timestamp in a verified clock domain shared with Metal presentedTime; drawable identity correlation does not establish pixel change or user visibility.",
        "gpu_sampling_correlation_supported": gpu_correlation_supported,
        "gpu_sampling_event_count": gpu_sampling_event_count,
        "gpu_sampling_completed_event_count": gpu_sampling_completed_event_count,
        "gpu_sampling_presented_drawable_match_count": gpu_sampling_presentation_match_count,
        "evicted_gpu_sampling_event_count": evicted_gpu_sampling_event_count,
        "gpu_sampling_submission_to_completion_callback_p50_ms": (!gpu_sampling_completion_durations_ms.is_empty())
            .then(|| nearest_rank(&gpu_sampling_completion_durations_ms, 0.5)),
        "gpu_sampling_submission_to_completion_callback_p95_ms": (!gpu_sampling_completion_durations_ms.is_empty())
            .then(|| nearest_rank(&gpu_sampling_completion_durations_ms, 0.95)),
        "gpu_sampling_submission_to_completion_callback_p99_ms": (!gpu_sampling_completion_durations_ms.is_empty())
            .then(|| nearest_rank(&gpu_sampling_completion_durations_ms, 0.99)),
    });
    if let (Some(summary), Some(additional)) = (
        frame_timing_summary.as_object_mut(),
        additional_frame_timing_summary.as_object(),
    ) {
        summary.extend(additional.clone());
    }
    diagnostics["frame_timing_summary"] = frame_timing_summary;
}

#[cfg(target_os = "macos")]
fn nearest_rank(values: &[f64], fraction: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() as f64 * fraction).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn insert_metric(
    metrics: &mut std::collections::BTreeMap<String, Vec<f64>>,
    name: &str,
    value: f64,
) {
    if value.is_finite() {
        metrics.entry(name.to_owned()).or_default().push(value);
    }
}

#[cfg(target_os = "macos")]
fn print_responsiveness_summary(report: &velocity_core::ResponsivenessBenchmarkReport) {
    use crate::terminal::{print_markdown, print_table};

    print_markdown(
        "## Responsiveness benchmark results\n\nValues summarize the measured fixture and available Engine diagnostics; they do not represent input-to-presentation latency.",
    );
    let rows: Vec<Vec<String>> = report
        .metrics
        .iter()
        .map(|(name, statistics)| {
            vec![
                name.clone(),
                format!("{:.3}", statistics.median),
                format!("{:.3}", statistics.mean),
                statistics.sample_count.to_string(),
            ]
        })
        .collect();
    print_table(&["Metric", "Median", "Mean", "Iterations"], rows, &[1, 2]);
    println!(
        "Coverage: {}/{} measured iterations; {} available process snapshots; {} retained task samples.",
        report.coverage.successful_iterations,
        report.coverage.requested_iterations,
        report.coverage.process_scoped_snapshots,
        report.coverage.retained_task_samples
    );
    for warning in &report.warnings {
        println!("Warning: {warning}");
    }
}

#[cfg(target_os = "macos")]
fn write_responsiveness_reports(
    json_path: &Path,
    html_path: &Path,
    report: &velocity_core::ResponsivenessBenchmarkReport,
) -> Result<(), String> {
    use std::fs;

    for parent in [json_path.parent(), html_path.parent()]
        .into_iter()
        .flatten()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("cannot serialize responsiveness report: {error}"))?;
    fs::write(json_path, json)
        .map_err(|error| format!("cannot write {}: {error}", json_path.display()))?;

    let metrics = report
        .metrics
        .iter()
        .map(|(name, stats)| {
            format!(
                "<tr><td>{}</td><td>{:.3}</td><td>{:.3}</td><td>{}</td></tr>",
                escape_html(name),
                stats.median,
                stats.mean,
                stats.sample_count
            )
        })
        .collect::<String>();
    let warnings = report
        .warnings
        .iter()
        .map(|warning| format!("<li>{}</li>", escape_html(warning)))
        .collect::<String>();
    let resource_timeline = report
        .samples
        .iter()
        .filter_map(|iteration| {
            let resources = iteration.process_resources.as_ref()?;
            Some(resources.samples.iter().map(move |sample| {
                format!(
                    "<tr><td>{}</td><td>{:.1}</td><td>{}</td><td>{}</td><td>{}</td><td>{:?}</td></tr>",
                    iteration.iteration,
                    sample.elapsed_ms,
                    sample.physical_footprint_bytes.map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
                    sample.resident_bytes.map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
                    sample.cpu_percent.map_or_else(|| "unavailable".to_owned(), |value| format!("{value:.2}")),
                    sample.coverage,
                )
            }))
        })
        .flatten()
        .collect::<String>();
    let frame_samples = report
        .samples
        .iter()
        .filter(|sample| !sample.warmup)
        .filter_map(|sample| {
            let summary = sample.metrics.as_ref()?.get("frame_timing_summary")?;
            Some((sample.iteration, summary))
        })
        .collect::<Vec<_>>();
    let frame_rows = frame_samples
        .iter()
        .map(|(iteration, summary)| {
            let value = |name: &str| {
                summary
                    .get(name)
                    .and_then(serde_json::Value::as_f64)
                    .map_or_else(|| "unavailable".to_owned(), |value| format!("{value:.3}"))
            };
            format!(
                "<tr><td>{iteration}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(summary.get("classification").and_then(serde_json::Value::as_str).unwrap_or("unavailable")),
                value("frame_interval_p50_ms"),
                value("frame_interval_p95_ms"),
                value("frame_interval_p99_ms"),
                value("frame_interval_max_ms"),
                summary.get("frame_interval_sample_count").and_then(serde_json::Value::as_u64).map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
                summary.get("display_refresh_rate_hz").and_then(serde_json::Value::as_f64).map_or_else(|| "unknown".to_owned(), |value| format!("{value:.2} Hz")),
            )
        })
        .collect::<String>();
    let frame_chart = render_frame_interval_chart(&frame_samples);
    let overhead_rows = report
        .configuration
        .pointer("/diagnostic_overhead_assessment/metrics")
        .and_then(serde_json::Value::as_object)
        .map(|metrics| {
            metrics.iter().map(|(name, metric)| {
                let number = |value: Option<&serde_json::Value>| value.and_then(serde_json::Value::as_f64).map_or_else(|| "unavailable".to_owned(), |value| format!("{value:.3}"));
                let interval = metric.get("bootstrap_95_percentile_interval_difference");
                let interval_label = interval.map_or_else(|| "unavailable (<5 pairs or missing resources)".to_owned(), |interval| format!("[{}, {}]", number(interval.get("low")), number(interval.get("high"))));
                format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(name), escape_html(metric.get("unit").and_then(serde_json::Value::as_str).unwrap_or("unknown")), metric.get("paired_sample_count").and_then(serde_json::Value::as_u64).unwrap_or_default(), number(metric.get("median_difference_on_minus_off")), interval_label, escape_html(&metric.get("median_relative_difference_percent").and_then(serde_json::Value::as_f64).map_or_else(|| "unavailable".to_owned(), |value| format!("{value:+.2}%"))))
            }).collect::<String>()
        })
        .unwrap_or_default();
    let overhead_section = if report
        .configuration
        .get("diagnostic_overhead_assessment")
        .is_some()
    {
        format!(
            "<h2>Paired diagnostics overhead</h2><p>Differences are diagnostics-on minus diagnostics-off. The bootstrap interval is descriptive paired uncertainty, not an automatic regression decision. Each pair uses separate fresh browser profiles.</p><table><thead><tr><th>Metric</th><th>Unit</th><th>Pairs</th><th>Median absolute difference</th><th>95% paired bootstrap interval</th><th>Median relative difference</th></tr></thead><tbody>{overhead_rows}</tbody></table>"
        )
    } else {
        String::new()
    };
    let html = format!(
        "<!doctype html><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Velocity responsiveness report</title><style>body{{font:15px/1.45 system-ui,sans-serif;max-width:1200px;margin:2rem auto;padding:0 1rem;color:#202833}}table{{border-collapse:collapse;width:100%;margin:1rem 0 2rem;display:block;overflow-x:auto}}th,td{{border-bottom:1px solid #ccd4dc;padding:.5rem;text-align:left;white-space:nowrap}}th{{background:#f2f5f7}}svg{{width:100%;height:auto;max-height:320px;border:1px solid #ccd4dc}}.warning{{margin:.4rem 0}}pre{{overflow:auto;max-height:32rem;background:#f2f5f7;padding:1rem}}</style><h1>Photon responsiveness — {}</h1><p>Run {} · {} · {} successful of {} measured iterations · refresh rate {}</p><table><thead><tr><th>Metric</th><th>Median</th><th>Mean</th><th>Iterations</th></tr></thead><tbody>{metrics}</tbody></table><h2>Presented frame intervals</h2><p>Intervals use every valid Metal drawable presentedTime event, including repeated displays of the same engine surface. They describe Photon window-drawable presentation cadence and do not prove that browser pixels changed. Percentiles below summarize each run's interval distribution; the chart compares per-run p50, p95, and maximum. These values do not infer missed refresh slots from a fixed refresh-rate threshold.</p>{frame_chart}<table><thead><tr><th>Run</th><th>Classification</th><th>p50 ms</th><th>p95 ms</th><th>p99 ms</th><th>Max ms</th><th>Intervals</th><th>Nominal display refresh</th></tr></thead><tbody>{frame_rows}</tbody></table>{overhead_section}<h2>Process-tree resource timeline</h2><p>Verified managed-session process tree; memory values are bytes and are not per-tab or JS heap measurements. Resident memory is RSS and may include shared pages.</p><table><thead><tr><th>Iteration</th><th>Elapsed ms</th><th>Physical footprint bytes</th><th>Resident bytes</th><th>CPU %</th><th>Attribution coverage</th></tr></thead><tbody>{resource_timeline}</tbody></table><h2>Warnings and limits</h2><ul>{warnings}</ul><h2>Raw samples</h2><pre>{}</pre>",
        escape_html(&report.workload),
        escape_html(&report.run_id),
        escape_html(&report.machine_information.operating_system),
        report.coverage.successful_iterations,
        report.coverage.requested_iterations,
        report
            .display_refresh_rate_hz
            .map_or_else(|| "unknown".to_owned(), |value| format!("{value:.2} Hz")),
        escape_html(&serde_json::to_string_pretty(report).unwrap_or_default()),
    );
    fs::write(html_path, html)
        .map_err(|error| format!("cannot write {}: {error}", html_path.display()))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn render_frame_interval_chart(samples: &[(usize, &serde_json::Value)]) -> String {
    if samples.is_empty() {
        return "<p>Drawable presentation feedback was unavailable for this run.</p>".to_owned();
    }
    let series = [
        ("p50", "frame_interval_p50_ms", ""),
        ("p95", "frame_interval_p95_ms", "8 4"),
        ("max", "frame_interval_max_ms", "2 4"),
    ];
    let maximum = samples
        .iter()
        .flat_map(|(_, summary)| {
            series
                .iter()
                .filter_map(|(_, key, _)| summary.get(*key).and_then(serde_json::Value::as_f64))
        })
        .filter(|value| value.is_finite())
        .max_by(f64::total_cmp)
        .unwrap_or(1.0)
        .max(1.0);
    let polylines = series
        .iter()
        .enumerate()
        .map(|(series_index, (label, key, dash))| {
            let points = samples
                .iter()
                .enumerate()
                .filter_map(|(index, (_, summary))| {
                    let value = summary.get(*key).and_then(serde_json::Value::as_f64)?;
                    if !value.is_finite() {
                        return None;
                    }
                    let x = if samples.len() == 1 {
                        400.0
                    } else {
                        40.0 + index as f64 * 720.0 / (samples.len() - 1) as f64
                    };
                    let y = 210.0 - value / maximum * 180.0;
                    Some(format!("{x:.1},{y:.1}"))
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "<polyline fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-dasharray=\"{dash}\" points=\"{points}\"/><text x=\"{}\" y=\"20\">{label}</text>",
                48 + series_index * 65
            )
        })
        .collect::<String>();
    format!(
        "<svg role=\"img\" aria-label=\"Frame interval p50, p95 and maximum across iterations\" viewBox=\"0 0 800 240\"><desc>Per-iteration frame interval statistics from Metal drawable presentedTime feedback. Vertical scale runs from zero to {maximum:.2} milliseconds.</desc><line x1=\"40\" y1=\"210\" x2=\"760\" y2=\"210\" stroke=\"currentColor\" stroke-opacity=\".35\"/><text x=\"5\" y=\"32\">{maximum:.1} ms</text><text x=\"24\" y=\"213\">0</text>{polylines}</svg>",
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
    use velocity_browser::{BrowserId, BrowserInstallation, BrowserMetadata, InstallationId};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn installation(id: &str) -> BrowserInstallation {
        BrowserInstallation {
            installation_id: InstallationId::new(format!("{id}-installation")).unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new(id).unwrap(),
                name: id.to_owned(),
                bundle_identifier: None,
                engine: None,
            },
            version: None,
            application_path: PathBuf::new(),
            executable_path: PathBuf::new(),
        }
    }

    #[test]
    fn responsiveness_accepts_a_selected_photon_installation_only() {
        let photon = installation("photon");
        assert!(validate_responsiveness_installations(std::slice::from_ref(&photon)).is_ok());

        let chrome = installation("chrome");
        assert!(validate_responsiveness_installations(std::slice::from_ref(&chrome)).is_err());
        assert!(validate_responsiveness_installations(&[photon, installation("photon")]).is_err());
    }

    #[test]
    fn responsiveness_workload_names_map_to_dedicated_fixtures() {
        assert_eq!(
            parse_responsiveness_fixture("task-backlog"),
            Ok(FixtureKind::TaskBacklog)
        );
        assert_eq!(
            parse_responsiveness_fixture("dom-style"),
            Ok(FixtureKind::DomStyle)
        );
        assert_eq!(
            parse_responsiveness_fixture("style-localized"),
            Ok(FixtureKind::StyleLocalized)
        );
        assert_eq!(
            parse_responsiveness_fixture("style-descendant"),
            Ok(FixtureKind::StyleDescendant)
        );
        assert_eq!(
            parse_responsiveness_fixture("style-sibling"),
            Ok(FixtureKind::StyleSibling)
        );
        assert_eq!(
            parse_responsiveness_fixture("style-relational"),
            Ok(FixtureKind::StyleRelational)
        );
        assert_eq!(
            parse_responsiveness_fixture("layout-updates"),
            Ok(FixtureKind::LayoutUpdates)
        );
        assert_eq!(
            parse_responsiveness_fixture("gc-pressure"),
            Ok(FixtureKind::GcPressure)
        );
        assert_eq!(
            parse_responsiveness_fixture("animation"),
            Ok(FixtureKind::Animation)
        );
        assert!(parse_responsiveness_fixture("scrolling").is_err());
    }

    #[test]
    fn diagnostics_overhead_pair_order_is_seeded_and_balanced() {
        let mut seed_a = 0x1234_5678_u64;
        let mut seed_b = seed_a;
        let first = (0..20)
            .map(|_| diagnostics_pair_order(&mut seed_a))
            .collect::<Vec<_>>();
        let replay = (0..20)
            .map(|_| diagnostics_pair_order(&mut seed_b))
            .collect::<Vec<_>>();
        assert_eq!(first, replay);
        assert!(
            first
                .iter()
                .all(|pair| *pair == [false, true] || *pair == [true, false])
        );
        assert!(first.iter().any(|pair| *pair == [false, true]));
        assert!(first.iter().any(|pair| *pair == [true, false]));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn frame_intervals_use_presented_drawable_times_and_refresh_is_context_only() {
        let times = [1.0, 1.01, 1.02, 1.03, 1.04, 1.09, 1.10];
        let frames = times
            .iter()
            .enumerate()
            .map(|(index, time)| {
                serde_json::json!([
                    1,
                    index + 1,
                    1_000,
                    if index == 0 {
                        serde_json::Value::Null
                    } else {
                        serde_json::json!(2_000)
                    },
                    3_000,
                    4_000,
                    1,
                    time,
                    true,
                    false,
                    1,
                    0,
                    false
                ])
            })
            .collect::<Vec<_>>();
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_gpu_sampling_correlation_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "display_refresh_rate_hz": 120.0,
                "measurement_window_ms": 100.0,
                "coalesced_before_install_count": 0,
                "evicted_frame_sample_count": 0,
                "frames": frames,
                "presentation_events": times.iter().enumerate().map(|(index, time)| {
                    serde_json::json!([1, index + 1, index + 11, time])
                }).collect::<Vec<_>>(),
                "gpu_sampling_events": []
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );
        let summary = &diagnostics["frame_timing_summary"];
        assert_eq!(summary["classification"], "irregular_frame_delivery");
        assert_eq!(summary["frame_interval_sample_count"], 6);
        assert_eq!(summary["frame_interval_p50_ms"], 10.000000000000009);
        assert_eq!(summary["frame_interval_max_ms"], 50.00000000000004);
        assert_eq!(summary["display_refresh_rate_hz"], 120.0);
        assert_eq!(coverage.actual_presentation_iterations, 1);
        assert_eq!(
            summary["installed_frame_presentation_feedback_coverage_percent"],
            100.0
        );
        assert_eq!(summary["presentation_feedback_frame_count"], 7);
        assert_eq!(summary["drawable_presentation_event_count"], 7);
        assert_eq!(summary["reused_frame_presentation_count"], 0);
        assert!(warnings.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gpu_sampling_callbacks_are_paired_by_engine_surface_and_drawable_identity() {
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_gpu_sampling_correlation_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "evicted_frame_sample_count": 0,
                "evicted_presentation_event_count": 0,
                "evicted_gpu_sampling_event_count": 0,
                "frames": [[1, 42, 500_000, 1_000_000, 2_000_000, 2_500_000, 7, 50.0, true, false, 2, 0, false]],
                "presentation_events": [[1, 42, 7, 50.0], [1, 42, 8, 50.016]],
                "gpu_sampling_events": [
                    [1, 42, 7, 2_000_000, 2_500_000],
                    [1, 42, 8, 3_000_000, 3_800_000],
                    [1, 42, 9, 4_000_000, 5_000_000]
                ]
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );

        assert_eq!(
            diagnostics["frame_timing_summary"]["gpu_sampling_presented_drawable_match_count"],
            2
        );
        assert_eq!(
            diagnostics["frame_timing_summary"]["reused_frame_presentation_count"],
            1
        );
        assert_eq!(
            metrics["gpu_sampling_submission_to_completion_callback_p50_ms"],
            vec![0.8]
        );
        assert_eq!(
            metrics["gpu_sampling_submission_to_completion_callback_max_ms"],
            vec![1.0]
        );
        assert!(warnings.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn animation_pre_roll_gap_is_reported_separately_from_frame_intervals() {
        let times = [
            10.0, 10.0167, 10.9167, 10.9334, 10.9501, 10.9668, 10.9835, 11.0002,
        ];
        let frames = times
            .iter()
            .enumerate()
            .map(|(index, time)| {
                serde_json::json!([1, index + 1, 1, 2, 3, 4, 1, time, true, false, 1, 0, false])
            })
            .collect::<Vec<_>>();
        let events = times
            .iter()
            .enumerate()
            .map(|(index, time)| serde_json::json!([1, index + 1, index + 1, time]))
            .collect::<Vec<_>>();
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_gpu_sampling_correlation_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "frames": frames,
                "presentation_events": events,
                "gpu_sampling_events": [],
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            Some(velocity_core::ANIMATION_PRE_ROLL_MS),
        );

        let summary = &diagnostics["frame_timing_summary"];
        assert_eq!(summary["known_fixture_pre_roll_ms"], 900);
        assert!((summary["excluded_pre_roll_interval_ms"].as_f64().unwrap() - 900.0).abs() < 0.001);
        assert_eq!(summary["excluded_pre_roll_interval_index"], 1);
        assert_eq!(summary["frame_interval_sample_count"], 6);
        assert_eq!(summary["classification"], "consistent_frame_delivery");
        assert!(warnings.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn presentation_intervals_include_reused_engine_surfaces() {
        let events = (0..10)
            .map(|index| {
                let frame_id = if index < 7 { 42 } else { index + 36 };
                serde_json::json!([1, frame_id, index + 1, 5.0 + index as f64 / 60.0])
            })
            .collect::<Vec<_>>();
        let frames = serde_json::json!([
            [
                1, 42, 1_000, 2_000, 3_000, 4_000, 1, 5.1, true, false, 7, 0, false
            ],
            [
                1, 43, 1_000, 2_000, 3_000, 4_000, 1, 5.2, true, false, 1, 0, false
            ],
            [
                1, 44, 1_000, 2_000, 3_000, 4_000, 1, 5.3, true, false, 1, 0, false
            ],
            [
                1, 45, 1_000, 2_000, 3_000, 4_000, 1, 5.4, true, false, 1, 0, false
            ],
        ]);
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_gpu_sampling_correlation_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "display_refresh_rate_hz": 60.0,
                "measurement_window_ms": 200.0,
                "evicted_presentation_event_count": 0,
                "frames": frames,
                "presentation_events": events,
                "gpu_sampling_events": [],
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );

        let summary = &diagnostics["frame_timing_summary"];
        assert_eq!(summary["drawable_presentation_event_count"], 10);
        assert_eq!(summary["frame_actual_drawable_presentation_count"], 10);
        assert_eq!(summary["unique_presented_engine_frame_count"], 4);
        assert_eq!(summary["reused_frame_presentation_count"], 6);
        assert_eq!(summary["frame_interval_sample_count"], 9);
        assert_eq!(summary["classification"], "consistent_frame_delivery");
        assert!(warnings.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_drawable_feedback_is_classified_without_fabricated_intervals() {
        let mut diagnostics = serde_json::json!({});
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );
        assert_eq!(
            diagnostics["frame_timing_summary"]["classification"],
            "missing_presentation_evidence"
        );
        assert!(!metrics.contains_key("frame_interval_p50_ms"));
        assert_eq!(coverage.frame_interval_samples, 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn frame_samples_without_event_stream_do_not_fabricate_display_intervals() {
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "frames": [
                    [1, 1, 1, 2, 3, 4, 8, 10.0, true, false, 1, 0, false],
                    [1, 2, 2, 3, 4, 5, 9, 10.016, true, false, 1, 0, false]
                ]
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );

        assert_eq!(
            diagnostics["frame_timing_summary"]["classification"],
            "missing_presentation_evidence"
        );
        assert_eq!(
            diagnostics["frame_timing_summary"]["frame_interval_sample_count"],
            0
        );
        assert!(!metrics.contains_key("frame_interval_p50_ms"));
        assert!(!warnings.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn presentation_intervals_sort_by_metal_time_and_report_callback_reordering() {
        let times = [8.02, 8.00, 8.01, 8.03, 8.04, 8.05, 8.06];
        let frames = times
            .iter()
            .enumerate()
            .map(|(index, time)| {
                serde_json::json!([1, index + 1, 1, 2, 3, 4, 1, time, true, false, 1, 0, false])
            })
            .collect::<Vec<_>>();
        let events = times
            .iter()
            .enumerate()
            .map(|(index, time)| serde_json::json!([1, index + 1, index + 1, time]))
            .collect::<Vec<_>>();
        let mut diagnostics = serde_json::json!({
            "photon_drawable_presentation_events_supported": true,
            "photon_frame_timing": {
                "schema_version": 1,
                "scope": "photon_tab_engine_surface_frames",
                "frames": frames,
                "presentation_events": events,
            }
        });
        let mut coverage = velocity_core::ResponsivenessCoverage::default();
        let mut metrics = BTreeMap::new();
        let mut refresh_rates = Vec::new();
        let mut warnings = Vec::new();
        add_frame_timing_metrics(
            &mut diagnostics,
            &mut coverage,
            &mut metrics,
            &mut refresh_rates,
            &mut warnings,
            None,
        );

        assert_eq!(
            diagnostics["frame_timing_summary"]["frame_interval_sample_count"],
            6
        );
        assert_eq!(
            diagnostics["frame_timing_summary"]["drawable_presentation_callback_order_is_monotonic"],
            false
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("callback order differed"))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn frame_interval_report_chart_renders_static_presentation_series() {
        let summary = serde_json::json!({
            "frame_interval_p50_ms": 8.0,
            "frame_interval_p95_ms": 16.0,
            "frame_interval_max_ms": 25.0,
        });
        let chart = render_frame_interval_chart(&[(1, &summary)]);
        assert!(chart.contains("<svg"));
        assert!(chart.contains("presentedTime"));
        assert!(chart.contains("<polyline"));
        assert!(chart.contains("p50"));
        assert!(chart.contains("p95"));
        assert!(chart.contains("max"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn responsiveness_html_describes_all_drawable_presentation_events() {
        use velocity_core::{
            ResponsivenessBenchmarkReport, ResponsivenessCoverage,
            ResponsivenessMachineInformation, ResponsivenessSample,
        };

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "velocity-responsiveness-html-{}-{stamp}",
            std::process::id()
        ));
        let report = ResponsivenessBenchmarkReport {
            benchmark: "responsiveness".to_owned(),
            benchmark_type: "browser_responsiveness".to_owned(),
            schema_version: velocity_core::RESPONSIVENESS_SCHEMA_VERSION,
            run_id: "html-test".to_owned(),
            timestamp_unix_ms: 0,
            browser: ReportBrowser {
                id: "photon".to_owned(),
                installation_id: "photon-test".to_owned(),
                name: "Photon test".to_owned(),
                version: None,
                executable_path: PathBuf::from("/fixture/Photon"),
            },
            browser_version: None,
            engine_revision: None,
            executable_revision: None,
            workload: "animation".to_owned(),
            fixture_revision: velocity_core::ANIMATION_FIXTURE_REVISION.to_owned(),
            configuration: serde_json::json!({}),
            machine_information: ResponsivenessMachineInformation {
                operating_system: "macos".to_owned(),
                architecture: "aarch64".to_owned(),
            },
            display_refresh_rate_hz: Some(60.0),
            iterations: 1,
            warmup_count: 0,
            metrics: BTreeMap::new(),
            samples: vec![ResponsivenessSample {
                iteration: 1,
                warmup: false,
                diagnostic_pair_id: None,
                diagnostics_enabled: Some(true),
                workload_duration_ms: Some(900.0),
                metrics: Some(serde_json::json!({
                    "frame_timing_summary": {
                        "classification": "irregular_frame_delivery",
                        "frame_interval_p50_ms": 16.667,
                        "frame_interval_p95_ms": 33.333,
                        "frame_interval_p99_ms": 50.0,
                        "frame_interval_max_ms": 50.0,
                        "frame_interval_sample_count": 5,
                        "display_refresh_rate_hz": 60.0
                    }
                })),
                process_resources: None,
                process_resource_error: None,
                failure: None,
            }],
            coverage: ResponsivenessCoverage {
                requested_iterations: 1,
                successful_iterations: 1,
                ..ResponsivenessCoverage::default()
            },
            warnings: Vec::new(),
            cancelled: false,
        };
        let json_path = directory.join("report.json");
        let html_path = directory.join("report.html");
        write_responsiveness_reports(&json_path, &html_path, &report).unwrap();
        let html = std::fs::read_to_string(&html_path).unwrap();

        assert!(html.contains("every valid Metal drawable presentedTime event"));
        assert!(html.contains("including repeated displays of the same engine surface"));
        assert!(html.contains("do not prove that browser pixels changed"));
        assert!(!html.contains("unique engine surface frame IDs"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn task_backlog_requires_samples_for_the_full_timer_workload() {
        let snapshot = |timer_samples| {
            serde_json::json!({
                "schema_version": 1,
                "metrics": {
                    "schema_version": 1,
                    "scope": "webcontent_processes",
                    "processes": [{
                        "available": true,
                        "diagnostics": {
                            "schema_version": 1,
                            "scope": "webcontent_process",
                            "queue_wait_unit": "ms",
                            "by_source": [{"source": "timer", "sample_count": timer_samples}]
                        }
                    }]
                }
            })
        };

        assert!(has_timer_task_samples(&snapshot(512), 512));
        assert!(has_timer_task_samples(&snapshot(513), 512));
        assert!(!has_timer_task_samples(&snapshot(511), 512));
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
            diagnostic_pair_id: None,
            diagnostics_enabled: None,
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
            responsiveness: None,
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
