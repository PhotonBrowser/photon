# Compositor frame rate

**Goal:** Engine frames at the display's refresh rate (120 Hz on a 4K ProMotion/external panel). **Observed before fixes:** 30 fps.

Two separate problems stack: a 60 fps ceiling, then a release round trip that misses every other tick.

## 1. Engine paced to 60 Hz regardless of the display — fixed

`PhotonHeadlessWebView` never sent display metadata, so these defaults applied:

- `ViewImplementation::m_maximum_frames_per_second` = 60 → WebContent `requestAnimationFrame` capped at 60.
- `ContextState::m_display_refresh_rate` = 60 → both `FramePacer`s use a 16.7 ms interval.
- `m_display_id` empty → `TimerVSyncScheduler` (millisecond-rounded `Core::Timer`, unaligned to vsync) instead of `CVDisplayLinkVSyncScheduler`.

Fix: `Photon::View::set_display_metadata(display_id, refresh_rate)` (mirrors Qt's `WebContentView::set_display_metadata`), re-sent for new pages in `prepare_page_for_tab`. The shell calls it from the WebView's prepaint with the window's `CGDirectDisplayID`; `crates/photon-shell/src/platform/display.rs` reads the rate from `CGDisplayMode`, falling back to the `CVDisplayLink` nominal period.

## 2. Release round trip longer than one tick — fixed

The pool has 3 IOSurfaces (`backing_store_count_for`). Photon holds the displayed frame and the newly received frame, so the compositor has one store to render into. The displayed store comes back only after:

1. compositor publish → UI IPC → `server_did_paint` → `native_frame_ready`
2. Rust `receive_frame` → `request_redraw` (`app.spawn`) → `present_latest` → `cx.refresh()`
3. next GPUI display-link frame (≤ 8.3 ms at 120 Hz)
4. GPUI command buffer waits on the Engine's `MTLSharedEvent` (Engine 4K paint must finish)
5. completion → CFRunLoop release-drain source → `photon_view_release_native_frame`
6. UI → compositor IPC → `release_buffer`
7. next compositor vsync tick

If 1–6 exceed one frame interval, `is_present_blocked()` defers to the following tick: half rate.

Measured on a full-window `requestAnimationFrame` animation at 120 Hz after fix 1 (2026-10-08):

| | p50 | p90 |
|---|---|---|
| Vsync ticks / rAF delivered | ~115/s | |
| Compositor frames submitted | ~28/s | |
| Compositor GPU work per frame | 1.5 ms | 3.9 ms |
| Next frame published → previous frame released by Photon | 11.4 ms | 20.1 ms |
| Release received → next frame begins (vsync wait) | 4.3 ms | 6.0 ms |

Ticks were deferred with `blocked=true backing_available=false` far more often than frames were submitted. Engine paint is not the limit; the release hand-back is.

The growth heuristic `add_backing_store_if_window_server_still_reads_every_released_store` cannot help, because lease-deferred stores stay `Presented` rather than `Available`.

Fixes:

- The compositor allocates 4 stores when the client samples them on its GPU (`BackingStoreManager::ClientSamplesOnGpu`), 3 otherwise.
- Photon releases a replaced frame once a command buffer sampling a newer frame has been *submitted* and the replaced frame's own sampling command buffers have completed (`presentation::Sampling`), instead of waiting for the newer frame's GPU completion. GPUI-CE submits on one queue in order, and may re-present its last drawn scene without redrawing, so release cannot happen at swap time.

After both, on the same animation:

| | Before | After |
|---|---|---|
| Next frame published → previous released, p50 / p90 | 11.4 / 20.1 ms | 8.0 / 8.7 ms |
| Ticks deferred for lack of a store | 470 | 0 (9 deferred only for the previous frame's GPU work) |
| Best 1 s window | 97 fps | 115 fps |

## 3. Apparent stalls were the test page — resolved

The first test animation moved its box off-screen for long stretches. The compositor's damage diff correctly found no visible change and skipped those frames, which looked like 150–600 ms stalls. GPUI-CE's frame loop was idle, not stuck. With the box kept on-screen, a 60 Hz display holds 60 fps with no skipped frames and one deferral in 8 s; lease accounting balances at shutdown.

When measuring, keep animated content inside the viewport and the window uncovered: GPUI-CE stops its display link while the window is occluded.

## 4. Occlusion and display changes — fixed

`native/embedder/WindowObserver.mm` reports a window's occlusion state, moves to another screen, and system screen-parameter changes (including refresh-rate switches) to `BrowserWindow`.

- While the window is fully occluded the active tab is marked hidden, so Engine stops delivering rendering opportunities and frames. Verified: hiding the app took Engine from 60 fps to 0, and bringing it to the front resumed 60 fps, with lease accounting balanced at shutdown.
- On a display change every tab forgets its cached display metadata and re-reads the refresh rate on its next layout.

## 5. Further opportunities — review of 2026-10-09

Reviewed after the crate split (`photon-ffi`, `photon-presentation-ipc`). The frame pipeline itself now runs at display rate; these are the remaining costs, most significant first.

1. **Every resize re-registers every backing over synchronous XPC.** `PhotonEmbedder.cpp` bumps `m_native_generation` on every physical size change, even when the compositor keeps its padded pool. Each new generation re-registers each backing: two synchronous broker round trips (`register_backing` + `import_backing`), an `IOSurfaceLookupFromMachPort` and a `CVPixelBuffer` on the UI thread, plus a GPUI-CE texture-cache miss. Measured: 20 window resizes produced 21 generations and 66 registrations. **Fixed** in Engine `74d3338`: the generation now advances only when the compositor replaces its pool (`on_backing_store_pool_changed`); the same resizes produce 3 generations and 10 registrations, with the page rendering correctly at each size.
2. **Registrations are never released.** `PresentationRuntime::backings` (Rust), `m_registered_native_backings` (embedder) and GPUI-CE's `external_surface_textures` only ever insert. Each entry retains its IOSurface, so pools the compositor has replaced stay alive in the UI process for the session; the Compositor showed 87 MB of owned, unmapped memory after resizing. **Fixed:** registering a new generation drops older generations from `PresentationRuntime::backings` and tells the broker to release their send rights (the broker kept one per surface, which alone kept every replaced pool alive); GPUI-CE evicts external textures not drawn for 120 frames and flushes its CoreVideo texture cache. Over 60 resizes the Compositor's unused surfaces stayed at 53–73 MB, where before they grew to 232 MB after 40. The embedder's registration set holds only IDs, not surfaces.
3. **The whole window re-renders on every page frame.** `update_webview` ends with `cx.refresh()`, which marks every window as refreshing, and no view uses `.cached()`, so titlebar, tabs, toolbar and omnibox rebuild for each Engine frame. Measured full-window draw: 1.26 ms p50, 1.76 ms p90 on the UI thread, of which the chrome was about half (0.65 / 0.83 ms without it). **Fixed:** the titlebar and toolbar render as a cached `BrowserChrome` view that re-renders on `WebViewEvent::StateChanged` and window notifications, and Engine frames notify only the page view: 0.84 ms p50, 1.03 ms p90.
4. **Frame delivery waits for a spawned task.** Measured: the `app.spawn` hop before `present_latest` takes 31 µs p50 and 76 µs p90. The frame waits for GPUI's next vsync either way, so this is not a real cost.
5. **Small per-frame costs.** `platform::trace` reads `PHOTON_VERBOSE` from the environment on every call, including from Metal completion handlers; `record_frame` scans up to ~1,200 gap entries per frame even with diagnostics off; `on_engine_native_frame` signals the release-drain source on every frame even when nothing is pending.

## 6. Idle pages kept rendering — fixed

Measured on `https://en.wikipedia.org/wiki/Web_browser` (2026-10-09). After loading, the page kept rendering at the full display rate with no script or CSS animation running.

- **Video elements requested a repaint on every rendering update.** `Page::sync_media_element_video_sink_ticking` called `Painting::push_video_paint_facts` for each laid-out video, which requested a repaint unconditionally; the repaint requested the next update. Any page with a `<video>`, even paused, rendered at display rate indefinitely. Frames are presented through the video's sink, so the per-update sync now repaints only when a video's paint facts or destination change (Engine `e2746e5`). Wikipedia now goes idle about 5 s after load (0 fps); a playing video still presents at 60 fps and a paused one goes idle.
- **Loading SVG images broadcast frame requests to every SVG image.** An SVG image document finishing its load requested a frame before its image existed, so the request went to every SVG image of the page. Wikipedia's 206 SVG images produced about 13,000 invalidations while loading. Such requests are now suppressed until the image exists (Engine `967707f`); images still render.

Both are upstream Ladybird code (`LibWeb: Commit video paint facts`, 2026-09-09; the SVG image loading changes of 2026-10-05/06) and still present on Ladybird `master` when fixed; worth sending upstream.

The Compositor's Skia GPU cache was also checked on the same page: 50–75 MB, almost all of it purgeable, which Skia trims only after a flush. The Compositor now purges resources unused for 5 s once no frame has completed for 5 s (Engine `1c9ff08eb5`): on idle Wikipedia its footprint fell from 230 MB to 122 MB and its GPU memory from 114 MB to 17 MB. Released GPU memory takes several seconds to leave the footprint.

## 7. Scrolling — measured

A long page with text, gradients, shadows and SVG images scrolled continuously by keeping a `scrollBy({behavior: "smooth"})` in flight (compositor smooth-scroll path), on a 60 Hz display:

| | 2384×1380 (3.3 MP) | 3384×1826 (6.2 MP) |
|---|---|---|
| Frames per second | 60, no deferrals for backings | 60 |
| Compositor GPU time (p50 / p90 / p99) | 4.1 / 5.3 / 5.7 ms | 5.8 / 9.1 / 12.5 ms |
| Compositor CPU paint (p50) | 0.8 ms | 0.4 ms |
| WebContent rendering update (p50) | 0.5 ms | 0.4 ms |
| GPUI-CE draw (p50) | 0.7 ms | 0.8 ms |

Each scroll frame re-rasterizes the whole viewport, so GPU time scales with pixels. A full 4K window (8.3 MP) extrapolates to about 7.7 ms p50 and 12 ms p90, over the 8.3 ms budget of 120 Hz. Caching rasterized scroll content instead of repainting the viewport each frame would address it.

## 8. Blank launch — not reproduced

20 launches (12 with a local page, 8 with Wikipedia) all presented frames.

## Commits

- Engine `864c2a0892` LibPhotonEmbedder: Accept display metadata from the embedder
- Engine `37e4b20582` Compositor: Allocate a fourth backing store for GPU-sampling clients
- Engine `74d3338ba6` LibPhotonEmbedder: Keep the presentation generation across resizes
- Engine `e2746e5c0a` LibWeb: Repaint for video paint facts only when they change
- Engine `967707f712` LibWeb: Keep loading SVG images from requesting frames of every image
- Engine `1c9ff08eb5` Compositor: Purge unused GPU cache once a page stops drawing
- GPUI-CE `fdd3033141` gpui_apple: Release external surface textures that are no longer drawn

`Tests/Compositor/TestContextState.cpp` does not compile on Engine `master` independently of these changes: its `TestCompositorClient` still uses the old `did_present_frame` signature (without `presentation_signal_value`) and `spin_event_loop_until` overloads. Photon's Engine build does not build these tests (`ENABLE_LADYBIRD_UI` is off), which is why the breakage went unnoticed.

## Checklist

- [x] Send display ID and refresh rate from the shell to Engine (compositor uses CVDisplayLink, rAF and pacers at 120 Hz)
- [x] Re-send when the refresh rate changes on the same display (`WindowObserver` reacts to `NSApplicationDidChangeScreenParametersNotification`; not yet exercised by an actual rate change)
- [x] Measure sustained fps on an animated page (`EXTERNAL_IMAGE_LEASE_TRACE=1 PHOTON_CORE_RUNLOOP_TRACE=1`; see table above)
- [x] Let the pool exceed 3 stores under native presentation (4 stores; ~33 MB per extra 4K BGRA store)
- [x] Release the previous backing once a newer frame's command buffer is submitted and its own sampling has completed
- [x] Explain the 150–600 ms gaps (off-screen test content, not a pipeline stall)
- [ ] Confirm 120 fps on the 120 Hz display (measurements above were on a 60 Hz screen)
- [x] Tell Engine the view is hidden while the window is occluded, so it stops rendering frames nobody draws
- [x] ~~Call `present_latest` directly from the frame callback~~ — measured the `app.spawn` hop at 31 µs p50 / 76 µs p90; a frame waits for GPUI's next vsync either way, so not worth a GPUI-CE API
- [x] Trim per-frame work: cache `PHOTON_VERBOSE`, track frame gaps only while the overlay is shown, signal the release drain only when a release is queued
- [x] Bump the presentation generation only when the compositor replaces its pool, not on every resize (Engine `74d3338`: 20 resizes went from 21 generations / 66 registrations to 3 / 10)
- [x] Release backing registrations for replaced generations (Rust map + broker send rights; GPUI-CE evicts textures undrawn for 120 frames). 60 resizes: unused Compositor surfaces stay ~50 MB instead of growing past 230 MB
- [x] Cache the browser chrome and stop refreshing the window per Engine frame: UI-thread draw 1.26 → 0.84 ms p50, 1.76 → 1.03 ms p90
- [x] Stop idle pages rendering at display rate (video paint facts; SVG image load broadcasts)
- [x] Purge Skia's unused GPU cache after the page goes idle (Compositor 230 → 122 MB on idle Wikipedia)
- [ ] Send the video and SVG fixes upstream to Ladybird: branches are on `TheoSlater/ladybird`; opening PRs from the CLI was refused by GitHub, so open them from the web
- [ ] Cache rasterized scroll content so 4K scrolling fits the 120 Hz GPU budget
- [x] Profile scrolling (section 7)
- [x] Chase the intermittent blank launch (not reproduced, section 8)
