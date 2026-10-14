use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use velocity_browser::{
    BrowserInstallation, BrowserWindowObserver, ProcessLaunchTiming, WindowObservation,
};
use velocity_metrics::summarize_metric_values;

use crate::{
    BenchmarkCategory, BenchmarkConfig, BenchmarkConfigError, BenchmarkDefinition,
    BenchmarkExecutionError, BenchmarkExecutor, BenchmarkId, BenchmarkResult, BenchmarkRun,
    MetricDefinition, MetricUnit, MetricValue,
};

const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(25);
const READY_WINDOW_SAMPLES: usize = 2;
static RUN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Startup profile handling for measured iterations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    Cold,
    Warm,
}

impl std::fmt::Display for StartupMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cold => formatter.write_str("Cold"),
            Self::Warm => formatter.write_str("Warm"),
        }
    }
}

/// One measured startup timing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StartupMetric {
    #[serde(rename = "process_start_ms")]
    ProcessStart,
    #[serde(rename = "window_visible_ms")]
    WindowVisible,
    #[serde(rename = "window_ready_ms")]
    WindowReady,
}

impl StartupMetric {
    pub const ALL: [Self; 3] = [Self::ProcessStart, Self::WindowVisible, Self::WindowReady];

    pub const fn name(self) -> &'static str {
        match self {
            Self::ProcessStart => "process_start_ms",
            Self::WindowVisible => "window_visible_ms",
            Self::WindowReady => "window_ready_ms",
        }
    }

    pub fn definition(self) -> MetricDefinition {
        MetricDefinition {
            name: self.name().to_owned(),
            unit: MetricUnit::Milliseconds,
        }
    }
}

/// Validated startup benchmark options. The generic run count remains shared
/// with the core executor; warm-ups, timeout, and mode are startup-specific.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupBenchmarkConfig {
    pub runs: BenchmarkConfig,
    pub warmup_iterations: usize,
    pub mode: StartupMode,
    pub timeout: Duration,
}

impl StartupBenchmarkConfig {
    pub fn new(
        runs: usize,
        mode: StartupMode,
        timeout: Duration,
    ) -> Result<Self, StartupConfigError> {
        Self::new_with_warmups(runs, 0, mode, timeout)
    }

    pub fn new_with_warmups(
        runs: usize,
        warmup_iterations: usize,
        mode: StartupMode,
        timeout: Duration,
    ) -> Result<Self, StartupConfigError> {
        if timeout.is_zero() {
            return Err(StartupConfigError::ZeroTimeout);
        }
        if Instant::now().checked_add(timeout).is_none() {
            return Err(StartupConfigError::TimeoutTooLarge);
        }
        Ok(Self {
            runs: BenchmarkConfig::new(runs).map_err(StartupConfigError::Benchmark)?,
            warmup_iterations,
            mode,
            timeout,
        })
    }
}

impl Default for StartupBenchmarkConfig {
    fn default() -> Self {
        Self {
            runs: BenchmarkConfig::default(),
            warmup_iterations: 0,
            mode: StartupMode::Cold,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupConfigError {
    Benchmark(BenchmarkConfigError),
    ZeroTimeout,
    TimeoutTooLarge,
}

impl std::fmt::Display for StartupConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Benchmark(error) => error.fmt(formatter),
            Self::ZeroTimeout => formatter.write_str("startup timeout must be greater than zero"),
            Self::TimeoutTooLarge => formatter.write_str("startup timeout is too large"),
        }
    }
}

impl std::error::Error for StartupConfigError {}

/// Failure category stored with a per-iteration result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkFailureKind {
    Setup,
    Launch,
    ProcessExited,
    OwnershipUncertain,
    WindowTimeout,
    WindowObserver,
    ProfileIsolationUnavailable,
    Cancelled,
    Cleanup,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkFailure {
    pub kind: BenchmarkFailureKind,
    pub message: String,
}

/// Status of an unmeasured launch performed before the result iterations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarmupIterationResult {
    pub iteration: usize,
    pub ready: bool,
    pub failures: Vec<BenchmarkFailure>,
}

/// Shared comparison-run scheduling metadata copied into each browser report.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunOrderMetadata {
    pub comparison_run_id: Option<String>,
    pub randomized: bool,
    pub randomization_seed: Option<u64>,
    pub warmup_order_by_round: Vec<Vec<String>>,
    pub measured_order_by_round: Vec<Vec<String>>,
}

impl BenchmarkFailure {
    pub fn new(kind: BenchmarkFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UnavailableMetric {
    pub metric: StartupMetric,
    pub reason: String,
}

/// The measured values and explicit missing-metric reasons for one launch.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StartupIterationOutcome {
    pub measurements_ms: BTreeMap<StartupMetric, f64>,
    pub unavailable_metrics: BTreeMap<StartupMetric, String>,
    pub failures: Vec<BenchmarkFailure>,
}

impl StartupIterationOutcome {
    pub fn unavailable_all(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            measurements_ms: BTreeMap::new(),
            unavailable_metrics: StartupMetric::ALL
                .into_iter()
                .map(|metric| (metric, reason.clone()))
                .collect(),
            failures: Vec::new(),
        }
    }

