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

## Commits

- Engine `864c2a0892` LibPhotonEmbedder: Accept display metadata from the embedder
- Engine `37e4b20582` Compositor: Allocate a fourth backing store for GPU-sampling clients

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
- [ ] Deliver releases without the CFRunLoop hop where thread safety allows
- [ ] Call `present_latest` directly from the main-thread frame callback instead of via `app.spawn`, so a frame is not deferred an extra GPUI frame
- [ ] Cache `PHOTON_VERBOSE` in `platform::trace` (`OnceLock`); it calls `getenv` on every call, including in Metal completion handlers
- [ ] If still short of 120: profile Engine 4K Skia paint time (`compositor_frame_profile`) against the 8.3 ms budget
