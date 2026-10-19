# Velocity

Velocity is an independent Rust workspace for automated browser benchmarks.
It lives inside the Photon repository and does not depend on Photon application
internals.

## Status

Implemented: macOS browser discovery and managed lifecycle, startup benchmarking,
round-based comparisons, statistical summaries, baseline regression checks,
standalone HTML reports, diagnostic resource monitoring, and automated idle,
local-page, Chromium multi-tab resource workloads, and Photon task-queue
responsiveness profiling with statistical baseline comparisons.

Not implemented: navigation-latency benchmarking, JavaScript engine suites,
style/layout and GC diagnostics, compositor/presentation timing, scrolling,
automatic machine calibration, or system cache control. Responsiveness task
queue diagnostics are process-scoped and do not measure native input-to-frame
latency. Resource workloads are descriptive and do not claim a universal
browser ranking.

## Supported browsers

macOS discovery recognizes Photon, Google Chrome, Safari, Firefox, Brave, Arc,
and Helium. Startup runs use isolated profiles for Photon, Chromium-family
browsers, and Firefox. Safari is discoverable but currently skipped for startup
benchmarking because its profile cannot be isolated safely by the current
adapter.

## Commands

Run from the Photon repository root:

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- list
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench startup --browser photon
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench startup --browsers photon,chrome,firefox --runs 10 --warmup 2 --randomize
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench startup --all --runs 5
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench responsiveness --browser photon --workload task-backlog --iterations 10 --warmup 1
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench responsiveness --browser photon --workload dom-style --iterations 10 --warmup 1
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench responsiveness --browser photon --workload layout-updates --iterations 10 --warmup 1
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- bench responsiveness --browser photon --workload gc-pressure --iterations 10 --warmup 1
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- compare startup --baseline tools/velocity/results/photon-before.json --current tools/velocity/results/photon-after.json --threshold-percent 5
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- compare responsiveness --baseline task-queue-before --current tools/velocity/results/responsiveness-after.json --threshold-percent 5
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- monitor photon --duration 10 --interval 100
```

`bench startup` options:

- `--browser <id>` selects one unambiguous browser installation.
- `--browsers <id,id,...>` selects several browsers for a comparison.
- `--all` selects every discovered installation.
- `--runs <count>` sets measured iterations per browser (default: 5).
- `--warmup <count>` adds unmeasured launches per browser (default: 0).
- `--randomize` shuffles browser order independently in each round.
- `--mode cold|warm` chooses isolated-profile behavior (default: `cold`).
- `--timeout <seconds>` sets each window-readiness timeout (default: 30).
- `--output <directory>` selects the JSON and HTML output directory (default:
  `tools/velocity/results/`).

`monitor <browser>` options:

- `--duration <seconds>` sets the diagnostic duration (default: 10; maximum:
  86400).
- `--interval <milliseconds>` sets the sample interval (default: 100).
- Intervals below 10 ms are rejected to limit overhead; each run retains no
  more than 100,000 samples in memory.
- `--output <directory>` selects where its unique JSON report is written
  (default: `tools/velocity/results/`).

The monitor creates a new isolated profile and Velocity-managed process
session, samples it synchronously, then closes the session and removes its
temporary profile. Ctrl-C requests an orderly stop and cleanup. A browser crash
ends sampling early. The command does not attach to an already-running browser.

`--browser`, `--browsers`, and `--all` are mutually exclusive. Browser runs are
always sequential. With several browsers, Velocity runs all warm-up rounds
first, then measured rounds. Each round launches each selected browser once.
`--randomize` independently shuffles the order in every warm-up and measured
round. Reports preserve the seed and the actual order for each completed round.

The CLI renders headings and summaries from Markdown and uses adaptive UTF-8
tables for browser lists, comparisons, and regression results. Markdown styling
is disabled when output is piped or `NO_COLOR` is set.

Pressing Ctrl-C or sending SIGTERM requests cancellation. Velocity stops
starting launches, closes the active managed session, finishes each isolated
profile safely, and saves partial reports.

## Warm-ups and startup modes

Warm-ups launch the browser and wait for the documented window-readiness
condition, then discard all timing measurements. Their success and failures are
retained in the JSON report. Warm-ups happen before measured rounds; with
`--mode warm`, they can prime the private profile. With `--mode cold`, each
warm-up profile is discarded and each measured launch gets a fresh profile.

Cold means a fresh browser profile, not a cold operating-system file cache.
Velocity never manipulates global caches. Warm mode reuses the same private
profile across warm-ups and measured launches while fully terminating each
managed browser between launches.

## Startup measurements

All timings use a monotonic clock and are reported in milliseconds:

- `process_start_ms`: immediately before `Command::spawn` until it returns an
  operating-system process handle.
- `window_visible_ms`: until Quartz first reports an on-screen, non-transparent,
  normal-layer window owned by the managed process session.
- `window_ready_ms`: until that same verified window ID appears in two
  consecutive observations. The observer polls at approximately 25 ms
  intervals.

Quartz Window Services (`CGWindowListCopyWindowInfo`) supplies current GUI
session window metadata. Velocity verifies each owner PID against the private
process session and launched executable; verified helper-process windows may
count. Readiness measures a stable native window, not page load or web content
readiness. The first visibility sample includes observer scheduling latency.
Missing observations are recorded as unavailable with a reason, never inferred
from process creation.

The window observer does not use Accessibility controls or capture pixels. It
does not request Accessibility or Screen Recording permission. If the current
GUI/window-server session is unavailable or ownership cannot be verified,
window metrics remain unavailable.

## Statistics and comparisons

Each measured metric summary includes median, mean, minimum, maximum, sample
standard deviation, sample count, and coefficient of variation. The coefficient
of variation is sample standard deviation divided by mean, expressed as a
percentage. Failed and unavailable measurements are excluded rather than
treated as zero. A one-sample standard deviation and coefficient of variation
are `null`.

The terminal comparison and HTML report show window-ready statistics without
ranking browsers. Profile isolation uses engine-specific launch settings, so
cross-browser values are descriptive and should be interpreted with the
recorded profile strategy and limitations.

Compare a Photon report with a previous Photon build using:

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- compare startup \
  --baseline tools/velocity/results/photon-before.json \
  --current tools/velocity/results/photon-after.json \
  --threshold 5 --min-absolute-ms 10
```