    pub fn fail_all(kind: BenchmarkFailureKind, message: impl Into<String>) -> Self {
        let message = message.into();
        let mut outcome = Self::unavailable_all(message.clone());
        outcome.failures.push(BenchmarkFailure::new(kind, message));
        outcome
    }
}

/// Progress events emitted by a sequential startup run.
#[derive(Clone, Debug, PartialEq)]
pub enum StartupProgressEvent {
    WarmupStarting {
        number: usize,
        total: usize,
    },
    WarmupFinished {
        number: usize,
        total: usize,
        ready: bool,
        reason: Option<String>,
    },
    IterationStarting {
        number: usize,
        total: usize,
    },
    Launching {
        number: usize,
    },
    ProcessStarted {
        number: usize,
        elapsed_ms: f64,
    },
    WindowVisible {
        number: usize,
        elapsed_ms: f64,
    },
    WindowReady {
        number: usize,
        elapsed_ms: f64,
    },
    MetricUnavailable {
        number: usize,
        metric: StartupMetric,
        reason: String,
    },
    CleaningUp {
        number: usize,
    },
    IterationFinished {
        number: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum StartupWindowProgressEvent {
    WindowVisible { elapsed_ms: f64 },
    WindowReady { elapsed_ms: f64 },
}

/// Backend contract for one sequential startup benchmark target.
pub trait StartupBenchmarkBackend {
    fn prepare(
        &mut self,
        installation: &BrowserInstallation,
        mode: StartupMode,
        run_id: &str,
    ) -> Result<(), String>;

    fn run_iteration(
        &mut self,
        installation: &BrowserInstallation,
        mode: StartupMode,
        iteration: NonZeroUsize,
        timeout: Duration,
        cancellation: &CancellationToken,
        progress: &mut dyn FnMut(StartupProgressEvent),
    ) -> StartupIterationOutcome;

    fn finish(&mut self) -> Result<(), String>;

    fn profile_strategy(&self, installation: &BrowserInstallation) -> String;
}

/// Executes a startup benchmark for one already-resolved browser installation.
pub struct StartupBenchmarkExecutor<B> {
    installation: BrowserInstallation,
    config: StartupBenchmarkConfig,
    backend: B,
    cancellation: CancellationToken,
    progress: Box<dyn FnMut(StartupProgressEvent)>,
    prepared: Option<Result<(), String>>,
    current_run: Option<BenchmarkRun>,
    finished: bool,
}

impl<B> StartupBenchmarkExecutor<B> {
    pub fn new(
        installation: BrowserInstallation,
        config: StartupBenchmarkConfig,
        backend: B,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            installation,
            config,
            backend,
            cancellation,
            progress: Box::new(|_| {}),
            prepared: None,
            current_run: None,
            finished: false,
        }
    }

    pub fn with_progress(mut self, progress: impl FnMut(StartupProgressEvent) + 'static) -> Self {
        self.progress = Box::new(progress);
        self
    }

    pub fn profile_strategy(&self) -> String
    where
        B: StartupBenchmarkBackend,
    {
        self.backend.profile_strategy(&self.installation)
    }

    pub fn installation(&self) -> &BrowserInstallation {
        &self.installation
    }

    pub fn config(&self) -> StartupBenchmarkConfig {
        self.config
    }

    /// Prepares one browser session so callers can schedule individual
    /// warm-up and measured iterations across several browsers in rounds.
    pub fn prepare_for(&mut self, run: &BenchmarkRun) -> Result<(), BenchmarkExecutionError>
    where
        B: StartupBenchmarkBackend,
    {
        self.validate_run(run)?;
        if self.prepared.is_some() {
            return Err(BenchmarkExecutionError {
                message: "startup executor has already been prepared".to_owned(),
            });
        }
        self.prepared = Some(self.backend.prepare(
            &self.installation,
            self.config.mode,
            &unique_run_id(),
        ));
        self.current_run = Some(run.clone());
        Ok(())
    }

    /// Runs one unmeasured warm-up launch. Only readiness and failure status
    /// are returned; collected timings are deliberately discarded.
    pub fn run_warmup_iteration(&mut self, iteration: NonZeroUsize) -> WarmupIterationResult
    where
        B: StartupBenchmarkBackend,
    {
        let total = self.config.warmup_iterations;
        (self.progress)(StartupProgressEvent::WarmupStarting {
            number: iteration.get(),
            total,
        });
        let mut progress = std::mem::replace(&mut self.progress, Box::new(|_| {}));
        let mut warmup_progress = |event| match event {
            StartupProgressEvent::Launching { .. } | StartupProgressEvent::CleaningUp { .. } => {
                progress(event)
            }
            _ => {}
        };
        let outcome = self.run_backend_iteration(iteration, &mut warmup_progress);
        drop(warmup_progress);
        self.progress = progress;
        let ready = outcome
            .measurements_ms
            .contains_key(&StartupMetric::WindowReady)
            && outcome.failures.is_empty();
        let mut failures = outcome.failures;
        if !ready && failures.is_empty() {
            failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::WindowObserver,
                "warm-up did not reach the browser window readiness condition",
            ));
        }
        let reason = failures.first().map(|failure| failure.message.clone());
        (self.progress)(StartupProgressEvent::WarmupFinished {
            number: iteration.get(),
            total,
            ready,
            reason,
        });
        WarmupIterationResult {
            iteration: iteration.get(),
            ready,
            failures,
        }
    }

    /// Runs one measured iteration and retains its raw measurements.
    pub fn run_measured_iteration(&mut self, iteration: NonZeroUsize) -> BenchmarkResult
    where
        B: StartupBenchmarkBackend,
    {
        let total = self.config.runs.iterations();
        (self.progress)(StartupProgressEvent::IterationStarting {
            number: iteration.get(),
            total,
        });
        let mut progress = std::mem::replace(&mut self.progress, Box::new(|_| {}));
        let outcome = self.run_backend_iteration(iteration, &mut progress);
        self.progress = progress;
        let cancelled = outcome
            .failures
            .iter()
            .any(|failure| failure.kind == BenchmarkFailureKind::Cancelled);
        for (metric, reason) in &outcome.unavailable_metrics {
            (self.progress)(StartupProgressEvent::MetricUnavailable {
                number: iteration.get(),
                metric: *metric,
                reason: reason.clone(),
            });
        }
        let result = self.current_run.as_ref().map_or_else(
            || setup_failure_result(&self.installation, iteration, "executor was not prepared"),
            |run| benchmark_result(run, iteration, outcome),
        );
        (self.progress)(StartupProgressEvent::IterationFinished {
            number: iteration.get(),
        });
        if cancelled {
            self.cancellation.cancel();
        }
        result
    }

    /// Releases backend resources once all browser rounds have completed.
    pub fn finish(&mut self) -> Result<(), String>
    where
        B: StartupBenchmarkBackend,
    {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        self.backend.finish()
    }

    fn validate_run(&self, run: &BenchmarkRun) -> Result<(), BenchmarkExecutionError> {
        if run.definition.category != BenchmarkCategory::Startup || run.definition.id.0 != "startup"
        {
            return Err(BenchmarkExecutionError {
                message: "startup executor only accepts the 'startup' benchmark".to_owned(),
            });
        }
        if run.browser_id != self.installation.metadata.id {
            return Err(BenchmarkExecutionError {
                message: format!(
                    "benchmark browser '{}' does not match selected installation '{}'",
                    run.browser_id, self.installation.metadata.id
                ),
            });
        }
        if run.config.iterations() != self.config.runs.iterations() {
            return Err(BenchmarkExecutionError {
                message: "startup configuration run count does not match the benchmark request"
                    .to_owned(),
            });
        }
        Ok(())
    }

    fn run_backend_iteration(
        &mut self,
        iteration: NonZeroUsize,
        progress: &mut dyn FnMut(StartupProgressEvent),
    ) -> StartupIterationOutcome
    where
        B: StartupBenchmarkBackend,
    {
        if self.finished {
            return StartupIterationOutcome::fail_all(
                BenchmarkFailureKind::Setup,
                "startup executor has already finished",
            );
        }
        if self.cancellation.is_cancelled() {
            return StartupIterationOutcome::fail_all(
                BenchmarkFailureKind::Cancelled,
                "benchmark cancelled before this iteration started",
            );
        }
        match self.prepared.as_ref() {
            None => StartupIterationOutcome::fail_all(
                BenchmarkFailureKind::Setup,
                "startup executor has not been prepared",
            ),
            Some(Err(message)) => {
                StartupIterationOutcome::fail_all(BenchmarkFailureKind::Setup, message.clone())
            }
            Some(Ok(())) => self.backend.run_iteration(
                &self.installation,
                self.config.mode,
                iteration,
                self.config.timeout,
                &self.cancellation,
                progress,
            ),
        }
    }
}

