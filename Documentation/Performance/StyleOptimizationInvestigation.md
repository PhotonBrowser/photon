# Phase 10.2 — Style Optimization Investigation

**Reviewed:** 18 October 2026  
**Photon checkout base:** `a950612155636c31fa06c6d5b0f702caf5f33caf` (dirty)  
**Engine checkout base:** `c3e20b5e55c478b5630f06af910ad5784f1ecb22` (dirty)  
**Velocity executable:** Photon Development (release), `/Users/theo/Documents/code/oss/photon/target/release/Photon`  
**Executable and Engine revisions in benchmark reports:** unknown; the authenticated protocol does not attest a binary build revision.

## Decision

**No Ladybird style algorithm optimization was selected or implemented.** The new diagnostics and repeatable fixtures show measurable style cost, but do not show that the measured reaction work is unnecessary or can safely be narrowed.

The most useful next investigation is the repeated full-layout path in the geometry workload. It accumulated 88.6 ms of full-layout round time over 256 full passes, compared with 5.9 ms over 256 partial passes. That is a strong synthetic workload signal, not proof of a real-site bottleneck. For style, the next step is mutation-specific tracing for dense matching batches and a locally hosted application fixture.

Evidence labels in this report mean:

- **Verified:** confirmed by source inspection or a successful fixture correctness check.
- **Measured:** observed in the recorded runs below; limited to the named fixture and machine.
- **Hypothesis:** a possible implication that still needs a representative workload or a causal trace.

## Measurement semantics

The old `style_update_duration_ms` value is elapsed wall time summed across active WebContent documents. It times calls to `update_style()` and includes transaction setup/submission, the Rust style bridge and reaction application, remainder work, and the pre-style embedding-container layout check. It is not CPU time. Nested document work can overlap in the sum, so this value is not an exclusive phase budget.

The new counters keep several different operations separate:

| Metric | Definition | Does not mean |
| --- | --- | --- |
| `style_engine_transaction_count` | Transactions with non-empty reactions consumed during the diagnostic interval. | Number of DOM mutations or elements visited. |
| `style_reaction_batches` | Style application passes containing published reactions. | Transactions or full-document traversals. |
| `style_reaction_elements` | Published style-reaction entries. | Unique nodes visited or computed styles recomputed. |
| `style_broad_matching_batch_count` | Passes using the current Rust broad-matching scratch strategy. | A full-document reaction batch or recomputation. |
| `style_unscoped_transaction_count` | Consumed transactions whose existing `is_scoped` flag is false. | A count of broad-matching passes. |
| `style_broad_matching_apply_duration_ms` | Existing pass wall timer accumulated for broad-matching passes. | CPU time or the cost of the entire style update. |
| `layout_full_duration_ms`, `layout_partial_duration_ms` | Elapsed time within classified full/partial render-state layout rounds, including committed-layout preparation. | Paint recording, time waiting to enter the render-state job, or CPU-only time. |

Style and layout durations can overlap through nested update work. They must not be added to claim a complete or exclusive breakdown. Painting duration remains unavailable. Style diagnostics currently do not expose per-transaction cause, mutation identity, or an update-cycle ID that can be joined to a later surface or drawable event.

### Source locations inspected