Regression analysis requires matching browser ID, OS, architecture, measured
and warm-up run counts, timeout, startup mode, isolated profile strategy, and
window-readiness condition. Each compared metric must have at least three valid
samples in both reports. Browser version and executable metadata may differ
between builds. For Phase 4 reports, the randomization policy and comparison
browser set must also match. A startup regression is marked only when its
median is slower by more than both the relative threshold and the absolute
millisecond floor. `--threshold` is an alias for `--threshold-percent`; the
default floor is 1 ms. This suppresses large percentages caused by tiny timing
changes such as 1.0 ms to 1.2 ms. The command exits nonzero on a regression and
writes a standalone HTML analysis report.

## Result files

Each selected installation gets a unique JSON file. Multi-browser runs also
write one standalone HTML comparison report with no external assets. The HTML
shows summary statistics and expandable serialized raw reports.

Generated reports in `tools/velocity/results/` are ignored by Git. Keep any
baseline you want to version in a separate, intentionally tracked directory.

JSON schema version 2 stores benchmark and browser identity, browser version and
executable path, timestamp, OS and architecture, startup mode, timeouts,
profile strategy, warm-up status, cancellation and cleanup failures, random seed,
actual round order, every measured iteration's raw metric values or unavailable
reasons, and summary statistics. Version 1 startup reports remain readable as
regression baselines when their available metadata matches.

## Persistent performance history

Successful startup, resource, and Speedometer runs are recorded automatically
after Velocity has saved their ordinary JSON reports. History storage errors
are printed as warnings and do not invalidate a benchmark result. Use
`--no-record-history` to disable this for one run; `--record-history` is also
accepted explicitly. The report remains available for later manual import.

On macOS the default store is
`~/Library/Application Support/Velocity/`; on other systems it uses
`$XDG_DATA_HOME/velocity/` or `~/.local/share/velocity/`. Set
`VELOCITY_DATA_DIR` to choose another location for both automatic recording and
history commands. History and archive files are private to the current user.
Velocity uses a locked JSON index plus SHA-256-named copies of the original
reports. Index updates are atomic, concurrent writers are serialized, imports
are idempotent, and raw reports are never rewritten. `history export` verifies
the archive hash and refuses to overwrite an existing output file.

Import existing startup, resource, and Speedometer 3.1 reports:

```sh
velocity history import /path/to/photon-speedometer.json
velocity history import /path/to/photon-startup.json --label before-window-change
velocity history import /path/to/photon-speedometer.json --commit 0123abcd
velocity history list --browser photon
velocity history list --browser photon --benchmark speedometer
velocity history show <record-id>
velocity history export <record-id> --output /tmp/original-report.json
```

Import accepts startup report schemas 1–2, resource schemas through 3, and
Speedometer schemas 1–2 for the pinned 3.1 source. Unsupported future schemas,
malformed reports, missing required browser/environment metadata, summaries
without matching raw startup measurements, unverified Speedometer completion,
and non-finite or non-positive Speedometer scores are rejected. Resource
imports preserve their measurement methodology, workload configuration,
scenario, attribution/coverage assessment, and warnings. Failed-only resource
or Speedometer reports are not added to history. Reimporting identical bytes
returns the existing record IDs instead of adding copies.

Name an immutable record as a baseline. A multi-browser report needs
`--browser` to select one result; replacing an existing name requires
`--replace`:

```sh
velocity baseline set --name photon-initial \
  --from /path/to/photon-speedometer.json --browser photon
velocity baseline list
velocity baseline show photon-initial
velocity baseline delete photon-initial
```

Deleting a baseline only removes its name. The history record and archived raw
report remain. Baselines and current selectors may be JSON paths, history record
IDs, or baseline names, so reports can be compared without copying their data:

```sh
velocity compare speedometer --baseline photon-initial \
  --current /path/to/photon-speedometer-new.json --threshold 5
velocity compare startup --baseline photon-startup-old \
  --current photon-startup-new --threshold 5 --min-absolute-ms 10
velocity history report --browser photon --benchmark speedometer \
  --baseline photon-initial
```

Comparisons keep the existing benchmark configuration and environment checks.
Speedometer requires the same pinned benchmark version, run configuration,
browser installation, OS, and architecture. Startup retains its isolated
profile, mode, timeout, readiness, run-count, and absolute/relative threshold
checks. A known hardware model, CPU model, core-count, or host-identity mismatch
makes the comparison incompatible. Missing metadata is treated as unknown and
called out; OS and architecture alone do not prove that runs used the same
machine. A hash of the platform machine ID
is stored for automatic runs to group results from the same host; Velocity
never stores the raw machine ID. Imported historical reports generally lack
that fingerprint.

