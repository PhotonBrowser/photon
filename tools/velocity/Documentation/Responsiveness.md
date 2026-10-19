# Velocity responsiveness and frame timing

Velocity measures Photon/Ladybird responsiveness before engine optimizations are
attempted. It reuses the existing isolated browser launcher, authenticated
Photon automation, process-tree resource sampler, JSON/HTML reporting, history,
and named baselines. Diagnostics are opt-in and are enabled only for benchmark
sessions.

## Frame lifecycle and timestamp ownership

The inspected macOS path is:

```text
Ladybird WebContent rendering and compositor backing-store update
  → LibPhotonEmbedder publishes an IOSurface-backed frame update with backing-pool generation and WebView frame ID
  → photon-shell receives the callback and queues the newest available surface
  → PresentationRuntime may replace an uninstalled surface with a newer one
  → PhotonWebView installs the selected surface into its page view
  → GPUI-CE samples the external Metal surface while rendering the Photon window
  → GPUI-CE obtains a CAMetalDrawable and commits the window command buffer
  → CAMetalDrawable presented handler reports presentedTime, when available
```

Ladybird owns web-content rendering, its compositor backing stores, and the
shared-surface update. Photon owns the callback boundary, latest-frame queue,
surface installation, and window lifecycle. GPUI-CE's macOS Metal renderer owns
window-drawable acquisition and GPU command submission. Metal/Core Animation
owns scheduling and displaying the window drawable. Ladybird does not directly
present a window drawable. The embedder increments the frame ID at surface
publication; it identifies a surface update, not an Engine-side frame-start
time or a content/pixel generation.

| Stage | Measurement | Clock domain | Meaning and limit |
| --- | --- | --- | --- |
| Engine frame created/completed | unavailable | — | Ladybird does not export a correlated source timestamp. The frame ID is assigned at native surface publication. |
| Surface handoff | `surface_handoff` | Photon `Instant` elapsed nanoseconds | Photon callback receipt for a generation/frame ID. It does not measure IPC time because there is no source-process timestamp. |
| Surface install | `photon_surface_install` | Photon `Instant` elapsed nanoseconds | Photon page-view installation of that surface update. |
| GPU sampling submission | `gpu_sampling_events[].submitted_elapsed_ns` | Photon `Instant` elapsed nanoseconds | CPU callback immediately before the command buffer that samples the surface is committed. It is paired by `(generation, engine_frame_id, drawable_id)`. It is not GPU start time. |
| GPU completion callback | `gpu_sampling_events[].completion_callback_elapsed_ns` | Photon `Instant` elapsed nanoseconds | Receipt of that command buffer's completion callback, paired by the same drawable ID. The delta is CPU elapsed time through callback receipt, not a GPU hardware duration or display timestamp. |
| Actual drawable presentation | `actual_presentation` | Metal host-time seconds | `CAMetalDrawable.presentedTime` reported by Metal's presented handler. Zero or invalid time is retained as missing/dropped feedback, not converted into a timestamp. |