| Area | Source and relevant code | What it establishes |
| --- | --- | --- |
| Style transaction timing and counts | `Engine/Libraries/LibWeb/CSS/UpdateStyle.cpp` (`Document::update_style`, `web_css_take_style_transaction`) | The existing update timer wraps document update work; diagnostic transaction counts increment when a non-empty transaction is consumed. |
| Broad versus narrow matching | `Engine/Libraries/LibWeb/Rust/src/css/style/reaction_application.rs` (`Application::take_transaction`, `update_style`) | Broad matching is selected for unscoped work or reaction density above one sixteenth of connected elements; it selects a matching strategy. |
| Transaction scope planning | `Engine/Libraries/LibWeb/Rust/src/css/style/flush.rs` (`plan_is_broad`, transaction publication) | Document, tree-scope, and root subtree regions can produce an unscoped transaction. This is distinct from the Rust density heuristic. |
| Layout rounds | `Engine/Libraries/LibWeb/Layout/UpdateLayout.cpp`, `Engine/Libraries/LibWeb/Rust/src/layout/update_layout.rs`, `Engine/Libraries/LibWeb/Rust/src/layout/layout_node_arena.rs` | Full/partial counts already existed; opt-in elapsed timers wrap the classified render-state round and committed-layout preparation. |
| Diagnostic data and export | `Engine/Libraries/LibWeb/CSS/PerformanceDiagnostics.h`, `Engine/Services/WebContent/ConnectionFromClient.cpp` | Per-document counters are collected only during an authenticated diagnostics session and exported without page content. |
| Surface and drawable stages | `crates/photon-shell/src/platform/macos/presentation.rs`, `crates/photon-shell/src/platform/macos/engine/session.rs`, `vendor/gpui-ce/crates/gpui_apple/src/metal_renderer.rs` | Photon reuses the Engine surface publication identity through installation and drawable callbacks; drawable feedback uses Metal host time. |
| Metric validation and report | `tools/velocity/crates/velocity-browser/src/photon.rs`, `tools/velocity/crates/velocity-cli/src/responsiveness_cli.rs` | Velocity validates the tab-scoped event stream, retains timing coverage, and reports drawable intervals separately from Engine style timing. |

The implementation reuses Ladybird's existing style and layout counters, adds broad/narrow apply counts and durations only while authenticated diagnostics are enabled, and times classified layout rounds behind the same opt-in session. Rust diagnostics do not gather per-element traces while disabled. Unscoped/fallback cause reporting remains unavailable; `style_fallback_reason_available` is explicitly `false`.

## Engine-to-presentation correlation

**Verified lifecycle:** WebContent and the Engine compositor update an IOSurface-backed surface; `LibPhotonEmbedder` publishes `(backing-pool generation, WebView frame ID)`; Photon receives and may coalesce updates before installation; the Photon page view installs a selected surface; GPUI-CE samples it into a Metal window drawable; Metal returns `CAMetalDrawable.presentedTime` feedback.

The generation is an IOSurface pool epoch. The frame ID is a monotonically increasing surface-publication sequence within one WebView. This identity is not a content generation, does not prove pixel changes, and does not prove the window was visible. Navigation retaining the WebView keeps its sequence; a replacement WebView starts a new diagnostic scope. Repeated drawable presentations of one surface are retained separately, and pre-install coalescing is counted.

Photon elapsed stage timestamps use one process-local monotonic origin. Metal `presentedTime` is kept in its host-time domain. There is no validated Engine-to-Metal shared timestamp, so generation-to-presentation duration remains `null`. GPU submission and command-buffer completion are correlated by `(generation, frame ID, drawable ID)`; completion is not presentation. No interval is synthesized when presentation feedback is absent.

The live run uncovered an old Velocity envelope check expecting `photon_window_engine_surface_frames` while Photon returns the correctly tab-scoped `photon_tab_engine_surface_frames`. The validator now checks the tab scope, which restored the already advertised per-drawable event stream. The short style fixtures usually produce only one or two drawable events, so they do not provide frame intervals. The animation fixture supplies enough drawable events for interval distributions.

## Chromium comparison

Chromium source was accessed remotely at revision [`58454d73e25bfac1610e92556a37a1c0fc1fa89e`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/); no Chromium checkout was created.