Regression classification separates threshold changes from statistical
evidence. Startup and Speedometer compare per-run medians with a two-sided
permutation test. A threshold-crossing change is called a regression or
improvement only when `p < 0.05`; otherwise it is inconclusive. Five valid raw
samples per side are needed for that classification. Comparisons with fewer
samples can show the numeric change, but a threshold crossing is not reported
as statistically established. Startup still requires its existing relative
and absolute millisecond floors. Resource history is recorded and charted, but
Velocity does not rank resource results across incompatible configurations or
incomplete attribution/coverage.

`history report` creates standalone HTML with inline SVG and no network assets.
It shows medians, sample variation, environment, build labels, and changes from
an optional baseline. Versions and configurations are separated. Trend lines
are drawn only when the same hashed host identity is available on each record;
older imports without it remain unconnected. Red markers mean a configured
threshold was crossed, not that the permutation test proved significance.

History metadata never treats the current repository commit as the browser
executable's build revision. The checkout revision and branch observed at
import time are shown separately; executable build revision/type/timestamp
remain unknown unless reliable report metadata supplies them. `--label` and
`--commit` attach user-provided metadata; a user-supplied commit is visibly
identified as unverified. Browser version and executable path come from the
original benchmark report. Hardware not present in an imported report remains
unknown rather than being inferred from the machine used for a later import.

## Resource benchmarks

Velocity can measure idle browser resources, a controlled local page, and
verified Chromium-family tabs. Each measured run gets a private browser profile,
performs the workload, waits for readiness, settles, samples resources, and
closes its managed process session. Runs are sequential. Warm-up runs execute
the workload and cleanup without recording resource samples.

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench idle --browser chrome --runs 5 --duration 10 --settle 5

cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench page --browsers photon,chrome,firefox --fixture complex --runs 5

cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench tabs --browsers chrome,helium --tabs 1,5,10 \
  --fixture simple --runs 5 --randomize