impl<B: StartupBenchmarkBackend> BenchmarkExecutor for StartupBenchmarkExecutor<B> {
    fn execute(
        &mut self,
        run: &BenchmarkRun,
    ) -> Result<Vec<BenchmarkResult>, BenchmarkExecutionError> {
        self.prepare_for(run)?;
        for number in 1..=self.config.warmup_iterations {
            self.run_warmup_iteration(NonZeroUsize::new(number).expect("warm-up is non-zero"));
            if self.cancellation.is_cancelled() {
                break;
            }
        }
        let total = run.config.iterations();
        let mut results = Vec::with_capacity(total);
        for number in 1..=total {
            let iteration = NonZeroUsize::new(number).expect("iteration is non-zero");
            let result = self.run_measured_iteration(iteration);
            let cancelled = result
                .failures
                .iter()
                .any(|failure| failure.kind == BenchmarkFailureKind::Cancelled);
            results.push(result);
            if cancelled {
                break;
            }
        }
        if let Err(message) = self.finish()
            && let Some(last) = results.last_mut()
        {
            last.failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::Cleanup,
                message,
            ));
        }

        Ok(results)
    }
}

fn benchmark_result(
    run: &BenchmarkRun,
    iteration: NonZeroUsize,
    mut outcome: StartupIterationOutcome,
) -> BenchmarkResult {
    let mut metrics = Vec::new();
    let mut unavailable_metrics = Vec::new();
    for metric in StartupMetric::ALL {
        match outcome.measurements_ms.remove(&metric) {
            Some(value) if value.is_finite() && value >= 0.0 => metrics.push(MetricValue {
                definition: metric.definition(),
                value,
            }),
            Some(_) => {
                unavailable_metrics.push(UnavailableMetric {
                    metric,
                    reason: "backend returned a non-finite or negative timing".to_owned(),
                });
            }
            None => unavailable_metrics.push(UnavailableMetric {
                metric,
                reason: outcome
                    .unavailable_metrics
                    .remove(&metric)
                    .unwrap_or_else(|| "measurement was not collected".to_owned()),
            }),
        }
    }

    BenchmarkResult {
        benchmark_id: run.definition.id.clone(),
        browser_id: run.browser_id.clone(),
        iteration,
        metrics,
        unavailable_metrics,
        failures: outcome.failures,
    }
}

