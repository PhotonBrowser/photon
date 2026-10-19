# Chromium–Ladybird Performance Analysis

**Reviewed:** 17 October 2026  
**Photon root revision:** `a950612155636c31fa06c6d5b0f702caf5f33caf`  
**Ladybird Engine revision:** `c3e20b5e55c478b5630f06af910ad5784f1ecb22`  
**Chromium source revision:** `58454d73e25bfac1610e92556a37a1c0fc1fa89e` (Chromium `main` observed during this review)

## Scope and evidence

This is source research, not a benchmark report. I reviewed Photon’s Engine submodule, shell integration, and `tools/velocity/`, then used the reviewed `chromium-search` utility to query individual files at the pinned Chromium revision. Chromium was not cloned or downloaded. The utility was fetched from [`jclay/chromium-search`](https://github.com/jclay/chromium-search) at commit `2db9c99d55ce8ffbc9fd9d5f876467e5b966f2a5` (SHA-256 `68a80953e771350dd3ab4d168f189f8fbfa91aaa7b1efbe28e7f5bfaa62f7d2c`), reviewed, and installed as `/Users/theo/.local/bin/chromium-search` with Python 3.13. A symlink in `/opt/homebrew/bin` makes it available on `PATH`; it was not added to the Photon source tree.

Evidence labels used below:

- **Verified:** the described source mechanism or architectural difference is present in the inspected code.
- **Hypothesis:** the difference could affect performance in the stated workload, but no profile or benchmark here confirms it.
- **Measured:** a controlled run confirms the impact. This review produced no measured findings.

The Photon and Engine checkouts were already substantially modified when reviewed. The Engine working tree includes uncommitted performance and memory work. Findings below describe the checked-out source and do not treat those edits as a clean upstream baseline or as measured improvements. Recheck the working-tree diff before using these findings to plan a patch.

## A. Executive summary

Ladybird already has meaningful incremental rendering machinery. DOM and style changes mark layout work through ancestors or at a narrower boundary; layout can run on planned partial-relayout subtrees and records when it falls back to full layout. Style transactions normalize mutation inputs, retain selector answers, and route reactions through impact regions. Painting retains paint-order scopes, copies clean recordings, tracks specific damage, and rasterizes damaged regions. Eligible scrolling is routed through the compositor. These are verified mechanisms, so generic recommendations to “add incremental layout,” “cache computed styles,” “reuse display lists,” or “move scrolling to the compositor” are not supported by this checkout.

Three differences merit focused measurement and possible follow-up:

1. **Task selection does not use the task source as a scheduling priority.** Ladybird’s HTML queue distinguishes normal from idle tasks and scans runnable normal tasks in insertion order; Chromium’s main-thread scheduler has a dedicated input queue and priority selection. This may affect queued interaction work during a normal-task backlog, but it cannot preempt a running JavaScript task, and eligible compositor scrolling already avoids the main thread.
2. **Style transactions can fall back to a complete document reaction batch.** Blink uses selector-feature invalidation sets to target affected descendants. Ladybird has substantial retained and region-based style work too; the source difference only creates an opportunity if common changes reach the broader fallback often enough to matter.
3. **Ladybird’s ordinary GC mark phase is synchronous while its sweep is incremental.** V8 has incremental marking steps and optional concurrent marking. The difference is real; pause reduction in Photon is still a hypothesis, and incremental marking would be a substantial collector project.

No source-backed opportunity was established for replacing Ladybird’s current partial-layout, paint-retention, or compositor-scroll designs. The DOM, CSS-value, and LibJS object representations also do not establish a memory-efficiency gap without comparable size profiles. The best first implementation candidate is a narrowly scoped scheduler improvement for eligible queued input-related tasks, gated by a queue-wait benchmark and strict event-ordering tests.

## Local architecture and boundaries

Photon’s shell owns GPUI views, native window integration, and the host side of the presentation surface. Browser behavior—DOM, CSS, layout, painting, event dispatch, and LibJS—is in the Ladybird Engine checkout. On macOS the shell receives an external Metal surface from the Engine compositor; layout/style changes therefore belong in Engine code, not in shell views. See [`renderer.rs`](../../crates/photon-shell/src/platform/macos/renderer.rs#L1) and [`PhotonEmbedderBridge.cpp`](../../native/embedder/PhotonEmbedderBridge.cpp#L102).

The inspected paths are:

| Area | Local implementation | What the source shows |
| --- | --- | --- |
| DOM invalidation | [`Node.cpp`](../../Engine/Libraries/LibWeb/DOM/Node.cpp#L3944), [`Element.cpp`](../../Engine/Libraries/LibWeb/DOM/Element.cpp#L1744) | Layout dirtiness propagates through ancestors by default; layout-tree rebuilding can be self-scoped, handled in place, or widened to a parent depending on box/top-layer conditions. |
| CSS/style | [`UpdateStyle.cpp`](../../Engine/Libraries/LibWeb/CSS/UpdateStyle.cpp#L111), `Engine/Libraries/LibWeb/Rust/src/css/style/` | Style changes distinguish relayout from tree rebuild; a safe absolute-positioned boundary can use self-only layout invalidation. Style updates are batched and timed. |
| Layout | [`partial_relayout.rs`](../../Engine/Libraries/LibWeb/Rust/src/layout/partial_relayout.rs#L258), [`update_layout.rs`](../../Engine/Libraries/LibWeb/Rust/src/layout/update_layout.rs#L652) | Partial relayout is planned when safe; pending escapes, root dirtiness, container-query work, DevTools collection, or other constraints can select full layout. Full and partial layout counters exist. |
| Paint | [`damage.rs`](../../Engine/Libraries/LibWeb/Rust/src/painting/record/damage.rs#L17), [`assemble.rs`](../../Engine/Libraries/LibWeb/Rust/src/painting/record/assemble.rs#L132), `Engine/Services/Compositor/ContextPresenter.cpp` | Damage is classified by producer/metadata; retained scopes are copied where clean; compositor damage rectangles limit raster work. |
| Events and scrolling | [`TaskQueue.cpp`](../../Engine/Libraries/LibWeb/HTML/EventLoop/TaskQueue.cpp#L36), [`EventLoop.cpp`](../../Engine/Libraries/LibWeb/HTML/EventLoop/EventLoop.cpp#L393), `Engine/Services/Compositor/WheelGestureRouting.cpp` | HTML tasks use normal/idle priority. Page input events are processed during rendering updates; eligible scroll gestures are routed asynchronously in the compositor. |
| Memory and GC | [`Heap.cpp`](../../Engine/Libraries/LibGC/Heap.cpp#L662), [`ElementRareData.h`](../../Engine/Libraries/LibWeb/DOM/ElementRareData.h#L19), `Engine/Libraries/LibWeb/Rust/src/css/style/computed.rs` | GC marking is synchronous and ordinary sweeping incremental. DOM rare data is lazy, CSS records can be shared, and decoded image caches have pruning limits. |

## B. Verified architectural differences and performance hypotheses

### 1. Task scheduling for queued interaction work

**Evidence: Verified scheduling difference; Hypothesis of user-visible impact.**

Chromium’s `SequenceManager` selects a high-priority work queue while accounting for starvation. Blink’s main-thread queue types include a dedicated input queue and allow queue priority to be set. See Chromium [`task_queue_selector.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/base/task/sequence_manager/task_queue_selector.cc#L181) and [`main_thread_task_queue.h`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/platform/scheduler/main_thread/main_thread_task_queue.h#L80).

Ladybird tags tasks with sources such as `UserInteraction`, but the scheduling priority enum is only `Normal` or `Idle`. Normal tasks share one queue; `take_first_runnable()` selects the first runnable item in that queue. Separately, platform/page input events are drained by `process_input_events()` during `update_the_rendering()`. See [`Task.h`](../../Engine/Libraries/LibWeb/HTML/EventLoop/Task.h#L28), [`TaskQueue.cpp`](../../Engine/Libraries/LibWeb/HTML/EventLoop/TaskQueue.cpp#L54), and [`EventLoop.cpp`](../../Engine/Libraries/LibWeb/HTML/EventLoop/EventLoop.cpp#L705).

The gap is specifically the scheduler’s lack of source-aware priority among queued tasks—not an absence of async scrolling or a way to interrupt JavaScript. If eligible interaction tasks sit behind a backlog of runnable normal tasks, queue wait could increase. A running synchronous script still runs to completion, and queued native input is currently consumed as part of a rendering update. Compositor-safe scrolling already avoids waiting for main-thread work in the supported cases. These limits make the real-world impact uncertain.

**Ladybird-native proposal:** first record enqueue-to-dispatch age by task source, queue depth, and rendering-opportunity age. If traces show interaction tasks waiting behind unrelated normal work, add a scheduler selection policy for only the task sources that can safely run ahead at task boundaries. Preserve FIFO within each source and HTML-required ordering; do not interrupt a running task. Keep the input-event queue/rendering path distinct until measurements show that it is the bottleneck.

- **Potential benefit:** lower tail queue wait for eligible input-related callbacks under task backlog.
- **Complexity:** medium to high.
- **Regression risk:** high; changing cross-source order can expose web compatibility and event-order bugs.
- **Benchmark:** deterministic event-loop workload with queued normal tasks and a queued `UserInteraction` task; measure enqueue-to-dispatch p50/p95/p99 and check source-local FIFO. Then compare a browser interaction fixture with and without blocking wheel listeners, recording input-to-presentation latency and frame intervals.

### 2. Style invalidation and broad transaction fallback

**Evidence: Verified difference in invalidation strategy; Hypothesis of excess work.**

When a class changes, Blink uses `RuleFeatureSet` to collect invalidation sets and schedule them for the changed node. The style invalidator checks pending sets and child invalidation bits to prune traversal. See Chromium [`style_engine.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/core/css/style_engine.cc#L1832) and [`style_invalidator.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/core/css/style_invalidator.cc#L236).

Ladybird’s style engine already batches changes, retains selector-match answers, and routes reactions through regions derived from its selector/program inputs. Its C++ flush publishes everything since the prior style boundary as one normalized batch; if the transaction cannot be proven narrower, it can publish a complete document reaction batch. The Rust style planner also has explicit broad/root cases. See [`UpdateStyle.cpp`](../../Engine/Libraries/LibWeb/CSS/UpdateStyle.cpp#L713) and [`flush.rs`](../../Engine/Libraries/LibWeb/Rust/src/css/style/flush.rs#L605). The source does not show that a broad batch necessarily recomputes every computed style, nor how often the fallback is reached.

**Ladybird-native proposal:** use existing counters (`style_update_microseconds`, reaction batch runs/elements, published reactions, and guard hits) to identify which common mutations reach broad batches. Only for a frequent, proven case, extend the existing transaction proof/impact-region route so it can remain narrow. Do not add a second selector cache or copy Blink’s invalidation structures without evidence.

- **Potential benefit:** less selector/reaction traversal and shorter style flushes for affected common mutations.
- **Complexity:** medium; depends on the fallback reason and the Rust planner path involved.
- **Regression risk:** medium to high because missed invalidations break CSS correctness.
- **Benchmark:** repeated class and attribute toggles over a fixed nested DOM with descendant, sibling, state, and relational selectors. Track style-update time, reaction elements, and style/layout output equivalence.

### 3. Stop-the-world GC marking

**Evidence: Verified collector difference; Hypothesis of pause impact.**

Ladybird’s ordinary `Heap::collect_garbage()` captures roots and calls `run_collection()` synchronously. The normal collection gathers roots and calls `mark_live_cells()` before returning; incremental work is applied to sweeping afterward. See [`Heap.cpp`](../../Engine/Libraries/LibGC/Heap.cpp#L662) and [`Heap.h`](../../Engine/Libraries/LibGC/Heap.h#L253).

V8 has incremental marking steps with a time/byte budget and can enable concurrent marking. Its marking job schedules work through a user-visible task runner. See Chromium [`incremental-marking.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/v8/src/heap/incremental-marking.cc#L782) and [`incremental-marking-job.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/v8/src/heap/incremental-marking-job.cc#L46). This comparison is limited to marking: it does not imply V8 has no stop-the-world phases or that every collection is concurrent.

**Ladybird-native proposal:** before changing collection behavior, use existing per-phase GC timing to correlate mark duration with input/frame stalls and live heap size. If marking is a confirmed tail-latency source, investigate incremental marking with explicit write/root barriers and bounded slices, then concurrency only if a single-thread incremental design is insufficient. Preserve conservative stack/root behavior and validate heap correctness under stress.

- **Potential benefit:** lower longest main-thread pause for large live heaps; possibly more predictable interaction latency.
- **Complexity:** very high.
- **Regression risk:** very high; barrier/root omissions can cause use-after-free or retention, and background marking adds synchronization costs.
- **Benchmark:** allocate and retain realistic DOM/JS graphs, trigger collections under an animation/input workload, and report pause distribution by GC phase, allocation throughput, live bytes, and post-collection process footprint.

### Areas where no actionable gap was verified

- **Layout:** Ladybird marks layout through ancestors and supports planned partial relayout. Its eligibility checks explicitly reject conditions that require a full pass, then fall back in the same round. Chromium also propagates dirty state through container chains and can stop at layout boundaries. The strategies differ, but code inspection alone does not show that Ladybird’s safe full-layout fallbacks are unnecessarily broad. Compare Chromium [`layout_object.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/core/layout/layout_object.cc#L1656) with Ladybird [`partial_relayout.rs`](../../Engine/Libraries/LibWeb/Rust/src/layout/partial_relayout.rs#L258) and [`update_layout.rs`](../../Engine/Libraries/LibWeb/Rust/src/layout/update_layout.rs#L652). Measure full/partial counts and reasons before proposing a change.
- **Painting:** Blink reuses cached paint items and subsequences through `PaintController`; Ladybird also reuses retained paint scopes/producers and copies clean ranges, while the compositor tracks and rasterizes damage. This is not evidence that one is more efficient. See Chromium [`paint_controller.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/platform/graphics/paint/paint_controller.cc#L227), Ladybird [`assemble.rs`](../../Engine/Libraries/LibWeb/Rust/src/painting/record/assemble.rs#L168), and [`ContextPresenter.cpp`](../../Engine/Services/Compositor/ContextPresenter.cpp#L422).
- **Scrolling:** Chromium’s compositor input handler requests main-thread hit testing when compositor hit testing cannot target a node. Ladybird has wheel-listener state/generation tracking, latches scroll targets, and routes eligible scroll updates asynchronously. No local scrolling gap was verified. See Chromium [`input_handler.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/cc/input/input_handler.cc#L242) and Ladybird `Engine/Services/Compositor/WheelGestureRouting.cpp` / `AsyncScrollTree.cpp`.
- **DOM and CSS memory:** Ladybird stores common element name/id/class facts directly and creates rare data lazily; style handles refer to StyleEngine records, and computed style records can be reused. This does not establish a per-node or per-style memory disadvantage. See [`Element.h`](../../Engine/Libraries/LibWeb/DOM/Element.h#L1050), [`ElementRareData.h`](../../Engine/Libraries/LibWeb/DOM/ElementRareData.h#L19), and `Engine/Libraries/LibWeb/Rust/src/css/style/computed.rs`.
- **LibJS memory:** the inspected object implementation uses size-classed value storage and shared descriptor arrays across shape transitions. Without matched heap profiles, object-size or fragmentation claims versus V8 would be speculative. See `Engine/Libraries/LibWeb/Rust/src/runtime/object.rs`, `value_storage.rs`, and `descriptor_array.rs`.
- **Image memory:** the document prunes eligible decoded image requests and available images using an 8 MiB / 96-entry cache bound; client-held and CSS-used resources are excluded. This is an existing policy, not evidence of adequate real-world behavior. See [`Document.cpp`](../../Engine/Libraries/LibWeb/DOM/Document.cpp#L7901).

## C. Ranked optimization opportunities

These are ranked by likely relevance to the stated priorities and tractability, not by measured speedup. Impact remains unverified.

| Rank | Optimization | Target | Expected impact | Complexity | Evidence |
| --- | --- | --- | --- | --- | --- |
| 1 | Prioritize eligible queued interaction tasks at task-selection boundaries | LibWeb event loop | Potentially lower tail queue wait under a runnable-task backlog; no effect on a currently running script | Medium–High | Verified scheduler difference; impact Hypothesis. Chromium input queue/priority selector; Ladybird normal/idle queue and `Task::Source`. |
| 2 | Narrow frequent style transactions that currently use a complete document reaction batch | LibWeb CSS/style engine | Potentially fewer style reactions for common mutations; incidence and benefit unknown | Medium | Verified broad fallback and Blink invalidation sets; Hypothesis that the fallback is frequent or costly. |
| 3 | Incrementalize GC marking if profiling confirms mark pauses | LibGC / LibJS | Potentially lower long-pause tail and smoother frames on large live heaps | Very High | Verified synchronous local mark vs V8 incremental/optional concurrent marking; pause reduction Hypothesis. |

## D. Implementation roadmap

### Quick wins

- Add or expose diagnostic counters for task enqueue-to-dispatch wait by task source, plus queue depth and frame/input correlation. This is measurement plumbing rather than a claimed optimization.
- Run the existing style and layout counters on controlled mutation fixtures. Record style reaction batch size, update time, and full/partial layout counts before changing invalidation rules.
- Extend Velocity’s resource/fixture workloads to retain per-run environment metadata and repeated-interaction phases where needed. Keep results as hypotheses until repeated runs show a stable effect.

### Medium improvements

- If event-loop traces confirm queueing delay, prototype a source-aware selection policy for narrowly defined interaction tasks. Preserve FIFO within a source, keep task execution non-preemptive, and test ordering across task sources.
- If style counters show repeated broad reactions for one common mutation, extend Ladybird’s existing transaction/impact-region proof for that case. Gate rollout on selector, pseudo-class, shadow-tree, and relational-selector correctness tests.

### Major investments

- Incremental GC marking is a collector redesign requiring barriers, bounded work slices, root handling, and heap stress validation. Concurrency is a later option, not the initial design.
- Broader scheduler redesign or changes to the separation between page input queues and rendering opportunities require event-order and platform-input review. Consider them only if the narrower queue-selection experiment leaves measured input latency unexplained.

The measurement work comes first because each candidate’s measured bottleneck is unknown. Style narrowing and task scheduling can proceed independently once their own traces identify the hot path. GC marking should depend on phase-level evidence that marking, rather than script execution, layout, or image work, accounts for long pauses.

## E. Top three recommendations

### 1. Source-aware scheduling for eligible interaction tasks

1. **Chromium:** maintains a dedicated main-thread input queue and a priority selector with starvation protection.
2. **Ladybird:** records `UserInteraction` as a task source, but normal tasks share one FIFO-like runnable queue; platform input events are drained during rendering updates.
3. **Change:** introduce a narrow priority class or queue for eligible interaction tasks at task boundaries. Do not preempt JavaScript, and do not silently reorder events within a source.
4. **Likely files:** `Engine/Libraries/LibWeb/HTML/EventLoop/Task.h`, `TaskQueue.h`, `TaskQueue.cpp`, `EventLoop.cpp`; possibly the task-queue tests under `Engine/Tests/LibWeb/` after locating the relevant harness.
5. **Steps:** add queue-age instrumentation; identify which task sources are safe to prioritize; add a scheduler policy with FIFO per source and starvation limits; add ordering and suspension/runnability tests; measure an input backlog fixture; only then validate an end-to-end browser interaction.
6. **Focused benchmark:** enqueue a fixed number of normal tasks, inject an eligible user-interaction task at controlled queue positions, and record its enqueue-to-dispatch latency across repetitions. Follow with a long scrolling page using both compositor-safe input and a blocking wheel listener; measure input-to-presentation time and frame-interval p50/p95/p99.
7. **Success criteria:** lower p95/p99 dispatch wait under backlog without increasing starvation or changing required same-source order; demonstrate an end-to-end improvement before claiming smoother real-world input. No material regression when the queue is not backlogged.
8. **Risks:** cross-source ordering can be web-observable; input-event queue processing may remain the dominant delay; a high-priority queue cannot interrupt a long-running script. This is why the first change should be narrow and benchmark-driven.

### 2. Narrow broad style-reaction transactions

1. **Chromium:** derives invalidation sets from selector features and prunes unaffected descendants during style invalidation.
2. **Ladybird:** batches and normalizes style inputs, retains selector answers, and routes reactions through impact regions, but can publish a complete document reaction batch if a narrower route is not proven.
3. **Change:** target one measured fallback reason at a time and extend the existing Rust style planner to preserve a narrow reaction region where correctness can be proven.
4. **Likely files:** `Engine/Libraries/LibWeb/CSS/UpdateStyle.cpp`, `Engine/Libraries/LibWeb/Rust/src/css/style/flush.rs`, the specific transaction/input-routing module identified by the fallback trace, and `Engine/Libraries/LibWeb/DOM/Document.h` only if an additional counter is needed.
5. **Steps:** collect counters for common class/attribute/state mutations; identify broad-batch cause and affected element count; add narrow planning for one cause; validate cascade and style output; compare the same workload against baseline.
6. **Focused benchmark:** repeatedly toggle a class on one element in a fixed DOM with descendant, sibling, state, and `:has()` rules. Record style update duration, reaction elements, and downstream layout/paint counts.
7. **Success criteria:** fewer reaction elements and lower p95 style time for the target workload, with identical computed-style/render output and no regression on broad stylesheet changes.
8. **Risks:** overly narrow invalidation can leave stale computed styles; benefits may be negligible if broad batches are rare or already cheap.

### 3. Incremental GC marking, only after pause evidence

1. **Chromium/V8:** supports incremental marking steps and optional background concurrent marking.
2. **Ladybird:** performs the mark phase synchronously for normal collection and makes sweep incremental.
3. **Change:** investigate bounded incremental mark slices first, with explicit barriers/root handling; do not start with concurrent marking.
4. **Likely files:** `Engine/Libraries/LibGC/Heap.cpp`, `Heap.h`, cell/barrier code in `Engine/Libraries/LibGC/`, and LibJS allocation/GC integration as required by the design.
5. **Steps:** correlate existing GC phase timings with frame/input traces; build representative live-graph workloads; define root/barrier invariants; implement bounded mark work; stress allocations and mutation during incremental cycles; compare total throughput and retained bytes as well as pause time.
6. **Focused benchmark:** sustained JS/DOM allocation with a long-lived graph while recording frame intervals, input latency, per-phase GC time, allocation throughput, and post-GC live bytes.
7. **Success criteria:** lower worst-tail mark pause without unacceptable throughput loss, live-memory growth, or correctness failures under stress.
8. **Risks:** missed roots/barriers can free live objects; scheduling mark work adds CPU contention; less frequent/partial marking can increase peak memory. This has the highest implementation and validation cost of the three.

## F. Velocity integration and benchmark plan

### What Velocity measures today

Velocity already supports startup timing, resource monitoring, and repeatable idle/page/tab resource workloads. Resource sampling records process CPU time, macOS physical footprint, resident bytes, process identity/role, and coverage at a default 100 ms interval. Multi-tab scenarios report process-tree memory scaling; they do not claim an isolated memory cost per tab. Aggregate RSS can double-count shared pages, and sampled peaks can miss short transients. See [`README.md`](../../tools/velocity/README.md#L274) and [`resource.rs`](../../tools/velocity/crates/velocity-metrics/src/resource.rs#L182).

Fixtures include simple, complex, and deterministic dynamic pages. The current resource fixture does not isolate DOM mutation throughput, style recalculation, layout, scrolling, paint, or interaction latency. Velocity’s startup `window_ready_ms` is a stable native window observation, not page readiness or navigation latency. The README identifies navigation-latency benchmarking and system-cache control as not implemented. A Speedometer 3.1 benchmark path and restricted Photon result observer exist; Photon’s authenticated protocol does not expose arbitrary JavaScript evaluation or input injection. See [`speedometer_cli.rs`](../../tools/velocity/crates/velocity-cli/src/speedometer_cli.rs#L550), [`photon.rs`](../../tools/velocity/crates/velocity-browser/src/photon.rs#L445), and [`README.md`](../../tools/velocity/README.md#L346).

The inspected Engine has style update and reaction counters, partial/full layout counters, per-phase GC timing, and paint damage/retention mechanisms. These are internal engine diagnostics; they are not currently reported by Velocity’s authenticated automation protocol. Photon’s current uncommitted performance work includes process and managed-heap monitoring, but Velocity does not read a reliable per-tab heap metric from that work. When several documents share a WebContent process, dividing process or heap bytes evenly among tabs is only an attribution heuristic, not a per-tab measurement.

### Proposed workload set

| Question | Repeatable workload | Required measurement | Current support / next step |
| --- | --- | --- | --- |
| DOM/style cost | Toggle class and data attributes over fixed DOM and selector families | Mutation throughput, reaction elements, style duration | Fixture can be served today; internal counters need an authenticated diagnostic export or a focused Engine benchmark. |
| Layout cost | Mutate isolated absolute-positioned and normal-flow descendants | Partial/full layout counts, layout duration, dirty-boundary count | Existing counters help; add fallback-reason breakdown if not already exposed. |
| Paint reuse | Alternate transform/opacity/paint-only/scroll changes | New versus copied paint producers/scopes, damage pixels/rectangles, paint/raster time | Paint mechanisms exist; expose timing/copy counters for the benchmark. |
| Scrolling | Long page with compositor-safe path and a blocking wheel listener | Frame interval distribution and input-to-present latency | Needs controlled input injection and a presentation timestamp; frame production time is not presentation latency. |
| GC pauses | Allocate and retain JS/DOM graphs during animation | Per-phase pause, GC pause distribution, heap live bytes, frame/input tail | Per-phase logging exists; structured export and repeatable workload needed. |
| Multi-tab memory | 1/5/10 tabs plus repeated create/navigate/close cycles | Process-tree physical footprint/RSS time series and actual per-process engine live bytes | Existing OS measurements and tab fixtures cover process memory; per-tab heap requires document-level accounting, not equal division. |

Add diagnostics through an explicit versioned, authenticated automation capability with fixed fields and bounded output. Keep the layers separate:

- **Browser-observable:** event-to-visible/present latency and frame-interval distribution.
- **Engine-internal:** style, layout, paint, input dispatch, and GC phase timings; DOM node count; LibJS heap live bytes.
- **OS resource:** existing process CPU, physical footprint, and RSS, reported as process/session totals.
- **Photon compositor:** frame received/installed and GPU presentation completion if the platform can provide it. Do not label frame production time as presentation latency.

The proposed field names (`layout_duration_ms`, `style_recalculation_ms`, `paint_duration_ms`, `input_processing_ms`, `frame_interval_ms`, `gc_pause_ms`, `dom_node_count`, `js_heap_bytes`, and `decoded_image_bytes`) are diagnostic ideas, not current Velocity or automation capabilities. `js_heap_bytes` must be explicitly defined; it is not process footprint. A per-tab metric is only valid if the engine can attribute it to documents even when they share a process.

Use the same machine, OS/build, display refresh rate, viewport, fixture revision, and benchmark order policy for before/after runs. Warm up, repeat runs, compare medians and tail distributions, and retain raw samples. Avoid competing browser runs. For memory, report time series and cleanup behavior in addition to sampled peaks. A generated frame does not prove the user saw that frame; use an actual presentation signal for presentation latency.

## G. First optimization to implement

**First candidate:** add scheduler-level priority for a narrowly defined set of eligible queued interaction tasks, selected only at task boundaries.

This is the best first implementation candidate because interaction latency is the highest stated priority, Chromium provides direct evidence for input-aware queue selection, and Ladybird’s queue exposes task source and priority as separate concepts. It can be attempted incrementally without changing layout, CSS, paint, or GC algorithms. It is also straightforward to falsify: if the queue-wait fixture shows no delay behind runnable normal work, do not implement a priority change.

The first patch should keep `UserInteraction` tasks FIFO within their source, maintain starvation limits, preserve the existing page input-event/rendering flow, and never interrupt synchronous JavaScript. Benchmark queue wait and browser-level input-to-presentation latency before and after. Treat it as a measured improvement only if both scheduler-level and real interaction data support that conclusion.

## References

- Chromium source queries used pinned revision `58454d73e25bfac1610e92556a37a1c0fc1fa89e` through Gitiles individual-file reads; Chromium was not cloned.
- Chromium search utility: [`jclay/chromium-search`](https://github.com/jclay/chromium-search), reviewed at `2db9c99d55ce8ffbc9fd9d5f876467e5b966f2a5`.
- Local Engine references point at the checked-out Ladybird revision `c3e20b5e55c478b5630f06af910ad5784f1ecb22`; the working tree was modified during this review.