```

`idle` launches an isolated browser at `about:blank` and measures after a
verified visible window and settling period. `page` measures a stable page state;
it does not measure navigation latency. `tabs` creates separate browser tabs and
verifies their count through CDP before accepting the workload. Defaults are 5
measured runs, no warm-ups, 10 seconds of sampling, 5 seconds of settling, and a
100 ms sample interval.

Options for all three resource workloads:

- `--browser <id>`, `--browsers <id,id,...>`, or `--all` selects installations.
- `--runs <count>` sets measured runs (1–100); `--warmup <count>` sets unmeasured
  workload warm-ups (0–20).
- `--randomize` shuffles browsers within each workload/run round and records the
  seed and execution order.
- `--duration <seconds>`, `--settle <seconds>`, and `--interval <milliseconds>`
  configure sampling. `--navigation-timeout <seconds>` waits for each fixture
  request to reach the local server; `--readiness-timeout <seconds>` waits for
  each page's load beacon or the idle window observer. `--timeout <seconds>` is
  a convenience that sets both.
- `--fixture simple|complex|dynamic` chooses a page fixture. `--tabs 1,5,10`
  chooses tab counts for the `tabs` workload only (1–50 tabs and at most 10
  distinct counts per command). Dynamic tab scenarios are limited to 100,000
  total fixture elements.
- `--intensity <count>` and `--update-interval <milliseconds>` configure the
  deterministic dynamic fixture. They are recorded in fixture metadata.
- `--output <directory>` selects the destination (default:
  `tools/velocity/results/`).

Configuration is validated before launching a browser. Velocity caps a run at
100,000 retained resource samples total across measured runs, browsers, and tab
scenarios; larger requests must use a longer interval or fewer scenarios.

Fixtures are served from Velocity's checked-in `fixtures/` files by a temporary
HTTP server bound to `127.0.0.1` on a dynamically assigned port. `simple` is
static minimal content; `complex` generates a fixed 128-card DOM and local CSS;
`dynamic` deterministically updates a configured number of DOM elements. There
are no remote assets or services. Each served page includes a readiness script
that sets `window.__velocityReady` when the document has fully loaded, then
sends a run- and tab-specific local HTTP beacon. The server separately observes
the fixture request as navigation arrival and the beacon as readiness. The
configurable settling time follows readiness so background-tab animation
throttling cannot create false readiness timeouts. The server stops at command
completion, including error and cancellation paths.

### Automation support

| Workload | Current automation | Support and limits |
| --- | --- | --- |
| Idle | Quartz observes a stable visible window owned by the managed process session; Photon also verifies its authenticated automation session and blank tab | Photon, Chromium-family browsers, and Firefox when isolated profile launch succeeds |
| Single page | Launch URL or runtime navigation plus the local, run-and-tab-specific readiness beacon | Photon, Chromium-family browsers, and Firefox; Photon runtime navigation uses its Automation Protocol v1 |
| Multiple tabs | Loopback Chrome DevTools Protocol or Photon Automation Protocol v1; verifies tab count and every fixture beacon | Chrome, Helium, and Photon have verified protocol adapters. Other Chromium products are rejected until isolated-profile tab control is verified. |

Photon's protocol is disabled unless the executable receives
`--automation-socket <absolute-path>`. Velocity creates the iteration's
isolated browser profile and a separate short-path, mode-`0700` socket
directory with a 256-bit credential; only the socket path appears in argv.
Photon validates the current user's private
session root and sibling `HOME`/profile directories, socket ownership and
permissions, and authenticates each request. Protocol support is
considered available only after a successful version and capability handshake.
The protocol reuses Photon’s ordinary tab creation, closure, activation, and
navigation methods on the GPUI app thread. It does not expose JavaScript
evaluation or arbitrary filesystem access. Firefox lacks a verified tab
control adapter, and Safari lacks a safe isolated-profile adapter, so their
tab or resource workloads are reported unsupported. Arc did not publish an
isolated-profile DevTools endpoint in smoke runs. Brave was not installed on
the verification host and is not advertised as tab-capable. Protocol failure,
unverified tab count, or a missing fixture beacon fails the iteration.

Photon currently reports document readiness as unavailable in its status
command. Page and tab workloads therefore use the existing Velocity fixture
beacon, which is emitted only after the actual page's `load` event and is
associated with the run token and tab index. Native window visibility is not
used as proof of webpage readiness. Chrome and Helium use CDP to read the
fixture's fixed, read-only `window.__velocityReady` signal on the assigned
target and verify that target's URL exactly matches its assigned fixture URL,
including the run and tab identifiers.
Chromium tabs are opened and verified sequentially before the next tab is
created. This does not expose arbitrary JavaScript evaluation through Photon.
See
[`Documentation/AutomationProtocol.md`](../../Documentation/AutomationProtocol.md)
for the protocol and isolation details. See
[`Documentation/Responsiveness.md`](Documentation/Responsiveness.md) for task
queue diagnostics, the current workload, report semantics, and baseline
comparison requirements.

### Resource measurements and reports

Resource workloads reuse the Phase 5A macOS collector. CPU is derived from
per-process cumulative user and system execution time deltas divided by monotonic
elapsed time; 100% is one logical core and multicore usage may exceed 100%.
Memory footprint uses macOS `ri_phys_footprint`; resident memory uses
`ri_resident_size`. Values are kept in bytes. Aggregate footprint is the sum of
verified process footprints. Aggregate RSS may double-count shared pages and is
not unique physical RAM.

Velocity identifies processes with `(PID, process start time)` and revalidates
the identity before retaining samples, preventing PID reuse from carrying CPU
counters forward. The managed root must also remain in the private session
created for that Velocity launch. Descendants are checked against their parent
PID, effective user, and private session. A periodic libproc process-table scan
finds reparented processes that remain in the private session. Process group,
session, executable, parent identity, and the ownership evidence are preserved
with raw per-process samples.

For Chromium-family helpers outside the private session, Velocity requires all
of the following: the process executable resolves inside the selected browser
application bundle, the executable belongs to the current user, and bounded
kernel argv metadata contains the exact isolated `--user-data-dir` path. The
process name is only a candidate-discovery filter and never proof of ownership.
Command-line data is used in memory and is not logged or serialized. Candidates
that cannot pass these checks are excluded and counted with a structured
reason. Candidates explicitly carrying a different isolated-profile path are
known unrelated; candidates whose ownership arguments or process identity
cannot be verified remain uncertain and mark attribution partial. CDP
targets are not treated as operating-system process IDs.

Photon's authenticated Automation Protocol adds the read-only
`list_processes` capability. It snapshots the actual active records owned by
Ladybird's `WebView::ProcessManager`, including the browser process and helper
roles that manager has launched, and the separate macOS `ProcessReaper`
lifecycle helper when it was started successfully. At process registration,
the engine captures the macOS process start time and the parent's PID and start
time. The process start time is also its launch-generation identity. The
registry reports only what it actually tracks and does not infer helper
processes from names.

Velocity independently re-reads each reported PID through libproc and requires
the reported `(PID, start time)` and parent identity to match live libproc
metadata, verifies the private session and user, and checks the root executable
against the managed install. Only then is the process sampled. A mismatch,
unavailable process or parent identity, or unregistered process discovered
inside the private session lowers attribution coverage. Process lifecycle
changes are recorded by comparing verified `(PID, start time)` identities
between samples, so PID reuse cannot inherit CPU history. `VERIFIED COMPLETE`
for Photon is scoped to the named
`WebView::ProcessManager` plus its separately reported `ProcessReaper`; it is
not a claim about unrelated native services or every process on the system.

For Chromium-family browsers, helpers are included when they are in Velocity's
private session or when libproc verifies the current user's process, an
executable inside the selected browser bundle, and the exact isolated
`--user-data-dir` argument. Process type arguments label roles when available.
Names are candidate filters only. CDP targets are not OS process IDs. Helpers
without these checks are excluded and diagnosed; Chromium has no authoritative
complete-process registry in this implementation, so absence of observed
unknown helpers does not promote its attribution status to complete.

Sampling coverage and process attribution are separate. Reports show attempted,
valid, failed, CPU, footprint, and RSS sample counts. Attribution reports
verified process observations, excluded candidate observations, uncertain
candidate observations, exited process observations, identity verification
failures, and evidence categories. Counts accumulated across a run are
observations across samples rather than unique PID totals; they show how often
those cases were observed.
Statuses are `VERIFIED COMPLETE`, `VERIFIED PARTIAL`, `UNKNOWN`, or
`UNSUPPORTED`. A complete state requires an authoritative registry declaring
its stated scope complete, every registered process independently verified,
and no relevant unregistered process observed in the private session. This is
not a cross-browser completeness claim. Chromium remains `UNKNOWN` when its
helper set cannot be proven exhaustive; Photon completeness is limited to the
explicit ProcessManager scope. Comparisons remain inconclusive when these
scopes or coverage states do not support a fair conclusion. No coverage
percentage is reported because an unknown helper count has no defensible
denominator.

Cross-browser assessment also requires the registered attribution source and
named scope to match. Two individually complete process sets with different
scopes remain inconclusive; a complete Photon ProcessManager set is not
silently treated as equivalent to a different browser's process inventory.

CPU is calculated from valid cumulative counter deltas divided by actual
monotonic elapsed intervals; the mean is time-weighted and does not clamp at
100% (one fully used logical core). Intervals with process churn, PID identity
changes, resets, or unavailable counters have no aggregate CPU value rather
than a fabricated zero. Memory footprint is the sum of verified
`ri_phys_footprint`; resident memory is the sum of verified `ri_resident_size`.
Neither aggregate is exact unique physical RAM: shared pages and process
architecture affect totals, and RSS can double-count shared pages. Peaks are
sampled peaks and may miss activity between intervals. Process-table scans and
per-process libproc queries add monitoring overhead.

Resource comparison statuses are `COMPARABLE`, `INCONCLUSIVE`, or
`INCOMPATIBLE`. Incompatible profile policies, resource schemas, and sampling
intervals are marked incompatible. Insufficient successful iterations, weak
metric sampling coverage, failed iterations, or attribution that is not
verified-complete makes a result inconclusive. Raw measurements remain in the
JSON report in all cases; a successful workload is not automatically a
trustworthy cross-browser comparison. HTML reports show classification reasons,
sampling coverage, attribution evidence and candidate counts, and do not rank
inconclusive results.

Each run writes unique `idle-*.json`/`.html`, `page-*.json`/`.html`, or
`tabs-*.json`/`.html` reports. JSON retains configuration, fixture and version,
browser and system metadata, automation protocol, randomized order, failures,
cleanup status, every raw per-sample timestamp and process measurement, and
statistics within each iteration and across repeated iterations. Reports carry
a schema version and a measurement-methodology version so later collectors can
be distinguished. The standalone
HTML report contains comparison tables, CPU and physical-footprint time-series
charts, memory by tab count, tab-count scenarios, failures, and coverage
warnings. It is offline and has no external assets. Chart x values are elapsed
within each run; overlaid runs were sampled sequentially and did not occur
simultaneously. Charts decimate dense time-series traces for display; JSON
retains every point.

Idle uses `about:blank`, not each browser's personal new-tab page. Page and tab
workloads use the same fixture and settings across selected browsers. Results
are descriptive: profile startup settings, background services, helper-process
attribution, OS activity, and thermal state can still differ. Velocity does not
clear system caches or run competing browsers concurrently.

## Resource monitoring (diagnostic)

`velocity monitor <browser>` collects CPU and memory samples from a newly
launched isolated browser. It is an infrastructure check, not a formal or
comparative benchmark; results are not ranked.

Monitoring currently requires an engine adapter that supports an isolated
Velocity profile. Photon, Chromium-family browsers, and Firefox support this
path; Safari is discovered but its current adapter cannot isolate a profile, so
Velocity refuses to monitor it with the user's normal profile.

macOS collection uses libproc process metadata and resource usage. The root PID
must still match Velocity's recorded microsecond-resolution process start time
and private session ID; its executable path was verified at launch. macOS may
move a signed application image to a temporary code-sign clone path after
launch. If that happens, Velocity records the current and launched paths as a
coverage warning and continues only while the PID, start time, and private
session identity still match. Child processes are found recursively
from that root, checked against their parent PID and private session ID, and
retained across reparenting only while their original PID/start-time identity
and private session membership still match. Process names are never used as
ownership evidence. Processes started outside this private session, including
some launchd-managed browser helpers, cannot be attributed safely and are
excluded. Such limitations are included as coverage warnings; the report does
not claim complete browser-process coverage.

CPU uses the difference between consecutive cumulative per-process user plus
system CPU times from macOS `proc_pid_rusage`, converted from Mach time units
using `mach_timebase_info`, then divided by monotonic elapsed time.
100% represents one fully used logical CPU core; multi-core totals can exceed
100%. If verified process membership changes during an interval, aggregate CPU
for that interval is unavailable rather than silently undercounted. New
processes become eligible after a baseline sample. Counter resets and
unavailable process counters are also reported. Memory aggregates are labelled
as sums of verified process footprints or resident sizes, not unique physical
RAM. Shared pages and differences in process architecture can affect either
aggregate; missing values are never treated as zero.

Multi-tab reports include descriptive memory scaling per browser and tab count:
mean and median of per-run mean footprint, peak observed footprint, and
incremental and average marginal footprint for adjacent configured tab counts.
The marginal value is only the difference between two measured scenarios and
does not represent an isolated per-tab cost or assume linear scaling. A missing
scenario breaks the adjacent comparison. If either scenario is inconclusive,
the scaling result is labelled diagnostic/inconclusive and is not ranked.

Physical memory is macOS `ri_phys_footprint`; resident memory is
`ri_resident_size`. Both are collected per verified process and reported in
bytes, with MiB used for terminal display. Aggregate footprint is the sum of
per-process footprints. Aggregate resident memory is a sum of RSS and can count
shared pages more than once, so it is not unique physical memory consumption.
Missing per-process memory makes that sample's corresponding aggregate
unavailable. No missing measurement is replaced with zero.

No Accessibility or Screen Recording permission is needed. macOS can restrict
resource queries for protected or unrelated processes; unavailable values are
omitted and recorded as coverage or collection warnings rather than bypassed
with elevated privileges. The libproc interfaces are macOS-specific and can
vary by OS release; unsupported or denied queries stay unavailable and are
reported.

The sampler defaults to 100 ms and is synchronous, without a sampling thread or
shelling out to `ps`/`top`. It performs libproc process-tree enumeration and
resource queries per sample, so sampling adds some system load. Means, minima,
maxima, and final observed memory use only samples where a metric was
available. Peaks are the highest sampled values, not guaranteed instantaneous
peaks between samples.

Each `monitor-<installation>-<run>.json` report stores browser and OS metadata,
the managed root PID/session, requested duration and interval, process
attribution method, cleanup status, every timestamped sample, per-process
measurements, coverage warnings, failures, and summary statistics. The resource
report has its own schema version. Collection failures include a machine-readable
kind and message; bytes are retained without display-unit rounding.

## Process safety and limitations

Every launch runs in a private macOS process session. Velocity tracks the
managed process tree and only signals a process group it can verify as its own;
it never kills by application name or closes a pre-existing browser session.
If ownership or cleanup becomes uncertain, it refuses destructive cleanup and
retains the private profile with a report failure.

Discovery, process lifecycle, window observation, and resource collection
currently support macOS only. Warm-up and random ordering reduce some systematic
effects but do not control background activity, thermal state, power management,
or file caches. Navigation latency, JavaScript, rendering, GPU, and network
benchmarks remain future work.

## Workspace

- `velocity-core`: benchmark definitions, startup and resource results,
  configuration, statistics, and cancellation.
- `velocity-browser`: browser models, registry, managed sessions, automation
  capabilities, Chromium DevTools client, and macOS discovery/process/window/
  resource adapters.
- `velocity-metrics`: shared metric units, process resource samples, the
  synchronous monitoring engine, and resource statistics.
- `velocity-cli`: command parsing, comparison scheduling, workload and lifecycle
  orchestration, local fixtures, JSON output, regression analysis, HTML reports,
  and diagnostic/resource benchmark commands.
- `benchmarks/`: reserved for future workload definitions.

Velocity has its own Cargo workspace and lockfile, separate from Photon's root
workspace.

## Speedometer 3.1

Velocity can run the unmodified official Speedometer 3.1 suite in a fresh,
managed, headed browser session. The official `startAutomatically` query
parameter starts the harness without a manual click. Velocity waits for the
benchmark's running and summary states, then reads the official
`benchmarkClient.metrics.Score` metric and its values. It does not calculate a
replacement score or infer a score from navigation time. Speedometer keeps its
default ten internal iterations and workload configuration.

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench speedometer --browser chrome --runs 5 --warmup 1

cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench speedometer --browsers photon,chrome,helium \
  --runs 5 --warmup 1 --randomize

cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  compare speedometer --baseline before.json --current after.json \
  --threshold 5
```