**Verified Blink technique:** Blink derives selector-feature invalidation sets and propagates pending invalidation state so style invalidation can target descendants and sibling regions, match elements against those sets, and prune traversal when a subtree is already invalid. See [`style_invalidator.cc`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/core/css/invalidation/style_invalidator.cc) and [`invalidation_set.h`](https://chromium.googlesource.com/chromium/src/+/58454d73e25bfac1610e92556a37a1c0fc1fa89e/third_party/blink/renderer/core/css/invalidation/invalidation_set.h).

**Verified Ladybird technique:** Ladybird already retains selector answers, plans impacts through `ImpactRegion`, and routes reactions adaptively. The Rust reaction layer selects a broad matching batch when a transaction is unscoped or when reaction density exceeds one sixteenth of the connected elements. That selects a scratch/matching strategy; it is not proof of full-document traversal. The planner separately marks a pass unscoped for document/tree-scope/root-subtree regions. The measured `dom-style` workload had zero unscoped transactions, so its broad matching passes were selected by the density heuristic.

This is an architectural difference, not evidence that Blink is faster or that Ladybird lacks incremental invalidation. The measured focused fixtures demonstrate that Ladybird already handles isolated class changes, descendant selectors, sibling selectors, and `:has()` through narrow batches. Porting Blink's data structures or changing Ladybird's planner without a proven source of unnecessary work would duplicate mechanisms and add correctness risk.

## Fixture correction

The first version of the focused style fixture toggled classes an even number of times between computed-style reads. Its first smoke capture showed only one narrow reaction and therefore did not exercise repeated style processing as intended. Fixture revision 2 now reads computed style after each 65 toggles, plus once after the remaining mutations. Each read follows an odd number of toggles and forces an alternating state. Revision 1 reports are retained in history but are not comparable to revision 2 and are excluded from the results below.

The fixture is deterministic and checks resulting computed custom properties for each selector family. It remains synthetic; it does not approximate all application pages.

## Measured baselines

All rows below use five successful measured iterations after one warm-up, diagnostics enabled, the same release Photon executable path, isolated profiles, a 100 ms process-resource sampler, and the same Mac17,4 / Apple M5 / 10 logical CPU host on macOS 27.0.1 arm64. Display refresh was reported as 60 Hz. Engine and executable revisions remain unknown in report provenance because the protocol does not attest them. Process memory is process-tree scope, not per-tab memory or JS heap.

| Workload (fixture revision) | Fixture median | `style_update_duration_ms` median | Broad matching | Narrow matching | Reaction entries | Full/partial layout duration median | Frame evidence |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `style-localized` (v2) | 8.4 ms | 4.001 ms | 0 batches | 17 batches | 17 | 0 / 0 ms | Insufficient samples; at most 1 interval per run. |
| `style-descendant` (v2) | 14.8 ms | 11.173 ms | 0 | 17 | 544 | 0 / 0 ms | Insufficient samples. |
| `style-sibling` (v2) | 14.8 ms | 10.827 ms | 0 | 17 | 527 | 0 / 0 ms | Insufficient samples. |
| `style-relational` (`:has()`, v2) | 8.3 ms | 4.057 ms | 0 | 17 | 17 | 0 / 0 ms | Insufficient samples. |
| `dom-style` (v1) | 49.2 ms | 43.902 ms | 16 batches; 16.862 ms apply time | 1 batch; 0.712 ms apply time | 1,560 | 0 / 0 ms | Insufficient samples. |
| `layout-updates` (v1) | 161.3 ms | 35.164 ms | — | — | — | 88.595 / 5.925 ms | Insufficient samples. |

For `dom-style`, the five individual runs measured 47.9–51.7 ms total, 43.785–44.231 ms in style updates, and 16.670–17.069 ms in the broad matching path. They reported 1,528 broad reaction entries and zero unscoped transactions each time. This confirms repeated dense matching work in this mixed synthetic workload. It does not establish a document-wide fallback: the metric counts published reaction entries, and the transactions remained scoped.

For the isolated fixtures, the observed reaction volume is consistent with the selected dependency: 17 local entries; 544 descendant entries (32 per pass); 527 sibling entries; and 17 relational entries. The descendant and sibling cases take longer because they process more affected entries. This is measured work, but no unnecessary work was demonstrated.

The geometry workload executed 256 full and 256 partial layout passes. Full layout duration was consistently 83.327–96.445 ms; partial layout was 5.812–7.069 ms. This is the clearest measured candidate for follow-up profiling, but the fixture deliberately forces geometry reads and does not establish real-world impact.

### Animation and drawable presentation

The five-run animation capture produced 1,105 valid frame-interval samples across five sessions with no frame or presentation-event buffer evictions. The reported nominal display rate was 60 Hz. Across run-level distributions:

- Median p50 drawable interval: **16.667 ms**.
- Median p95: **33.333 ms**; individual run p95 values ranged from 16.671 to 33.333 ms.
- Median p99: **83.333 ms**; individual run p99 values ranged from 50.000 to 133.333 ms.
- Median outlier count: **7** per run, using Velocity's relative threshold of twice that run's median interval. This is a screening rule, not a dropped-refresh claim.
- Four sessions completed the five 900 ms phases in 4.53–4.59 s. One session took 124.12 s and contained a 121.25 s drawable interval. The app did not expose occlusion or visibility state; this gap cannot be attributed to the Engine.

The animation run measured 1,118 drawable feedback callbacks and 1,105 interval samples, retained 100% of bounded presentation events, and observed pre-install coalescing. Four sessions had no repeated surface presentation; one session had four repeated presentations. These timestamps establish drawable presentation, not changed webpage pixels or user perception. No style transaction is timestamped into a later frame ID, so same-run style/frame co-occurrence does not prove causation.

`engine_surface_to_presentation_latency_ms` remains null. Photon and Metal stages can be correlated after Photon receives the surface, but Engine publication has no timestamp in a validated clock domain shared with Metal. The browser also does not expose changed-pixel or occlusion evidence.

### Instrumentation overhead

Velocity ran five seeded, adjacent randomized diagnostics-off/on pairs on `style-localized` v2 (`seed=20261018`). Each sample used a fresh profile; pairs do not share browser state. Effects are on-minus-off with a 10,000-resample percentile bootstrap interval over five paired differences:

| Metric | Median paired difference | 95% interval | Interpretation |
| --- | ---: | ---: | --- |
| Fixture duration | -0.30 ms | [-3.10, +0.20] ms | No measurable slowdown; interval is inconclusive. |
| Process-tree CPU mean | -0.763 percentage points | [-3.027, +3.055] pp | Does not support the previous sequential CPU-increase interpretation. |
| Physical footprint mean | -540,672 bytes | [-16,454,393, +14,378,816] bytes | Too much variation to resolve an effect. |
| RSS mean | -70,802 bytes | [-782,336, +772,096] bytes | No resolved effect; RSS can include shared pages. |

Peak memory intervals also include zero. Five pairs are a small sample and the interval widths remain large. This experiment finds no material diagnostics overhead, but it does not prove zero overhead. The raw paired samples and order are retained in Velocity history.

## Optimization decision and ranking

| Rank | Candidate | Evidence | Decision |
| ---: | --- | --- | --- |
| 1 | Reduce dense broad matching in the mixed DOM/style workload | **Measured:** 16 broad passes and 16.862 ms apply time in `dom-style`. **Verified:** broad mode follows the density heuristic; unscoped count is zero. | Do not change yet. The work may be required for the 1,528 reaction entries; no selector family/reason or safe narrower region is identified. First add bounded per-transaction reason/region summaries and test a local application fixture. |
| 2 | Reduce repeated full-layout work | **Measured:** 256 full passes and 88.595 ms median full-layout time in the geometry fixture. **Hypothesis:** the ratio may matter for real interactions. | Best next subsystem to investigate. Add/inspect fallback reasons and associate layout rounds with an update-cycle identifier before changing layout behavior. |
| 3 | Narrow descendant/sibling invalidation | **Measured:** 544 and 527 entries across 17 passes, with no broad matching. | No target found: reaction volume matches the intended changed descendant/sibling regions. |
| 4 | Narrow isolated class or `:has()` invalidation | **Measured:** 17 entries across 17 narrow passes; 4.001 ms and 4.057 ms total style-update medians respectively. | Low benefit signal in this fixture; preserve current algorithm. |

The timer queue p95 observed during these fixtures was roughly 0.05–0.08 ms. These workloads do not inject user interaction or create a competing runnable backlog, so they do not falsify the task-scheduling hypothesis. The animation workload observed one GC-timing-bearing session and low mark/pause durations there; collection coverage was 1/5, so this does not falsify the GC hypothesis either.

**No before/after optimization result exists** because no algorithm change passed the evidence gate. There is no claimed style-speedup, smoothness improvement, or memory improvement. Current memory samples are descriptive process-tree baselines only.

## Validation and recorded baselines

Validation completed on this checkout:

- `./photon format` — passed.
- `./photon check` — passed.
- `./photon test` — passed.
- `./photon build` — passed; the release Photon executable used for the live captures was built before those captures.
- `cargo fmt --all -- --check` in `tools/velocity/` — passed.
- `cargo test --workspace` in `tools/velocity/` — 165 tests passed (43 browser, 92 CLI, 17 core, 13 metrics); doc tests passed with no tests defined.

The style fixtures completed their computed-style assertions in five measured runs each. The task fixture harness and history provenance checks verify that workloads use the registered fixture revision and selector definition. Velocity history accepts the focused fixture and frame metric schemas, including units and sample coverage.

Named baseline references were created for the measured diagnostics-on reports:

| Name | Velocity history record |
| --- | --- |
| `phase10-2-style-localized-v2` | `record-fd9a4fa62f4d57730652562b27ddb969a3551987e487cc35045348bc37432f58` |
| `phase10-2-style-descendant-v2` | `record-6c212ff1698684c9d630726093991bcbedd02b5e6b9376b117002626cdacbbc9` |
| `phase10-2-style-sibling-v2` | `record-59e22f52c641ab1bd8f4547b16e20a34797daba8980db7589f59618ffb06c11a` |
| `phase10-2-style-relational-v2` | `record-6267d417989a2d695cace31b37974b211a9d8f86b7d1dcb9644b975a1c5c73c4` |
| `phase10-2-dom-style` | `record-39600b95347e794314e1ca712997406c76f9b6f063d6e3136e4b86b089b6b631` |
| `phase10-2-layout-updates` | `record-327b2dc46c2dadaf22dbed537683943fdf19c20061623d7374538685145f8e36` |
| `phase10-2-animation` | `record-30d248a7781be02818e42d861b27f5b994aa47ca2c0bf0c3e223191842974bec` |

The balanced overhead report is retained in history but is not used as a performance baseline. Exact revisions remain unknown in the benchmark records; the checked-out base commits and dirty state above are context, not executable attestations.

## Limitations and next steps

1. There are no reproducible local dashboard, feed, long-document interaction, or component-app fixtures yet. Synthetic results must not be generalized to all sites.
2. Style counters aggregate a diagnostic interval and do not include mutation identity, transaction cause, distinct-node visits, or a join key to layout/presentation.
3. Full-document recomputation count/reason and style fallback reason are unavailable. Broad matching must not be presented as a full-document fallback.
4. The frame interval sample contains one extreme long gap with unknown window visibility; input-to-presentation latency is not measured.
5. Process-tree physical footprint and RSS do not attribute memory per tab; GC live-heap coverage is incomplete in the animation workload.
6. One early style-fixture capture logged a transient WebContent/RequestServer disconnect; Velocity still recorded complete fixture coverage, and subsequent focused and balanced runs completed. Its timing relative to teardown and its cause were not established, so it is not used to explain benchmark timing.

Next, instrument layout fallback reasons and update-cycle IDs, then run the same geometry and selector fixtures plus deterministic local application pages while correlating update cycles to Engine surface publication and Metal presentation. Reconsider a style change only if that tracing shows frequent, avoidable dense work for a real interaction pattern and correctness can be preserved by narrowing the existing `ImpactRegion` plan.
