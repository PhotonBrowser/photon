use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use serde::Serialize;
#[cfg(target_os = "macos")]
use velocity_browser::BrowserInstallation;
#[cfg(target_os = "macos")]
use velocity_metrics::{
    ManagedProcessTarget, ResourceCollector, ResourceConfig, ResourceMonitor, ResourceReport,
};

#[cfg(target_os = "macos")]
static CANCELLED: AtomicBool = AtomicBool::new(false);
const MAX_RETAINED_SAMPLES: u64 = 100_000;

#[derive(Clone, Debug)]
struct MonitorOptions {
    browser: Option<String>,
    duration_seconds: u64,
    interval_ms: u64,
    output_directory: PathBuf,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct MonitorResult {
    schema_version: u32,
    benchmark: &'static str,
    run_id: String,
    browser: BrowserResult,
    system: SystemResult,
    monitoring: MonitoringResult,
    managed_root_pid: u32,
    managed_session_id: u32,
    attribution_method: &'static str,
    cleanup: CleanupResult,
    resource: ResourceReport,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct BrowserResult {
    id: String,
    installation_id: String,
    name: String,
    version: Option<String>,
    application_path: PathBuf,
    executable_path: PathBuf,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct SystemResult {
    operating_system: &'static str,
    architecture: &'static str,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct MonitoringResult {
    duration_seconds_requested: u64,
    sampling_interval_ms: u64,
    cancelled: bool,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct CleanupResult {
    closed: bool,
    detail: Option<String>,
}

pub fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|argument| matches!(argument.as_str(), "--help" | "-h" | "help"))
    {
        print_help();
        return Ok(());
    }
    let options = parse_options(arguments.into_iter())?;
    run_monitor(options)
}

fn parse_options(mut arguments: impl Iterator<Item = String>) -> Result<MonitorOptions, String> {
    let mut browser = None;
    let mut duration_seconds = 10_u64;
    let mut interval_ms = 100_u64;
    let mut output_directory = default_output_directory();
    while let Some(option) = arguments.next() {
        match option.as_str() {
            "--browser" if browser.is_none() => {
                browser = Some(required_value(&mut arguments, "--browser")?)
            }
            "--duration" => duration_seconds = parse_value(&mut arguments, "--duration")?,
            "--interval" => interval_ms = parse_value(&mut arguments, "--interval")?,
            "--output" => {
                output_directory = PathBuf::from(required_value(&mut arguments, "--output")?)
            }
            value if !value.starts_with('-') && browser.is_none() => {
                browser = Some(value.to_owned())
            }
            other => return Err(format!("unknown monitor option '{other}'")),
        }
    }
    if !(1..=86_400).contains(&duration_seconds) {
        return Err("--duration must be between 1 and 86400 seconds".to_owned());
    }
    if !(10..=86_400_000).contains(&interval_ms) {
        return Err("--interval must be between 10 and 86400000 milliseconds".to_owned());
    }
    let expected_samples = duration_seconds.saturating_mul(1_000).div_ceil(interval_ms);
    if expected_samples > MAX_RETAINED_SAMPLES {
        return Err(format!(
            "requested run would retain {expected_samples} samples; increase --interval to stay at or below {MAX_RETAINED_SAMPLES}"
        ));
    }
    Ok(MonitorOptions {
        browser,
        duration_seconds,
        interval_ms,
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
        "## Resource monitor\n\n`velocity monitor [browser|number] [--duration SECONDS] [--interval MILLISECONDS] [--output DIRECTORY]`\n\nOmit the browser selector in an interactive terminal to choose from a numbered list. Defaults: 10 seconds, 100 ms, and `tools/velocity/results`. Intervals below 10 ms are rejected to limit sampler overhead; no more than 100,000 samples are retained. Monitoring launches an isolated Velocity-managed browser session and is diagnostic; it does not rank browsers.",
    );
}

#[cfg(target_os = "macos")]
fn run_monitor(options: MonitorOptions) -> Result<(), String> {
    use velocity_browser::{
        BrowserLifecycleManager, CloseOutcome, FileSessionStore,
        macos::{MacOSBrowserDiscovery, MacOSProcessDriver, default_session_store_path},
    };

    let registry = MacOSBrowserDiscovery
        .discover()
        .map_err(|error| error.to_string())?;
    let installation =
        crate::browser_selection::choose(&registry, options.browser.as_deref(), false)?[0].clone();
    fs::create_dir_all(&options.output_directory).map_err(|error| {
        format!(
            "cannot create result directory {}: {error}",
            options.output_directory.display()
        )
    })?;
    CANCELLED.store(false, Ordering::SeqCst);
    install_cancellation_handler()?;

    let store_path = default_session_store_path()?;
    let profile_root = store_path
        .parent()
        .ok_or_else(|| "Velocity session store has no parent directory".to_owned())?
        .join("Profiles");
    let run_id = unique_run_id();
    let profile_path =
        profile_root.join(format!("monitor-{}-{run_id}", installation.installation_id));
    if fs::symlink_metadata(&profile_path).is_ok() {
        return Err(format!(
            "refusing to reuse existing isolated profile {}",
            profile_path.display()
        ));
    }
    let driver = MacOSProcessDriver::new(profile_root).with_benchmark_profile(profile_path, false);
    let manager = BrowserLifecycleManager::new(driver, FileSessionStore::new(store_path));
    let session = manager
        .launch(&installation, None)
        .map_err(|error| error.to_string())?;
    let start_time = session.identity.start_time.ok_or_else(|| {
        close_after_setup_error(
            &manager,
            &session.session_id,
            "macOS did not provide a stable process start identity",
        )
    })?;
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
    let config = ResourceConfig {
        sampling_interval_ms: options.interval_ms,
    };
    let collector = velocity_browser::macos::MacOSResourceCollector::new(target.clone());
    let monitor = match ResourceMonitor::start(target, config, collector) {
        Ok(monitor) => monitor,
        Err(error) => {
            let cleanup = manager.close(&session.session_id).map_err(|cleanup_error| {
                format!("{error}; additionally, managed session cleanup failed: {cleanup_error}")
            });
            cleanup?;
            return Err(error.to_string());
        }
    };

    println!("Velocity — Resource Monitor\n");
    println!("Browser: {}", session.browser_name);
    println!("Duration: {} seconds", options.duration_seconds);
    println!("Sampling interval: {} ms", options.interval_ms);
    println!("\nMonitoring... Press Ctrl-C to stop and clean up.");

    let (resource, cancelled, close_result) = sample_and_cleanup(
        monitor,
        Duration::from_secs(options.duration_seconds),
        config.interval(),
        || CANCELLED.load(Ordering::SeqCst),
        || manager.close(&session.session_id),
    );
    let cleanup = match close_result {
        Ok(CloseOutcome::Closed(_)) | Ok(CloseOutcome::AlreadyExited(_)) => CleanupResult {
            closed: true,
            detail: None,
        },
        Err(error) => CleanupResult {
            closed: false,
            detail: Some(error.to_string()),
        },
    };
    let result = MonitorResult {
        schema_version: 1,
        benchmark: "resource_monitor",
        run_id: run_id.clone(),
        browser: browser_result(&installation),
        system: SystemResult {
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
        },
        monitoring: MonitoringResult {
            duration_seconds_requested: options.duration_seconds,
            sampling_interval_ms: options.interval_ms,
            cancelled,
        },
        managed_root_pid: session.identity.pid,
        managed_session_id: session.identity.session_id,
        attribution_method: "macOS libproc descendants verified by private session ID and parent relationship; root also matched by executable path and precise process start timestamp; PID reuse is excluded",
        cleanup,
        resource,
    };
    let output_path = write_result(&options.output_directory, &installation, &run_id, &result)?;
    print_summary(&result, &output_path);
    if let Some(error) = result.cleanup.detail.as_deref() {
        return Err(format!(
            "resource report was saved to {}, but managed browser cleanup was not verified: {error}",
            output_path.display()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_monitor(_options: MonitorOptions) -> Result<(), String> {
    let _ = (
        &_options.browser,
        _options.duration_seconds,
        _options.interval_ms,
        &_options.output_directory,
    );
    Err("resource monitoring currently supports macOS only".to_owned())
}

#[cfg(target_os = "macos")]
fn close_after_setup_error(
    manager: &velocity_browser::BrowserLifecycleManager<
        velocity_browser::macos::MacOSProcessDriver,
        velocity_browser::FileSessionStore,
    >,
    session_id: &str,
    message: &str,
) -> String {
    match manager.close(session_id) {
        Ok(_) => message.to_owned(),
        Err(error) => format!("{message}; additionally, managed session cleanup failed: {error}"),
    }
}

#[cfg(target_os = "macos")]
fn install_cancellation_handler() -> Result<(), String> {
    unsafe extern "C" fn cancel_on_signal(_signal: libc::c_int) {
        CANCELLED.store(true, Ordering::Relaxed);
    }
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::signal(signal, cancel_on_signal as *const () as libc::sighandler_t)
                == libc::SIG_ERR
            {
                return Err(format!(
                    "cannot install cleanup handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn sample_and_cleanup<C, ShouldCancel, Cleanup, CleanupResult>(
    mut monitor: ResourceMonitor<C>,
    duration: Duration,
    interval: Duration,
    mut should_cancel: ShouldCancel,
    cleanup: Cleanup,
) -> (ResourceReport, bool, CleanupResult)
where
    C: ResourceCollector,
    ShouldCancel: FnMut() -> bool,
    Cleanup: FnOnce() -> CleanupResult,
{
    const CANCELLATION_POLL: Duration = Duration::from_millis(50);
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline && !should_cancel() {
        let next_sample = (Instant::now() + interval).min(deadline);
        while Instant::now() < next_sample && !should_cancel() {
            thread::sleep(
                next_sample
                    .saturating_duration_since(Instant::now())
                    .min(CANCELLATION_POLL),
            );
        }
        if should_cancel() || monitor.sample().collection_error.is_some() {
            break;
        }
    }
    let cancelled = should_cancel();
    let report = monitor.stop();
    (report, cancelled, cleanup())
}

#[cfg(target_os = "macos")]
fn unique_run_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!(
        "{timestamp}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(target_os = "macos")]
fn browser_result(installation: &BrowserInstallation) -> BrowserResult {
    BrowserResult {
        id: installation.metadata.id.to_string(),
        installation_id: installation.installation_id.to_string(),
        name: installation.metadata.name.clone(),
        version: installation.version.clone(),
        application_path: installation.application_path.clone(),
        executable_path: installation.executable_path.clone(),
    }
}

#[cfg(target_os = "macos")]
fn write_result(
    output_directory: &Path,
    installation: &BrowserInstallation,
    run_id: &str,
    result: &MonitorResult,
) -> Result<PathBuf, String> {
    let file_name = format!("monitor-{}-{run_id}.json", installation.installation_id);
    let path = output_directory.join(file_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("cannot create resource report {}: {error}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, result)
        .map_err(|error| format!("cannot serialize resource report: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("cannot finish resource report {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn print_summary(result: &MonitorResult, output_path: &Path) {
    let statistics = &result.resource.statistics;
    println!("\nCPU");
    println!(
        "  Average:       {}",
        format_percent(statistics.cpu_mean_percent)
    );
    println!(
        "  Peak:          {}",
        format_percent(statistics.cpu_peak_percent)
    );
    println!(
        "  Minimum:       {}",
        format_percent(statistics.cpu_min_percent)
    );
    println!("\nMemory");
    println!(
        "  Footprint avg:  {}",
        format_bytes(statistics.physical_footprint_mean_bytes)
    );
    println!(
        "  Footprint min:  {}",
        format_bytes(
            statistics
                .physical_footprint_min_bytes
                .map(|bytes| bytes as f64)
        )
    );
    println!(
        "  Footprint peak: {}",
        format_bytes(
            statistics
                .physical_footprint_peak_bytes
                .map(|bytes| bytes as f64)
        )
    );
    println!(
        "  Footprint end:  {}",
        format_bytes(
            statistics
                .physical_footprint_final_bytes
                .map(|bytes| bytes as f64)
        )
    );
    println!(
        "  Resident avg:   {}",
        format_bytes(statistics.resident_mean_bytes)
    );
    println!(
        "  Resident min:   {}",
        format_bytes(statistics.resident_min_bytes.map(|bytes| bytes as f64))
    );
    println!(
        "  Resident peak:  {}",
        format_bytes(statistics.resident_peak_bytes.map(|bytes| bytes as f64))
    );
    println!(
        "  Resident end:   {}",
        format_bytes(statistics.resident_final_bytes.map(|bytes| bytes as f64))
    );
    println!("\nProcesses");
    println!("  Peak observed: {}", statistics.peak_process_count);
    println!("\nSamples");
    println!("  Collected:     {}", statistics.attempted_samples);
    println!("  With metrics:  {}", statistics.valid_samples);
    println!("  Failed:        {}", statistics.failed_samples);
    println!("  CPU valid:     {}", statistics.cpu_samples);
    println!(
        "  Footprint valid: {}",
        statistics.physical_footprint_samples
    );
    println!("  Resident valid:  {}", statistics.resident_samples);
    println!("  Coverage:      session-scoped; out-of-session helpers excluded");
    if result.monitoring.cancelled {
        println!("  Stopped by:    Ctrl-C");
    }
    println!("\nSaved: {}", output_path.display());
    if !statistics.warnings.is_empty() {
        println!("\nCoverage and collection notes:");
        for warning in &statistics.warnings {
            println!("  - {warning}");
        }
    }
}

#[cfg(target_os = "macos")]
fn format_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "N/A".to_owned(), |value| format!("{value:.1}%"))
}

#[cfg(target_os = "macos")]
fn format_bytes(value: Option<f64>) -> String {
    value.map_or_else(
        || "N/A".to_owned(),
        |value| format!("{:.1} MiB", value / 1_048_576.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    use std::{
        collections::VecDeque,
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
    };
    #[cfg(target_os = "macos")]
    use velocity_metrics::{
        ProcessResource, ProcessResourceSnapshot, ProcessStartTime, ResourceCollectionError,
    };

    #[cfg(target_os = "macos")]
    struct MockCollector {
        snapshots: VecDeque<ProcessResourceSnapshot>,
    }

    #[cfg(target_os = "macos")]
    impl ResourceCollector for MockCollector {
        fn collect(
            &mut self,
            _: &ManagedProcessTarget,
        ) -> Result<ProcessResourceSnapshot, ResourceCollectionError> {
            Ok(self
                .snapshots
                .pop_front()
                .unwrap_or_else(|| ProcessResourceSnapshot {
                    processes: vec![ProcessResource {
                        pid: 10,
                        start_time: ProcessStartTime {
                            seconds: 1,
                            microseconds: 1,
                        },
                        parent_pid: None,
                        parent_start_time: None,
                        executable_path: Some(PathBuf::from("/fixture/browser")),
                        process_group_id: Some(10),
                        process_session_id: Some(10),
                        attribution_evidence: Some(
                            velocity_metrics::AttributionEvidence::ManagedRootIdentity,
                        ),
                        role: None,
                        cpu_time_ns: Some(1),
                        physical_footprint_bytes: Some(100),
                        resident_bytes: Some(80),
                    }],
                    warnings: Vec::new(),
                    attribution: velocity_metrics::ProcessAttributionDiagnostics::default(),
                }))
        }
    }

    #[cfg(target_os = "macos")]
    fn test_monitor() -> ResourceMonitor<MockCollector> {
        ResourceMonitor::start(
            ManagedProcessTarget {
                root_pid: 10,
                session_id: 10,
                root_start_time: ProcessStartTime {
                    seconds: 1,
                    microseconds: 1,
                },
                executable_path: PathBuf::from("/fixture/browser"),
                application_path: PathBuf::from("/fixture/browser.app"),
                isolated_profile_path: None,
                allow_profile_argument_attribution: false,
            },
            ResourceConfig::default(),
            MockCollector {
                snapshots: VecDeque::new(),
            },
        )
        .unwrap()
    }

    #[test]
    fn monitor_arguments_use_documented_defaults() {
        let options = parse_options(["photon".to_owned()].into_iter()).unwrap();
        assert_eq!(options.duration_seconds, 10);
        assert_eq!(options.interval_ms, 100);
        assert!(options.output_directory.ends_with("tools/velocity/results"));
    }

    #[test]
    fn monitor_arguments_validate_duration_and_interval() {
        assert!(
            parse_options(
                ["photon".to_owned(), "--duration".to_owned(), "0".to_owned()].into_iter()
            )
            .is_err()
        );
        assert!(
            parse_options(
                ["photon".to_owned(), "--interval".to_owned(), "9".to_owned()].into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn monitor_arguments_bound_in_memory_sample_count() {
        let error = parse_options(
            [
                "photon".to_owned(),
                "--duration".to_owned(),
                "86400".to_owned(),
                "--interval".to_owned(),
                "10".to_owned(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(error.contains("below 100000"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn cancellation_stops_sampling_and_still_runs_managed_cleanup() {
        let cleaned_up = AtomicBool::new(false);
        let (report, cancelled, cleanup_result) = sample_and_cleanup(
            test_monitor(),
            Duration::from_secs(10),
            Duration::from_millis(100),
            || true,
            || {
                cleaned_up.store(true, Ordering::SeqCst);
                Ok::<(), &'static str>(())
            },
        );
        assert!(cancelled);
        assert!(report.samples.is_empty());
        assert_eq!(cleanup_result, Ok(()));
        assert!(cleaned_up.load(Ordering::SeqCst));
    }
}