The benchmark runs sequentially. Warm-up runs execute the complete official
test and are kept in report diagnostics, but are excluded from measured
statistics. Browser order is randomized by round when requested, and reports
store the seed and observed order. The default timeout is 600 seconds per run;
`--timeout` accepts 30–3600 seconds. Headless runs are rejected because this
adapter does not establish comparable headless conditions. At least three valid
measured runs are required before Velocity permits a comparison. Scores are
higher-is-better; a lower score exceeding the configured relative threshold is
a regression. The report describes score differences and does not claim that a
small median difference is statistically significant.

The default `--benchmark-source official` uses the versioned official hosted
page at `https://browserbench.org/Speedometer3.1/`. For repeatable offline
workloads, prepare a local copy:

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  setup speedometer
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench speedometer --browser chrome --benchmark-source local --runs 1
```

Setup downloads the official WebKit/Speedometer source at commit
`1386415be8fef2f6b6bbdbe1828872471c5d802a` and verifies its pinned SHA-256
before extraction. Local runs serve that copy from an ephemeral loopback HTTP
port. The local copy is not rewritten or patched by Velocity. Hosted runs are
versioned by the official 3.1 URL but do not expose a source commit; reports
record this distinction. The local cache is stored under
`~/Library/Caches/Velocity/speedometer/` and is separate from browser profiles.

Chrome and Helium have isolated-profile CDP adapters. Photon has a negotiated,
authenticated `probe_speedometer_result` operation in explicit automation
sessions. It is restricted to the official Speedometer 3.1 origin or a loopback
Speedometer 3.1 page and reads the benchmark's official score without accepting
caller-supplied JavaScript. Other discovered browsers are reported unsupported
for this workload.

**Live validation (2026-10-14):** Chrome completed three measured runs after
one excluded warm-up on the official hosted page. Measured scores were
59.985, 60.026, and 59.726 (median 59.985, mean 59.912, sample standard
deviation 0.162); all completion and cleanup checks passed. Helium completed a
separate hosted smoke run with a verified score of 57.056 and successful
cleanup. Earlier one-run smoke scores differed, so these runs are not treated
as a cross-browser comparison. Hosted runs do not expose a pinned source
commit. The original Chrome exit and Helium CDP reset were not reproduced, and
the old `/tmp` reports were unavailable, so their original causes remain
unknown. After the local-server fixes below, Chrome completed a one-run check
against the pinned source at `60.720958330347536`; the earlier CDP disconnect
was not reproduced.

**Photon pinned-source validation (2026-10-15):** The observer's original
`page_unavailable` failure came from a native bridge pointer-cast bug: the
restricted observer treated a returned `ViewHandle` as a `Photon::View`. The
bridge now unwraps the handle correctly. `get_tab_status` exposes the actual
embedder `on_load_finish` state, and Velocity waits for it and checks tab crash
state before installing the observer.

A separate confirmed engine assertion was in viewport updates: WebContent
unconditionally treated the page's top-level traversable as local, although a
valid page may have a remote top-level traversable. PageClient now calls a
LibWeb helper that updates viewport size only when the page has a local
traversable. The `TestPageViewport` regression covers both local and remote
top-level cases; it passed in the release engine build.

The first benchmark-run failure was in Velocity's local source server. Accepted
connections inherited the listener's nonblocking mode, so large fixture
responses could stop with `WouldBlock`; accepted streams are now switched to
blocking mode, and a regression test verifies the complete 700,000-byte body.
The next failure was independently reproduced in the official
`Perf-Dashboard` suite: its frame navigates to
`resources/perf.webkit.org/public/v3/`, but the server returned `404 not found`
because it did not map nested directory URLs to `index.html`. The handler now
serves a directory index after canonicalizing and checking that it remains
inside the pinned source root. A focused regression test requests this exact
nested URL. The suite waited for its `#app-is-ready` element, which could not
exist in the plain-text 404 document. This was a Velocity source-serving bug,
not a confirmed JavaScript engine stall; the earlier TodoMVC tests were seen
progressing in the successful run.

