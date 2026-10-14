use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

use velocity_browser::macos::{MacOSProcessDriver, MacOSWindowObserver};
use velocity_browser::{
    BrowserEngine, BrowserInstallation, BrowserLifecycleManager, CloseOutcome, FileSessionStore,
    LifecycleError, ProcessLaunchTiming,
};
use velocity_core::{
    BenchmarkFailure, BenchmarkFailureKind, CancellationToken, StartupBenchmarkBackend,
    StartupIterationOutcome, StartupMode, StartupProgressEvent, StartupWindowProgressEvent,
    observe_startup_windows_with_progress,
};

pub struct MacOSStartupBackend {
    profile_root: PathBuf,
    session_store_path: PathBuf,
    profile_path: Option<PathBuf>,
    driver: Option<MacOSProcessDriver>,
    mode: StartupMode,
    profile_cleanup_allowed: bool,
    unsupported_reason: Option<String>,
}

impl MacOSStartupBackend {
    pub fn new(profile_root: PathBuf, session_store_path: PathBuf, mode: StartupMode) -> Self {
        Self {
            profile_root,
            session_store_path,
            profile_path: None,
            driver: None,
            mode,
            profile_cleanup_allowed: true,
            unsupported_reason: None,
        }
    }

    fn manager(
        &self,
    ) -> Result<BrowserLifecycleManager<MacOSProcessDriver, FileSessionStore>, String> {
        let driver = self
            .driver
            .clone()
            .ok_or_else(|| "startup profile was not prepared".to_owned())?;
        Ok(BrowserLifecycleManager::new(
            driver,
            FileSessionStore::new(self.session_store_path.clone()),
        ))
    }
}

impl StartupBenchmarkBackend for MacOSStartupBackend {
    fn prepare(
        &mut self,
        installation: &BrowserInstallation,
        mode: StartupMode,
        run_id: &str,
    ) -> Result<(), String> {
        self.mode = mode;
        self.unsupported_reason = None;
        let Some(engine) = installation.metadata.engine else {
            self.unsupported_reason = Some(format!(
                "{} has no known engine; Velocity cannot configure an isolated startup profile",
                installation.metadata.name
            ));
            return Ok(());
        };
        if !supports_isolated_profile(engine) {
            self.unsupported_reason = Some(format!(
                "{} has no isolated-profile option in the current macOS adapter; refusing to launch with its normal profile",
                installation.metadata.name
            ));
            return Ok(());
        }

        fs::create_dir_all(&self.profile_root).map_err(|error| {
            format!(
                "cannot create Velocity profile directory {}: {error}",
                self.profile_root.display()
            )
        })?;
        set_private_directory_permissions(&self.profile_root).map_err(|error| error.to_string())?;

        let profile_path = self
            .profile_root
            .join(format!("startup-{}-{run_id}", installation.installation_id));
        if fs::symlink_metadata(&profile_path).is_ok() {
            return Err(format!(
                "refusing to reuse an existing benchmark profile at {}",
                profile_path.display()
            ));
        }
        self.driver = Some(
            MacOSProcessDriver::new(self.profile_root.clone())
                .with_benchmark_profile(profile_path.clone(), mode == StartupMode::Warm),
        );
        self.profile_path = Some(profile_path);
        self.profile_cleanup_allowed = true;
        Ok(())
    }