Apple defines `presentedTime` as the host time when the drawable was displayed;
Apple's presented handler runs after presentation. The measurement therefore
reports actual window-drawable presentation feedback when Metal supplies it,
not CPU submission or GPU completion ([`presentedTime`](https://developer.apple.com/documentation/metal/mtldrawable/presentedtime),
[`addPresentedHandler`](https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler%28_%3A%29)).
A presented window drawable does not prove that the window was unobscured or
that a person perceived its contents.

All elapsed stage durations use one Photon-process monotonic `Instant` origin.
Frame intervals use differences between Metal host-time presentation
timestamps. Velocity never subtracts a Photon `Instant` timestamp from a
Ladybird-process timestamp or a Metal host timestamp. The available `metal`
crate does not expose GPU start/end timestamps here. The display refresh-rate
field is the known nominal display rate when the macOS display adapter can
report one; it is `null` when unknown. It is context, not a fixed threshold,
and does not describe every variable-refresh presentation interval.

Surface identities use the pair `(generation, frame ID)`. Photon explicitly
counts a queued surface that is replaced before installation. One installed engine surface may be
sampled into more than one Photon window drawable. Photon retains one bounded
event for every Metal presented-handler callback, including repeated displays
of the same engine surface. Each event carries the generation, engine frame ID,
drawable ID, and that drawable's `presentedTime`; the per-surface frame row
also keeps the most recent valid timestamp as a compact summary. The generation
is the IOSurface backing-pool epoch; the frame ID is the embedder's publication
sequence within one WebView. This pair identifies a published surface update,
not a content generation and not a visible-pixel change. The authenticated
diagnostics command is addressed to a tab ID, so samples are scoped to that
tab's Engine WebView. Navigation that retains the WebView continues its
sequence; replacing the WebView begins a new diagnostic scope. A drawable event
and GPU sampling event correlate only when all three IDs match. No
generation-to-presentation duration is reported: the Engine has no validated
timestamp in Metal's host-time clock domain.
`frame_skipped_count` is not reported: coalescing before installation does not
account for every possible skipped or never-produced update. Per-session frame
samples are bounded to 384 and drawable presentation events to 768; the report
preserves both eviction counts. The event capacity covers a typical 5.5 second
animation capture at 120 Hz. A short 150 ms tail after animation completion gives
normal in-flight presentation handlers time to report before the snapshot, but
cannot guarantee feedback for arbitrarily delayed or occluded work.

## Commands and workloads

```sh
velocity bench responsiveness \
  --browser photon \
  --workload animation \
  --iterations 10 \
  --warmup 1 \
  --output results/animation.json
```

Supported Photon workloads are `task-backlog`, `dom-style`, `style-localized`,
`style-descendant`, `style-sibling`, `style-relational`, `layout-updates`,
`gc-pressure`, and `animation`. Each measured iteration uses a fresh isolated
profile and the same managed Photon launch path. Velocity verifies fixture
completion and correctness before treating a run as successful. It also
collects process CPU, physical footprint, resident memory, process attribution,
and measurement provenance. These are process-session measurements, not
per-tab memory or JavaScript-heap measurements.

`animation` runs five deterministic 900 ms phases: CSS transform, CSS opacity,
JavaScript-driven `requestAnimationFrame`, DOM changes during animation, and
visible transform animation under allocation pressure. It uses a fixed 900 ms
pre-roll so Velocity can start diagnostics after fixture readiness; if the
first drawable gap matches that known pre-roll, it is retained in the report as
`excluded_pre_roll_interval_ms` and excluded from active-animation interval
statistics. The report gives the fixture's active window, callback count, and
browser-observable rAF interval distribution separately from Metal
presentation intervals. rAF callback timing is never used as a substitute for
presentation timing. The fixture does not inject native input and cannot
establish input-to-presentation latency.

The targeted style fixtures each mutate a fixed 16-branch by 32-item nested
tree 1,024 times, isolating one dependency pattern: one element's class, an
ancestor class and descendant selector, adjacent/general sibling selectors,
or a descendant mutation affecting an ancestor through `:has()`. Every 65
mutations is followed by a computed-style read, and a final read covers the
remaining mutations. Each read follows an odd number of toggles so the sampled
class state changes and the fixture exercises style processing. Correctness
checks verify the resulting computed custom properties. These are synthetic
diagnostic fixtures, not representative application pages. Fixture revision 2
changes the read cadence; reports from revisions 1 and 2 are not directly
comparable. Velocity does not yet include reproducible dashboard, feed,
document, or component-app fixtures.
The other fixtures retain their fixed definitions: 512 zero-delay timer tasks;
1,024 DOM/style mutations on a fixed 512-item nested tree; 512 geometry updates
across normal-flow and absolutely positioned elements; and 800,000 logical
allocation records over 32 cycles. The GC workload does not force collection.

Options are `--iterations` (1–100, default 10), `--warmup` (0–20, default 1),
`--timeout` seconds (1–600, default 60), `--diagnostics enabled|disabled|balanced`
(default `enabled`), `--seed` (balanced mode only), `--output FILE.json`, and
`--no-record-history`. Only Photon is supported for these Engine/frame
diagnostics. Chrome and Helium are not reported as comparable because they do
not expose equivalent task, style/layout, GC, or drawable-presentation data.

## Frame interval and stutter semantics

`frame_interval_p50_ms`, `frame_interval_p95_ms`, `frame_interval_p99_ms`, and
`frame_interval_max_ms` are computed from every drawable presentation event
with a valid Metal timestamp, ordered by `presentedTime`. A repeated display
of the same engine surface remains an interval sample because it is a separate
window drawable presentation; reuse is reported separately. The intervals
describe window drawable presentations and do not prove that each presented
drawable contained changed browser pixels. Report-level values summarize the
per-iteration percentiles; raw bounded lifecycle and presentation-event samples
remain in JSON for re-analysis. Old Photon builds that advertise frame timing
but not `performance_drawable_presentation_events_v1` do not produce
`frame_interval_*` metrics.

The report classifies a run as `missing_presentation_evidence`,
`insufficient_frame_samples` (fewer than five intervals),
`irregular_frame_delivery` (at least one interval exceeds twice that run's
median), or `consistent_frame_delivery`. This relative outlier rule is a
screening heuristic, not a claim that a refresh slot was missed. It does not
assume 60 Hz and does not infer dropped frames from nominal refresh rate. Frame
ID consistency, installed-frame feedback coverage, reused displays, coalesced
updates, zero/invalid presented-time feedback, callback ordering, refresh-rate
changes, and frame/event sample-capacity evictions are shown separately. Missing
evidence remains missing;
Velocity does not synthesize intervals from rAF callbacks, display-link ticks,
GPU completion, or frame production.

The reports can align available Engine task wait, style/layout activity, GC
mark/sweep timings, and Photon surface/Metal stages by iteration. Such
co-occurrence is correlation only; the current events do not prove which stage
caused a long interval. Task backlog is timer-source work and is not an input
latency workload.

## Diagnostics overhead experiment

Use balanced mode to replace a diagnostics-off block followed by a diagnostics-
on block:

```sh
velocity bench responsiveness \
  --browser photon --workload task-backlog \
  --diagnostics balanced --iterations 10 --warmup 1 \
  --seed 88271498792782 \
  --output results/task-overhead-paired.json
```

Each warm-up round and measured pair runs diagnostics off and on in a seeded
random order. Each run uses a fresh private browser profile, the same fixture,
managed launcher, process sampler, and Photon window creation path. Pair IDs and
the observed order are retained in raw samples. The fixed default seed is
`88271498792782`; pass `--seed` to replay a different order. With `N` measured
iterations and `W` warm-ups, balanced mode executes `2N` measured sessions and
`2W` warm-up sessions.

The JSON `diagnostic_overhead_assessment` and HTML table report on-minus-off
absolute paired differences for fixture duration, process CPU mean, physical
footprint mean/peak, and resident-memory mean/peak when attribution is complete.
They include medians, means, relative differences, and a deterministic 10,000
resample percentile-bootstrap 95% interval for the median paired difference
when there are at least five complete pairs. They do not label any metric as a
regression or use an independent-sample significance test. Pairing blocks
nearby runs against local time drift, but sessions do not share browser state;
thermal state, background processes, display occlusion, and window/viewport
geometry remain possible confounders. The responsiveness report records OS,
architecture, machine model, CPU model, logical CPU count, known display rate,
and the actual mode order/seed when available. Window geometry and system
activity are not currently recorded. Process resource readings are collected
in both diagnostic modes.

Diagnostics-on controls enable the authenticated fixed-schema performance
session and frame-timing capability. Photon exposes bounded process/window
snapshots through its authenticated automation protocol; the interface does
not accept page JavaScript or arbitrary commands. `performance_frame_snapshot`
is capability-gated by `performance_frame_timing_v1`; actual drawable
intervals additionally require `performance_drawable_presentation_events_v1`.
Diagnostics are not started for ordinary browsing by these benchmark runs.

## Metric semantics

| Metric | Scope and unit | Meaning |
| --- | --- | --- |
| `task_queue_wait_*_ms` | WebContent process, ms | Nearest-rank enqueue-to-dispatch distribution over retained completed tasks. |
| `timer_task_queue_wait_p95_ms` | WebContent process, ms | Timer-source queue-wait p95 for that iteration. |
| `task_execution_duration_p95_ms` | WebContent process, ms | Task wall time including its microtask checkpoint; not JS-only time. |
| `style_update_duration_ms` | Active WebContent documents, summed elapsed wall-time ms | Time around each `update_style()` call, including transaction setup/submission, Rust style planning and reaction application, remainder, and the pre-style embedding-container layout check. Summed document counters can include overlapping nested work. It is not CPU time or a disjoint style-only phase. |
| `style_update_submission_duration_ms`, `style_update_bridge_duration_ms`, `style_update_apply_duration_ms`, `style_update_remainder_duration_ms` | Active WebContent documents, summed elapsed wall-time ms | Existing phase counters and remainder. Remainder can include incidental C++ work; rounded phases may not sum exactly to the total. Do not treat style and layout totals as mutually exclusive. |
| `style_reaction_elements`, `style_reaction_batches` | Active WebContent documents, counts | Published style-reaction entries and application passes containing published entries. They do not count every element visited or every computed style recomputed, and batches are not transactions. |
| `style_engine_transaction_count`, `style_unscoped_transaction_count`, `style_unscoped_transaction_reaction_entries` | Active WebContent documents, counts | Transactions with non-empty published reactions, transactions whose existing `is_scoped` flag is false, and their reaction entries. Unscoped does not imply a measured full-document traversal. |
| `style_broad_matching_batch_count`, `style_narrow_matching_batch_count`, related reaction elements and apply durations | Active WebContent documents, counts/ms | The existing Rust heuristic classifies batches as broad when unscoped or dense enough to use cold matching scratch. Reaction counts are published entries, not visited nodes. Broad matching is not a full-document style-recomputation count. |
| `style_fallback_reason_available` | Boolean | Currently `false`. Ladybird has no reliable bounded cause classification for broad/unscoped transactions, so Velocity does not infer reasons from selectors or mutations. |
| `layout_full_duration_ms`, `layout_partial_duration_ms` | Active WebContent documents, summed elapsed wall-time ms | Opt-in time inside classified full/partial render-state layout rounds, including round work and committed-layout preparation. Excludes waiting before the render-state job and paint recording. This is elapsed time, not CPU time. |
| `full_layout_count`, `partial_layout_count` | Active WebContent documents, counts | Existing full/partial layout pass counters, distinct from layout duration. |
| `gc_mark_duration_*_ms` | Main-thread JS heap, ms | Root gathering and live-cell marking duration per observed collection. |
| `gc_synchronous_sweep_duration_*_ms` | Main-thread JS heap, ms | Synchronous finalization/weak processing/sweep portion; timer-driven incremental sweep is excluded. |
| `gc_pause_duration_*_ms` | Main-thread JS heap, ms | Duration of the collection call, including completion of prior incremental sweep where needed. |
| `gc_live_heap_bytes` | Main-thread JS heap, bytes | Live cell plus external bytes after completed sweep; not process memory. |
| `surface_handoff_to_install_p50_ms`, `surface_install_to_gpu_submission_p50_ms` | Photon process, ms | Differences between local Photon monotonic timestamps at the named stages. |
| `gpu_sampling_submission_to_completion_callback_p50_ms` | Photon process, ms | Difference between submission and completion callback timestamps for one command buffer sampling the same Engine surface into the same drawable. Not a GPU hardware duration or visible latency. |
| `frame_gpu_completion_callback_count` | Photon session, callbacks | Number of frame records with a Metal command-buffer completion callback timestamp. |
| `frame_zero_presented_time_event_count` | Photon session, events | Presented-handler callbacks whose drawable timestamp was zero/invalid; absent handlers are not inferred as drops. |
| `frame_interval_*_ms` | Photon window drawable, ms | Differences between actual `presentedTime` values for each drawable event, including repeated displays of one engine surface. |
| `frame_actual_drawable_presentation_count` | Photon window drawable, count | Valid Metal presented-handler events in the bounded event stream. |
| `frame_reused_count` | Photon window drawable, count | Additional presentation events that display an engine surface ID already observed in the session. |
| `frame_presentation_event_retention_percent` | Photon timing buffer, percent | Retained drawable events divided by retained plus evicted events. Lower than 100% means the bounded event ring truncated early presentations. |
| `raf_interval_*_ms` | Fixture, ms | `requestAnimationFrame` callback intervals on the page's `performance.now()` clock. |
| `process_tree_physical_footprint_*_bytes` | Verified managed process tree, bytes | macOS physical-footprint readings across attributed processes; not per-tab or unique shared memory. |
| `process_tree_resident_*_bytes` | Verified managed process tree, bytes | RSS readings, which may include shared pages. |
| `process_tree_cpu_mean_percent` | Verified managed process tree, percent | CPU reported by Velocity's process monitor for the verified session tree. |

Engine task timestamps use a process-local monotonic clock. The Engine retains
at most 1,024 task samples and tracks at most 4,096 outstanding instrumented
tasks per WebContent process. GC timing retains at most 256 samples. Style and
layout counters are reused rather than duplicated. Metrics are not merged
across multiple WebContent processes when percentile scopes cannot be
reconstructed; incomplete and unavailable values are omitted, not replaced
with zero.

## Baselines, history, and comparison

Successful reports are imported into Velocity history by default. Use the
existing named-baseline workflow to compare the same fixture before and after
an Engine change:

```sh
velocity baseline set \
  --name animation-before \
  --from results/animation-before.json \
  --browser photon \
  --label "before animation-path change"

velocity bench responsiveness \
  --browser photon --workload animation \
  --iterations 10 --warmup 1 \
  --output results/animation-after.json

velocity compare responsiveness \
  --baseline photon-phase10-1-animation-20261018 \
  --current results/animation-after.json \
  --threshold-percent 5
```

Comparison checks browser, OS/architecture, report schema, fixture revision, workload
parameters, diagnostic schema/capacity, iteration counts, and process-resource
collection provenance. New reports use responsiveness schema 2; schema 1 reports
remain importable for history, but they cannot be compared against schema 2. The
animation fixture is revision 2 after adding visible motion during allocation
pressure and explicitly accounting for its 900 ms pre-roll. At least five successful measured samples are required;
diagnostic comparisons also need sufficient complete single-process snapshots.
Balanced overhead reports are intentionally rejected by the independent
baseline comparison command; their embedded paired assessment is the correct
analysis for that experimental design. Unknown Engine/executable revisions
remain `null`. Current Photon automation does not attest the executable's
source revision, so a dirty checkout's Git HEAD must not be presented as the
revision inside the benchmarked executable.

## First local baseline capture

On 2026-10-18, the Photon Development release build was measured on macOS ARM64,
Mac17,4 / Apple M5 (10 logical CPUs), with a reported nominal display rate of
60 Hz. Each workload used a fresh private profile, one warm-up, and five
measured iterations, except animation, which requested ten and completed nine.
The executable and Engine revisions were unknown to automation and remain
`null`; the working checkout was dirty, so its Git HEAD is not used as binary
provenance. Named baselines are stored in this machine's Velocity history. JSON
and HTML reports are in `tools/velocity/results/phase10.1/frame-events/`.

| Named baseline | Valid runs | Median workload time | Measured diagnostic signal | Process-tree footprint mean / RSS mean (MiB; animation peak shown) |
| --- | ---: | ---: | --- | ---: |
| `photon-phase10-1-task-backlog-20261018` | 5/5 | 21.9 ms | Timer queue wait p50 9.079 ms, p95 15.837 ms, p99 16.428 ms; enqueue backlog mean 255 | physical footprint 388.4 MiB; RSS 423.9 MiB |
| `photon-phase10-1-dom-style-20261018` | 5/5 | 46.9 ms | Style update 42.411 ms; 1,560 reaction elements, 17 batches; one full layout | physical footprint 386.5 MiB; RSS 428.4 MiB |
| `photon-phase10-1-layout-updates-20261018` | 5/5 | 149.8 ms | Style update 31.129 ms; 257 full and 256 partial layout passes; no layout duration metric | physical footprint 380.3 MiB; RSS 420.1 MiB |
| `photon-phase10-1-gc-pressure-20261018` | 5/5 | 57.2 ms | Five collections; mark p95 2.990 ms; collection pause p95 3.512 ms; live heap 10.44 MB | physical footprint 385.4 MiB; RSS 456.6 MiB |
| `photon-phase10-1-animation-20261018` | 9/10 | 4,539.7 ms active window | Drawable interval p50 16.667 ms, p95 16.667 ms, p99 83.333 ms, maximum 133.333 ms; median seven intervals per run exceeded twice that run's median | physical footprint 400.0 MiB; peak 443.3 MiB; RSS 415.4 MiB |

The animation run retained all presentation events for its nine successful
iterations and had 99.5% mean presented-time feedback coverage. At 60 Hz, the
16.667 ms median interval matches one nominal refresh interval, while the tail
shows irregular drawable delivery. The outlier rule is a relative screening
heuristic; it does not prove a missed refresh slot, changed browser pixels, or
the cause of the delay. One requested animation run did not finish within its
60 second fixture deadline and is excluded from the statistics.

The memory values above are sums over the verified Photon process session tree;
they are neither per-tab measurements nor unique physical memory. Physical
footprint peak was 443.3 MiB for animation; the other workload peak values
remain in their raw reports. Style and layout counters do not give layout
duration, and animation produced no GC collections, so these runs do not
establish a frame-to-GC relationship.

Two corrected overhead assessments used ten seeded randomized off/on pairs
(seed `88271498792782`) and one warm-up pair per workload. Each completed all
20 measured sessions. The animation pair is more relevant to frame diagnostics:
its median paired duration difference (on minus off) was +9.25 ms with a paired
bootstrap 95% interval of [−19.6, +31.8] ms. The paired median difference for
process-CPU-mean was +0.244 percentage points [−0.328, +0.750]; for the
process-footprint-mean metric it was −521,046 bytes [−9,180,581, +2,931,191];
and for RSS-mean it was −156,349 bytes [−809,519, +478,860]. All intervals
include zero. The animation report's frame intervals use diagnostics-enabled
sessions only; off sessions intentionally install no Metal presentation
handlers, so the paired assessment does not compare frame intervals.

The corrected task-backlog assessment's duration difference was −0.70 ms
[−2.15, +1.65] ms; process-CPU-mean was +0.284 percentage points
[−0.072, +1.308]; process-footprint-mean was +61,452 bytes
[−305,128, +1,628,168]; and RSS-mean was +90,112 bytes
[−421,888, +585,728]. These intervals also include zero. The results do not
establish a material overhead or rule out smaller effects, effects in other
workloads, or environmental confounders. The earlier sequential CPU difference
is not treated as an instrumentation effect. Full paired samples and peak
memory intervals remain in the JSON reports
`task-backlog-overhead-interleaved-final.json` and
`animation-overhead-interleaved-final.json` under
`tools/velocity/results/phase10.1/frame-events/`.

These captures give useful repeatable workload baselines, not a ranking of
Ladybird bottlenecks. The high task queue wait comes from a saturated timer
backlog and does not measure input response. The GC pressure mark pauses were
not correlated to presentation events. Layout pass counts do not show layout
cost. The animation run does confirm irregular Photon window-drawable timing in
this fixture, but current timestamps cannot attribute those intervals to
Engine rendering, Photon work, GPU execution, window occlusion, or display
scheduling. No Ladybird performance optimization is justified by these
measurements alone; first add reliable per-frame correlation between the
drawable timeline and Engine task/style/layout/GC activity, then repeat the
animation and input-response measurements.

## Interpretation, limitations, and bottleneck ranking

The implementation measures engine/process activity and compositor feedback
without changing scheduling, style invalidation, layout, rendering, or GC
behavior. It does not currently measure Engine frame creation/completion
source timestamps, true GPU hardware start/end timestamps, native input to
visible presentation latency, exact viewport/window geometry, per-tab memory,
unique physical memory, or all skipped engine frames. Nominal refresh rate
may be unavailable or may change; variable refresh and occlusion affect
interpretation. `presentedTime` means the window drawable was displayed, not
that it was unobscured or that its browser surface changed.

Rank a Ladybird bottleneck only after workload evidence supports it. Task queue
wait tails, broad style reaction activity, full layout counts, GC marking
pauses, frame interval outliers, and Photon stage durations are diagnostic
signals. A source-level architectural difference alone is not a measured
bottleneck, and timing overlap is not causation. Report a candidate as
unverified if frame or process coverage is insufficient. Do not rank task
scheduling, style invalidation, or synchronous GC marking as a confirmed
bottleneck without repeatable measured impact and interaction relevance.