fn setup_failure_result(
    installation: &BrowserInstallation,
    iteration: NonZeroUsize,
    message: &str,
) -> BenchmarkResult {
    BenchmarkResult {
        benchmark_id: BenchmarkId("startup".to_owned()),
        browser_id: installation.metadata.id.clone(),
        iteration,
        metrics: Vec::new(),
        unavailable_metrics: StartupMetric::ALL
            .into_iter()
            .map(|metric| UnavailableMetric {
                metric,
                reason: message.to_owned(),
            })
            .collect(),
        failures: vec![BenchmarkFailure::new(BenchmarkFailureKind::Setup, message)],
    }
}

/// Cooperative cancellation shared by the CLI, iteration runner, and window observer.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Times first visibility and a stable, verified window across observer samples.
pub fn observe_startup_windows<O: BrowserWindowObserver + ?Sized>(
    timing: &ProcessLaunchTiming,
    observer: &O,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> StartupIterationOutcome {
    observe_startup_windows_with_progress(timing, observer, timeout, cancellation, &mut |_| {})
}

pub fn observe_startup_windows_with_progress<O: BrowserWindowObserver + ?Sized>(
    timing: &ProcessLaunchTiming,
    observer: &O,
    timeout: Duration,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(StartupWindowProgressEvent),
) -> StartupIterationOutcome {
    let mut outcome = StartupIterationOutcome::default();
    let Some(deadline) = timing.launch_started_at.checked_add(timeout) else {
        let mut outcome = StartupIterationOutcome::fail_all(
            BenchmarkFailureKind::Setup,
            "startup timeout exceeds the monotonic clock range",
        );
        for failure in &mut outcome.failures {
            failure.kind = BenchmarkFailureKind::Setup;
        }
        return outcome;
    };
    let mut previous_windows = BTreeSet::new();
    let mut stable_samples = 0;

    loop {
        if cancellation.is_cancelled() {
            outcome.unavailable_metrics.insert(
                StartupMetric::WindowVisible,
                "benchmark cancelled before a visible window was observed".to_owned(),
            );
            outcome.unavailable_metrics.insert(
                StartupMetric::WindowReady,
                "benchmark cancelled before the readiness condition was met".to_owned(),
            );
            outcome.failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::Cancelled,
                "benchmark cancelled while waiting for a browser window",
            ));
            return outcome;
        }

        let now = Instant::now();
        if now >= deadline {
            let reason = if outcome
                .measurements_ms
                .contains_key(&StartupMetric::WindowVisible)
            {
                format!(
                    "visible browser window did not remain stable for two consecutive samples within {} ms",
                    timeout.as_millis()
                )
            } else {
                format!(
                    "no verified visible browser window was observed within {} ms",
                    timeout.as_millis()
                )
            };
            if !outcome
                .measurements_ms
                .contains_key(&StartupMetric::WindowVisible)
            {
                outcome
                    .unavailable_metrics
                    .insert(StartupMetric::WindowVisible, reason.clone());
            }
            outcome
                .unavailable_metrics
                .insert(StartupMetric::WindowReady, reason.clone());
            outcome.failures.push(BenchmarkFailure::new(
                BenchmarkFailureKind::WindowTimeout,
                reason,
            ));
            return outcome;
        }

        match observer.observe(timing.pid, timing.session_id, &timing.executable_path) {
            Ok(WindowObservation::ProcessExited) => {
                mark_window_metrics_unavailable(&mut outcome, "managed browser process exited");
                outcome.failures.push(BenchmarkFailure::new(
                    BenchmarkFailureKind::ProcessExited,
                    "managed browser process exited before its window became ready",
                ));
                return outcome;
            }
            Ok(WindowObservation::OwnershipUncertain) => {
                mark_window_metrics_unavailable(
                    &mut outcome,
                    "window ownership could not be verified against the managed process",
                );
                outcome.failures.push(BenchmarkFailure::new(
                    BenchmarkFailureKind::OwnershipUncertain,
                    "refusing to attribute an unverified window to this browser launch",
                ));
                return outcome;
            }
            Ok(WindowObservation::VisibleWindows(windows)) => {
                let observed_at = Instant::now();
                if !windows.is_empty()
                    && let std::collections::btree_map::Entry::Vacant(entry) =
                        outcome.measurements_ms.entry(StartupMetric::WindowVisible)
                {
                    let elapsed_ms = elapsed_ms(timing.launch_started_at, observed_at);
                    entry.insert(elapsed_ms);
                    progress(StartupWindowProgressEvent::WindowVisible { elapsed_ms });
                }
                let current_windows = windows.into_iter().collect::<BTreeSet<_>>();
                if current_windows.is_empty() {
                    previous_windows.clear();
                    stable_samples = 0;
                } else if !previous_windows.is_disjoint(&current_windows) {
                    stable_samples += 1;
                    if stable_samples >= READY_WINDOW_SAMPLES {
                        let elapsed_ms = elapsed_ms(timing.launch_started_at, observed_at);
                        outcome
                            .measurements_ms
                            .insert(StartupMetric::WindowReady, elapsed_ms);
                        progress(StartupWindowProgressEvent::WindowReady { elapsed_ms });
                        return outcome;
                    }
                    previous_windows = current_windows;
                } else {
                    stable_samples = 1;
                    previous_windows = current_windows;
                }
            }
            Err(error) => {
                mark_window_metrics_unavailable(
                    &mut outcome,
                    format!("macOS window observation failed: {error}"),
                );
                outcome.failures.push(BenchmarkFailure::new(
                    BenchmarkFailureKind::WindowObserver,
                    error.to_string(),
                ));
                return outcome;
            }
        }

        std::thread::sleep(
            WINDOW_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn mark_window_metrics_unavailable(
    outcome: &mut StartupIterationOutcome,
    reason: impl Into<String>,
) {
    let reason = reason.into();
    for metric in [StartupMetric::WindowVisible, StartupMetric::WindowReady] {
        if !outcome.measurements_ms.contains_key(&metric) {
            outcome.unavailable_metrics.insert(metric, reason.clone());
        }
    }
}

fn elapsed_ms(start: Instant, end: Instant) -> f64 {
    end.saturating_duration_since(start).as_secs_f64() * 1_000.0
}

/// Summary statistics for one metric. Standard deviation is the sample standard
/// deviation and is unavailable for a single observation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MetricSummary {
    pub sample_count: usize,
    pub median_ms: f64,
    pub mean_ms: f64,
    pub minimum_ms: f64,
    pub maximum_ms: f64,
    pub standard_deviation_ms: Option<f64>,
    #[serde(default)]
    pub coefficient_of_variation_percent: Option<f64>,
}

pub fn summarize_results(
    results: &[BenchmarkResult],
) -> BTreeMap<StartupMetric, Option<MetricSummary>> {
    StartupMetric::ALL
        .into_iter()
        .map(|metric| {
            let mut values = results
                .iter()
                .flat_map(|result| &result.metrics)
                .filter(|value| {
                    value.definition.name == metric.name()
                        && value.definition.unit == MetricUnit::Milliseconds
                        && value.value.is_finite()
                        && value.value >= 0.0
                })
                .map(|value| value.value)
                .collect::<Vec<_>>();
            (metric, summarize_values(&mut values))
        })
        .collect()
}

fn summarize_values(values: &mut [f64]) -> Option<MetricSummary> {
    let statistics = summarize_metric_values(values.iter().copied())?;
    Some(MetricSummary {
        sample_count: statistics.sample_count,
        median_ms: statistics.median,
        mean_ms: statistics.mean,
        minimum_ms: statistics.minimum,
        maximum_ms: statistics.maximum,
        standard_deviation_ms: statistics.sample_standard_deviation,
        coefficient_of_variation_percent: statistics.coefficient_of_variation_percent,
    })
}

/// Browser identity embedded in a startup JSON report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportBrowser {
    pub id: String,
    pub installation_id: String,
    pub name: String,
    pub version: Option<String>,
    pub executable_path: PathBuf,
}