    fn run_iteration(
        &mut self,
        installation: &BrowserInstallation,
        _mode: StartupMode,
        iteration: std::num::NonZeroUsize,
        timeout: Duration,
        cancellation: &CancellationToken,
        progress: &mut dyn FnMut(StartupProgressEvent),
    ) -> StartupIterationOutcome {
        if let Some(reason) = self.unsupported_reason.as_ref() {
            return StartupIterationOutcome::fail_all(
                BenchmarkFailureKind::ProfileIsolationUnavailable,
                reason.clone(),
            );
        }
        if cancellation.is_cancelled() {
            return StartupIterationOutcome::fail_all(
                BenchmarkFailureKind::Cancelled,
                "benchmark cancelled before browser launch",
            );
        }

        let manager = match self.manager() {
            Ok(manager) => manager,
            Err(message) => {
                return StartupIterationOutcome::fail_all(BenchmarkFailureKind::Setup, message);
            }
        };

        let (timing_sender, timing_receiver) = mpsc::channel::<ProcessLaunchTiming>();
        let (window_progress_sender, window_progress_receiver) =
            mpsc::channel::<StartupWindowProgressEvent>();
        let observer = MacOSWindowObserver;
        let monitor_cancellation = cancellation.clone();
        let monitor = thread::spawn(move || {
            let timing = timing_receiver.recv().ok()?;
            let mut send_progress = |event| {
                let _ = window_progress_sender.send(event);
            };
            Some(observe_startup_windows_with_progress(
                &timing,
                &observer,
                timeout,
                &monitor_cancellation,
                &mut send_progress,
            ))
        });

        let mut process_start_ms = None;
        progress(StartupProgressEvent::Launching {
            number: iteration.get(),
        });
        let mut on_started = |timing: ProcessLaunchTiming| {
            let elapsed_ms = timing
                .process_started_at
                .saturating_duration_since(timing.launch_started_at)
                .as_secs_f64()
                * 1_000.0;
            process_start_ms = Some(elapsed_ms);
            progress(StartupProgressEvent::ProcessStarted {
                number: iteration.get(),
                elapsed_ms,
            });
            let _ = timing_sender.send(timing);
        };
        let launch_result = manager.launch_observed(installation, None, &mut on_started);
        drop(on_started);
        drop(timing_sender);

        let mut window_progress = Vec::new();
        while !monitor.is_finished() {
            match window_progress_receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(event) => window_progress.push(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        window_progress.extend(window_progress_receiver.try_iter());
        let observation = monitor.join().ok().flatten();

        for event in window_progress {
            match event {
                StartupWindowProgressEvent::WindowVisible { elapsed_ms } => {
                    progress(StartupProgressEvent::WindowVisible {
                        number: iteration.get(),
                        elapsed_ms,
                    });
                }
                StartupWindowProgressEvent::WindowReady { elapsed_ms } => {
                    progress(StartupProgressEvent::WindowReady {
                        number: iteration.get(),
                        elapsed_ms,
                    });
                }
            }
        }

        let mut outcome = StartupIterationOutcome::default();
        if let Some(elapsed_ms) = process_start_ms {
            outcome
                .measurements_ms
                .insert(velocity_core::StartupMetric::ProcessStart, elapsed_ms);
        }
        if let Some(observation) = observation {
            merge_observation(&mut outcome, observation);
        } else {
            outcome.unavailable_metrics.insert(
                velocity_core::StartupMetric::WindowVisible,
                "browser process did not start, so no window could be observed".to_owned(),
            );
            outcome.unavailable_metrics.insert(
                velocity_core::StartupMetric::WindowReady,
                "browser process did not start, so readiness could not be observed".to_owned(),
            );
            if process_start_ms.is_some() {
                outcome.failures.push(BenchmarkFailure::new(
                    BenchmarkFailureKind::WindowObserver,
                    "window observer stopped before returning a result",
                ));
            }
        }

        progress(StartupProgressEvent::CleaningUp {
            number: iteration.get(),
        });
        match launch_result {
            Err(error) => {
                let message = lifecycle_error_message(error);
                let kind = classify_launch_failure(&message);
                if kind == BenchmarkFailureKind::OwnershipUncertain {
                    self.profile_cleanup_allowed = false;
                }
                set_window_unavailable(&mut outcome, &message);
                outcome.failures.push(BenchmarkFailure::new(kind, message));
            }
            Ok(session) => match manager.close(&session.session_id) {
                Ok(CloseOutcome::Closed(_) | CloseOutcome::AlreadyExited(_)) => {}
                Err(error) => {
                    self.profile_cleanup_allowed = false;
                    outcome.failures.push(BenchmarkFailure::new(
                        BenchmarkFailureKind::Cleanup,
                        lifecycle_error_message(error),
                    ));
                }
            },
        }

        for metric in [
            velocity_core::StartupMetric::ProcessStart,
            velocity_core::StartupMetric::WindowVisible,
            velocity_core::StartupMetric::WindowReady,
        ] {
            if !outcome.measurements_ms.contains_key(&metric)
                && !outcome.unavailable_metrics.contains_key(&metric)
            {
                outcome.unavailable_metrics.insert(
                    metric,
                    "measurement was unavailable for this iteration".to_owned(),
                );
            }
        }
        outcome
    }

    fn finish(&mut self) -> Result<(), String> {
        let Some(profile_path) = self.profile_path.take() else {
            return Ok(());
        };
        if !self.profile_cleanup_allowed {
            return Err(format!(
                "left benchmark profile at {} because process ownership or cleanup could not be verified",
                profile_path.display()
            ));
        }
        match fs::remove_dir_all(&profile_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "cannot remove benchmark profile {}: {error}",
                profile_path.display()
            )),
        }
    }

    fn profile_strategy(&self, installation: &BrowserInstallation) -> String {
        let strategy = match installation.metadata.engine {
            Some(BrowserEngine::Chromium) => "Chromium --user-data-dir",
            Some(BrowserEngine::Gecko) => "Firefox --profile with --no-remote",
            Some(BrowserEngine::Ladybird) => "Photon PHOTON_PROFILE_DIR with private HOME",
            Some(BrowserEngine::WebKit) => "unsupported: Safari profile isolation is unavailable",
            None => "unsupported: browser engine is unknown",
        };
        let policy = match self.mode {
            StartupMode::Cold => "fresh isolated profile for every measured launch",
            StartupMode::Warm => "same isolated profile reused across measured launches",
        };
        format!("{strategy}; {policy}")
    }
}

fn supports_isolated_profile(engine: BrowserEngine) -> bool {
    matches!(
        engine,
        BrowserEngine::Chromium | BrowserEngine::Gecko | BrowserEngine::Ladybird
    )
}

fn merge_observation(
    target: &mut StartupIterationOutcome,
    mut observation: StartupIterationOutcome,
) {
    target
        .measurements_ms
        .append(&mut observation.measurements_ms);
    target
        .unavailable_metrics
        .append(&mut observation.unavailable_metrics);
    target.failures.append(&mut observation.failures);
}

fn set_window_unavailable(outcome: &mut StartupIterationOutcome, reason: &str) {
    for metric in [
        velocity_core::StartupMetric::WindowVisible,
        velocity_core::StartupMetric::WindowReady,
    ] {
        if !outcome.measurements_ms.contains_key(&metric) {
            outcome
                .unavailable_metrics
                .entry(metric)
                .or_insert_with(|| reason.to_owned());
        }
    }
}

fn classify_launch_failure(message: &str) -> BenchmarkFailureKind {
    if message.contains("isolated startup profile") || message.contains("normal profile") {
        BenchmarkFailureKind::ProfileIsolationUnavailable
    } else if message.contains("ownership")
        || message.contains("refusing")
        || message.contains("could not verify")
    {
        BenchmarkFailureKind::OwnershipUncertain
    } else {
        BenchmarkFailureKind::Launch
    }
}

fn lifecycle_error_message(error: LifecycleError) -> String {
    error.to_string()
}

fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