Photon completed the full, unmodified pinned Speedometer 3.1 source at commit
`1386415be8fef2f6b6bbdbe1828872471c5d802a` in the repository's installed
Photon app bundle. The first verified score was `12.438724688400566` (10
official internal iterations, completion verified, no failed suites). A
separate warm-up plus three measured runs scored `12.557977802910989`,
`12.028336377298244`, and `11.805356945193214` (median `12.028336377298244`,
mean `12.130557041800815`, sample standard deviation `0.3865828980393081`,
coefficient of variation `3.186851986328229%`). All four executions completed
all ten official internal iterations and cleaned up the app, profile, and
automation resources. The first run's JSON and HTML are under
`/tmp/velocity-speedometer-phase9-2-nested-index-fix/`; the repeatability
reports are under `/tmp/velocity-speedometer-phase9-2-photon-repeatability/`.

For local source-server diagnosis, set `VELOCITY_SPEEDOMETER_HTTP_TRACE=1`;
the trace records at most 64 request paths/statuses/body sizes/write results.
For Photon lifecycle diagnosis, set
`PHOTON_AUTOMATION_LIFECYCLE_TRACE=1`; that trace is also bounded. Both are
opt-in and are disabled in ordinary sessions. Failure reports preserve the
last observed benchmark state, cleanup reason, managed PID/session identity,
process count, and observed exit code/signal. Raw stdout/stderr is not retained
because browser helper processes inherit those streams and may emit host
paths or personal profile metadata. Process output is never used as a score
source.