impl From<&BrowserInstallation> for ReportBrowser {
    fn from(installation: &BrowserInstallation) -> Self {
        Self {
            id: installation.metadata.id.to_string(),
            installation_id: installation.installation_id.to_string(),
            name: installation.metadata.name.clone(),
            version: installation.version.clone(),
            executable_path: installation.executable_path.clone(),
        }
    }
}

/// Serializable report written after a startup benchmark.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StartupBenchmarkReport {
    pub schema_version: u32,
    pub benchmark: String,
    pub browser: ReportBrowser,
    pub timestamp_unix_ms: u128,
    pub operating_system: String,
    pub architecture: String,
    pub mode: StartupMode,
    pub requested_iterations: usize,
    #[serde(default)]
    pub requested_warmup_iterations: usize,
    pub startup_timeout_ms: u64,
    pub profile_isolated: bool,
    pub profile_strategy: String,
    pub os_file_cache_cold: bool,
    pub window_ready_condition: String,
    pub iterations: Vec<BenchmarkResult>,
    #[serde(default)]
    pub warmup_iterations: Vec<WarmupIterationResult>,
    #[serde(default)]
    pub run_order: RunOrderMetadata,
    #[serde(default)]
    pub run_failures: Vec<BenchmarkFailure>,
    pub summary: BTreeMap<StartupMetric, Option<MetricSummary>>,
    pub limitations: Vec<String>,
    pub cancelled: bool,
}

impl StartupBenchmarkReport {
    pub fn new(
        installation: &BrowserInstallation,
        config: StartupBenchmarkConfig,
        profile_isolated: bool,
        profile_strategy: String,
        iterations: Vec<BenchmarkResult>,
    ) -> Self {
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let cancelled = iterations.iter().any(|iteration| {
            iteration
                .failures
                .iter()
                .any(|failure| failure.kind == BenchmarkFailureKind::Cancelled)
        });
        let mut limitations = vec![
            "Cold mode creates a fresh isolated profile for each measured launch; operating-system file caches are not cleared.".to_owned(),
            "Window readiness means the same verified, on-screen application window was observed in two consecutive 25 ms samples; it does not measure page load or content readiness.".to_owned(),
            "Background activity, thermal state, power management, and operating-system caches are not controlled.".to_owned(),
        ];
        if config.warmup_iterations == 0 {
            limitations.push("No unmeasured warm-up launches were performed.".to_owned());
        } else {
            limitations.push(format!(
                "{} unmeasured warm-up launch(es) were requested per browser; warm-up timing values are discarded.",
                config.warmup_iterations
            ));
        }
        if !profile_isolated {
            limitations.push(
                "This browser does not support an isolated profile through the current adapter; no launch was attempted.".to_owned(),
            );
        } else {
            limitations.push(
                "Profile isolation uses browser-specific launch flags or environment variables; these setup paths are recorded for comparison.".to_owned(),
            );
        }
        Self {
            schema_version: 2,
            benchmark: "startup".to_owned(),
            browser: ReportBrowser::from(installation),
            timestamp_unix_ms,
            operating_system: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            mode: config.mode,
            requested_iterations: config.runs.iterations(),
            requested_warmup_iterations: config.warmup_iterations,
            startup_timeout_ms: duration_millis(config.timeout),
            profile_isolated,
            profile_strategy,
            os_file_cache_cold: false,
            window_ready_condition:
                "same verified on-screen window ID observed in two consecutive 25 ms samples"
                    .to_owned(),
            summary: summarize_results(&iterations),
            iterations,
            warmup_iterations: Vec::new(),
            run_order: RunOrderMetadata::default(),
            run_failures: Vec::new(),
            limitations,
            cancelled,
        }
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

/// Persists a report with a unique ID component suitable for result filenames.
pub fn unique_run_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{timestamp}-{counter}", std::process::id())
}

/// Creates the standard definition used by the CLI.
pub fn startup_benchmark_definition() -> BenchmarkDefinition {
    BenchmarkDefinition {
        id: BenchmarkId("startup".to_owned()),
        name: "Startup Benchmark".to_owned(),
        description: "Measures process creation and verified visible-window readiness.".to_owned(),
        category: BenchmarkCategory::Startup,
    }
}

/// Tracks a window sample for a test or platform observer implementation.
pub fn update_window_tracker(
    previous_windows: &mut BTreeSet<u32>,
    stable_samples: &mut usize,
    current_windows: impl IntoIterator<Item = u32>,
) -> bool {
    let current = current_windows.into_iter().collect::<BTreeSet<_>>();
    if current.is_empty() {
        previous_windows.clear();
        *stable_samples = 0;
        false
    } else if previous_windows.is_disjoint(&current) {
        *stable_samples = 1;
        *previous_windows = current;
        false
    } else {
        *stable_samples += 1;
        *previous_windows = current;
        *stable_samples >= READY_WINDOW_SAMPLES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use velocity_browser::{
        BrowserEngine, BrowserId, BrowserMetadata, InstallationId, WindowObservationError,
    };

    fn fixture_installation() -> BrowserInstallation {
        BrowserInstallation {
            installation_id: InstallationId::new("chrome-fixture").unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new("chrome").unwrap(),
                name: "Fixture Chrome".to_owned(),
                bundle_identifier: Some("com.google.Chrome".to_owned()),
                engine: Some(BrowserEngine::Chromium),
            },
            version: Some("1.0".to_owned()),
            application_path: PathBuf::from("/fixture/Chrome.app"),
            executable_path: PathBuf::from("/fixture/Chrome"),
        }
    }

    #[derive(Clone, Default)]
    struct MockBackend {
        calls: Arc<Mutex<Vec<usize>>>,
        finish_calls: Arc<Mutex<usize>>,
        fail_iteration: Option<usize>,
    }

    impl StartupBenchmarkBackend for MockBackend {
        fn prepare(
            &mut self,
            _installation: &BrowserInstallation,
            _mode: StartupMode,
            _run_id: &str,
        ) -> Result<(), String> {
            Ok(())
        }

        fn run_iteration(
            &mut self,
            _installation: &BrowserInstallation,
            _mode: StartupMode,
            iteration: NonZeroUsize,
            _timeout: Duration,
            cancellation: &CancellationToken,
            progress: &mut dyn FnMut(StartupProgressEvent),
        ) -> StartupIterationOutcome {
            self.calls.lock().unwrap().push(iteration.get());
            progress(StartupProgressEvent::ProcessStarted {
                number: iteration.get(),
                elapsed_ms: 2.0,
            });
            if cancellation.is_cancelled() {
                return StartupIterationOutcome::fail_all(
                    BenchmarkFailureKind::Cancelled,
                    "cancelled",
                );
            }
            if self.fail_iteration == Some(iteration.get()) {
                return StartupIterationOutcome::fail_all(
                    BenchmarkFailureKind::Launch,
                    "fixture launch failed",
                );
            }
            StartupIterationOutcome {
                measurements_ms: BTreeMap::from([(
                    StartupMetric::ProcessStart,
                    2.0 + iteration.get() as f64,
                )]),
                unavailable_metrics: BTreeMap::from([
                    (
                        StartupMetric::WindowVisible,
                        "fixture had no window".to_owned(),
                    ),
                    (
                        StartupMetric::WindowReady,
                        "fixture had no window".to_owned(),
                    ),
                ]),
                failures: Vec::new(),
            }
        }

        fn finish(&mut self) -> Result<(), String> {
            *self.finish_calls.lock().unwrap() += 1;
            Ok(())
        }

        fn profile_strategy(&self, _installation: &BrowserInstallation) -> String {
            "fixture isolated profile".to_owned()
        }
    }

    fn benchmark_run(runs: usize) -> BenchmarkRun {
        BenchmarkRun {
            definition: startup_benchmark_definition(),
            browser_id: BrowserId::new("chrome").unwrap(),
            config: BenchmarkConfig::new(runs).unwrap(),
        }
    }

    #[test]
    fn startup_configuration_rejects_zero_runs_and_timeout() {
        assert_eq!(
            StartupBenchmarkConfig::new(0, StartupMode::Cold, Duration::from_secs(1)),
            Err(StartupConfigError::Benchmark(
                BenchmarkConfigError::ZeroIterations
            ))
        );
        assert_eq!(
            StartupBenchmarkConfig::new(1, StartupMode::Cold, Duration::ZERO),
            Err(StartupConfigError::ZeroTimeout)
        );
        assert_eq!(StartupBenchmarkConfig::default().runs.iterations(), 5);
    }

    #[test]
    fn startup_executor_runs_sequentially_and_finishes_after_iteration_failures() {
        let backend = MockBackend {
            fail_iteration: Some(2),
            ..MockBackend::default()
        };
        let calls = backend.calls.clone();
        let finish_calls = backend.finish_calls.clone();
        let mut executor = StartupBenchmarkExecutor::new(
            fixture_installation(),
            StartupBenchmarkConfig::new(3, StartupMode::Cold, Duration::from_secs(1)).unwrap(),
            backend,
            CancellationToken::default(),
        );
        let results = executor.execute(&benchmark_run(3)).unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(*finish_calls.lock().unwrap(), 1);
        assert_eq!(results.len(), 3);
        assert_eq!(results[1].failures[0].kind, BenchmarkFailureKind::Launch);
        assert!(results[1].metrics.is_empty());
        assert_eq!(
            results[1].unavailable_metrics.len(),
            StartupMetric::ALL.len()
        );
    }

    #[test]
    fn startup_cancellation_stops_later_iterations_and_still_finishes() {
        let token = CancellationToken::default();
        token.cancel();
        let backend = MockBackend::default();
        let calls = backend.calls.clone();
        let finish_calls = backend.finish_calls.clone();
        let mut executor = StartupBenchmarkExecutor::new(
            fixture_installation(),
            StartupBenchmarkConfig::default(),
            backend,
            token,
        );
        let results = executor.execute(&benchmark_run(5)).unwrap();
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(*finish_calls.lock().unwrap(), 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].failures[0].kind, BenchmarkFailureKind::Cancelled);
    }