Each run writes unique JSON and standalone offline HTML reports under
`tools/velocity/results/` by default; `--output` changes the destination. JSON
keeps browser metadata, source/version, environment, configuration, execution
order, warm-up and measured iterations, official per-iteration scores,
failures, managed-process diagnostics, cleanup status, and summary statistics.
Reports never include the session authentication token. Missing or invalid
scores remain unavailable and failed runs are excluded from statistics. The
compare command requires matching source mode/version, run configuration, OS,
and architecture; it reuses the
existing nonzero-on-regression exit convention.

Speedometer is a CPU responsiveness workload. It does not produce Velocity
CPU or memory monitoring samples, and it does not replace the idle/page/tab
resource workloads. Browser versions, OS scheduling, thermal state, power
settings, and foreground activity can affect results. Follow the official
Speedometer instructions and keep the machine otherwise idle for comparisons.

## JetStream 3.0

Velocity runs the complete, unmodified official BrowserBench JetStream 3.0
suite in isolated, headed browser sessions. The benchmark version and source
are pinned to WebKit/JetStream commit
`06785cf861ac44855f168cbbe829278c2802e6de`. Setup verifies the downloaded
archive against SHA-256
`0361851eb2ddeb003e06be74239c3f7ce437c64e774a29ba446faaed9ab16fa9` before
extracting it. The cache is stored under
`~/Library/Caches/Velocity/jetstream/`, outside browser profiles.