    #[test]
    fn summaries_exclude_failed_and_missing_measurements_and_use_sample_deviation() {
        let mut results = Vec::new();
        for index in 0..3 {
            results.push(BenchmarkResult {
                benchmark_id: BenchmarkId("startup".to_owned()),
                browser_id: BrowserId::new("chrome").unwrap(),
                iteration: NonZeroUsize::new(index + 1).unwrap(),
                metrics: if index == 1 {
                    Vec::new()
                } else {
                    vec![MetricValue {
                        definition: StartupMetric::WindowReady.definition(),
                        value: if index == 0 { 10.0 } else { 30.0 },
                    }]
                },
                unavailable_metrics: if index == 1 {
                    vec![UnavailableMetric {
                        metric: StartupMetric::WindowReady,
                        reason: "window timed out".to_owned(),
                    }]
                } else {
                    Vec::new()
                },
                failures: if index == 1 {
                    vec![BenchmarkFailure::new(
                        BenchmarkFailureKind::WindowTimeout,
                        "window timed out after visible sample",
                    )]
                } else {
                    Vec::new()
                },
            });
        }
        let summary = summarize_results(&results)[&StartupMetric::WindowReady].unwrap();
        assert_eq!(summary.sample_count, 2);
        assert_eq!(summary.median_ms, 20.0);
        assert_eq!(summary.mean_ms, 20.0);
        assert_eq!(summary.minimum_ms, 10.0);
        assert_eq!(summary.maximum_ms, 30.0);
        assert!((summary.standard_deviation_ms.unwrap() - 200.0_f64.sqrt()).abs() < 1e-10);
        assert_eq!(summarize_results(&[])[&StartupMetric::WindowReady], None);
    }

    #[test]
    fn startup_results_serialize_missing_metrics_without_zero_substitution() {
        let result = BenchmarkResult {
            benchmark_id: BenchmarkId("startup".to_owned()),
            browser_id: BrowserId::new("chrome").unwrap(),
            iteration: NonZeroUsize::new(1).unwrap(),
            metrics: Vec::new(),
            unavailable_metrics: vec![UnavailableMetric {
                metric: StartupMetric::WindowReady,
                reason: "window metadata unavailable".to_owned(),
            }],
            failures: Vec::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("window_ready"));
        assert!(json.contains("window metadata unavailable"));
        assert!(!json.contains("0.0"));

        let report = StartupBenchmarkReport::new(
            &fixture_installation(),
            StartupBenchmarkConfig::default(),
            true,
            "fixture isolated profile".to_owned(),
            vec![result],
        );
        let report_json = serde_json::to_string(&report).unwrap();
        assert!(report_json.contains("process_start_ms"));
        assert!(report_json.contains("window metadata unavailable"));
        assert!(report_json.contains("operating_system"));
        assert!(report_json.contains("os_file_cache_cold"));
    }

    #[test]
    fn window_tracker_requires_a_same_window_in_two_samples() {
        let mut previous = BTreeSet::new();
        let mut stable_samples = 0;
        assert!(!update_window_tracker(
            &mut previous,
            &mut stable_samples,
            [10, 11]
        ));
        assert!(!update_window_tracker(
            &mut previous,
            &mut stable_samples,
            [12]
        ));
        assert!(update_window_tracker(
            &mut previous,
            &mut stable_samples,
            [12, 13]
        ));
        assert!(!update_window_tracker(
            &mut previous,
            &mut stable_samples,
            []
        ));
    }

    #[derive(Default)]
    struct MockWindowObserver(Mutex<VecDeque<WindowObservation>>);

    impl BrowserWindowObserver for MockWindowObserver {
        fn observe(
            &self,
            _process_id: u32,
            _process_session_id: u32,
            _executable_path: &std::path::Path,
        ) -> Result<WindowObservation, WindowObservationError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(WindowObservation::VisibleWindows(vec![1])))
        }
    }

    #[test]
    fn window_observer_stable_window_produces_visible_and_ready_timings() {
        let now = Instant::now();
        let timing = ProcessLaunchTiming {
            pid: 1,
            session_id: 1,
            executable_path: PathBuf::from("/fixture/browser"),
            launch_started_at: now,
            process_started_at: now,
        };
        let observer = MockWindowObserver(Mutex::new(VecDeque::from([
            WindowObservation::VisibleWindows(vec![5]),
            WindowObservation::VisibleWindows(vec![5]),
        ])));
        let outcome = observe_startup_windows(
            &timing,
            &observer,
            Duration::from_secs(1),
            &CancellationToken::default(),
        );
        assert!(
            outcome
                .measurements_ms
                .contains_key(&StartupMetric::WindowVisible)
        );
        assert!(
            outcome
                .measurements_ms
                .contains_key(&StartupMetric::WindowReady)
        );
    }

    #[test]
    fn startup_window_timeout_is_a_failure_with_missing_metrics() {
        let now = Instant::now();
        let timing = ProcessLaunchTiming {
            pid: 1,
            session_id: 1,
            executable_path: PathBuf::from("/fixture/browser"),
            launch_started_at: now,
            process_started_at: now,
        };
        let observer = MockWindowObserver(Mutex::new(VecDeque::from(vec![
            WindowObservation::VisibleWindows(Vec::new());
            8
        ])));
        let outcome = observe_startup_windows(
            &timing,
            &observer,
            Duration::from_millis(55),
            &CancellationToken::default(),
        );
        assert!(outcome.measurements_ms.is_empty());
        assert_eq!(
            outcome.failures[0].kind,
            BenchmarkFailureKind::WindowTimeout
        );
        assert!(
            outcome
                .unavailable_metrics
                .contains_key(&StartupMetric::WindowVisible)
        );
        assert!(
            outcome
                .unavailable_metrics
                .contains_key(&StartupMetric::WindowReady)
        );
    }

    #[test]
    fn process_exit_before_a_window_is_recorded_as_failure() {
        let now = Instant::now();
        let timing = ProcessLaunchTiming {
            pid: 1,
            session_id: 1,
            executable_path: PathBuf::from("/fixture/browser"),
            launch_started_at: now,
            process_started_at: now,
        };
        let observer = MockWindowObserver(Mutex::new(VecDeque::from([
            WindowObservation::ProcessExited,
        ])));
        let outcome = observe_startup_windows(
            &timing,
            &observer,
            Duration::from_secs(1),
            &CancellationToken::default(),
        );
        assert_eq!(
            outcome.failures[0].kind,
            BenchmarkFailureKind::ProcessExited
        );
        assert!(outcome.measurements_ms.is_empty());
    }
}