Prepare the pinned source and run Chrome:

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  setup jetstream
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench jetstream --browser chrome --runs 5 --warmup 1
```

Photon, Chrome, and Helium can be selected by their discovered ID or number.
Velocity runs browsers sequentially, reuses the official Start Test control,
uses the complete default workload set and parameters, and verifies all
workloads finished before accepting the score. The `--benchmark-source local`
mode is the default and serves the verified pinned files from a loopback-only
server. `--benchmark-source official` uses the versioned page at
`https://browserbench.org/JetStream3.0/index.html` and requires network access.
Hosted reports record the official version but have no immutable source commit;
Velocity therefore rejects them for statistical baseline comparisons.
Chromium pages are brought to the foreground before start and reactivated at a
bounded rate if the document becomes hidden, avoiding background-tab timer
throttling during the workload.

```sh
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench jetstream --browsers photon,chrome,helium \
  --runs 5 --warmup 1 --randomize
cargo run --manifest-path tools/velocity/Cargo.toml -p velocity-cli -- \
  bench jetstream --browser photon --benchmark-source official \
  --runs 1 --warmup 0
```

JetStream scores are higher-is-better and are distinct from Speedometer scores.
Velocity stores the official score shown by the completed result page, the
individual official workload scores, and completion evidence. It does not
recalculate JetStream's geometric mean. Warm-up scores are kept in reports but
excluded from measured statistics. Summaries include median, mean, min, max,
sample standard deviation, and coefficient of variation. Successful reports
are automatically imported into persistent history; use `--no-record-history`
to disable this for a run.

Name a historical result as a baseline, list JetStream history, and compare
two local pinned-source reports:

```sh
velocity baseline set --name photon-jetstream-initial \
  --from results/photon-jetstream.json --browser photon
velocity history list --browser photon --benchmark jetstream
velocity history report --browser photon --benchmark jetstream
velocity compare jetstream --baseline photon-jetstream-initial \
  --current results/photon-jetstream-new.json --threshold 5
```

Comparison requires matching JetStream source revision, version, workload
configuration, execution mode, OS, architecture, and comparable recorded
hardware. Each side needs at least five valid measured scores. Velocity reuses
the history comparison's median permutation test; crossing the percentage
threshold alone does not establish a regression. With a p-value below 0.05, a
score decrease beyond the configured threshold is reported as a regression;
otherwise the result is improvement, within-threshold, inconclusive, or
incompatible. This test has limited power for small samples and should not be
read as a universal guarantee of statistical significance.

JSON and standalone offline HTML reports retain the source revision and hash,
browser metadata, environment, run order and randomization seed, per-run and
per-workload scores, failures, process diagnostics, cleanup state, and summary
statistics. Raw reports remain available through the normal Velocity history
archive. Unknown executable build revisions stay unknown; a repository
revision is not assumed to identify the built browser.

Photon automation uses two authenticated, fixed JetStream 3.0 operations. The
read-only observer is tied to the active document generation and extracts
bounded state from JetStream's own UI. The start operation can only activate
JetStream's own default Start Test control on the official page or the pinned
loopback route; it accepts no caller-provided JavaScript. Chrome and Helium use
the same fixed-state and official-control checks over CDP. Chrome and Helium
both completed all 77 JetStream 3.0 workloads from the pinned local source on
this machine. Chrome's successful three-run set scored 447.45, 453.72, and
463.69 (median 453.72, mean 454.95, sample standard deviation 8.19). A separate
Helium run scored 457.71. One-run smoke results also completed on Chrome at
424.11 and 457.14. These measurements are local validation results, not a
cross-machine target or a basis for comparing different browser versions.

For a fresh Photon development build, run `./photon run` once to register the
development app's presentation service before using Velocity. Until registered,
Photon automation returns a structured service-not-registered failure.

Photon's hosted diagnostic run reached 7 of 77 workloads, then the official
`transformersjs-bert-wasm` workload failed while compiling the pinned
ONNX Runtime WebAssembly binary (`ort-wasm-simd-threaded.wasm`, SHA-256
`f061472c6e77d6d50d079aacdc0ff9b63fee287ddd2cbf46cf62438d3891de2b`). The
file served by the official hosted URL has the same hash as the pinned local
source. The module imports `a.a` as shared memory. At byte offset 2779 its limits
flags are `0x03` (has maximum and shared), with minimum 256 and maximum 65536
pages. `LibWasm::Limits::parse()` rejects the shared bit and returns
`InvalidTag`, which becomes the reported
`CompileError: A parsed tag did not make sense in context`. The engine also
does not preserve sharedness in `Wasm::Limits`, and its WebAssembly instance
wrapper still marks exported memories as non-shared. This is an incomplete
WebAssembly threads/shared-memory implementation, not an observer or score
extraction failure.

The minimal diagnostic module is 19 bytes:

```text
00 61 73 6d 01 00 00 00 02 09 01 01 61 01 61 02 03 00 01
```

It contains only an import of shared memory `a.a` with limits flags `0x03`,
minimum 0 and maximum 1. Node accepts it (`WebAssembly.validate()` is true and
compilation succeeds). A direct Photon run against the same local test page
reported `WebAssembly.validate() === false` and the same `CompileError`. The
official 11 MB ONNX module likewise passes Node validation. Accepting the flag
alone would be unsafe because sharedness is not carried through the Wasm memory
type and instance model; completing this workload requires that engine support,
which is outside Velocity's scope. No official JetStream workloads were
changed or skipped, and Velocity correctly rejects the incomplete run.

A separate Photon run against the local loopback benchmark copy failed during
initial resource prefetch with connection failures; that transport issue has
not yet been isolated. Photon therefore does not yet complete JetStream 3.0.
