# Performance

Photon's frame-path and memory work from October 2026, measured before and after
each change. The investigation notes, traces and method are in
[CompositorPerformance.md](CompositorPerformance.md).

Unless a row says otherwise, measurements were taken on macOS with the window
uncovered, using a full-window `requestAnimationFrame` animation, a long
self-scrolling page, or `https://en.wikipedia.org/wiki/Web_browser`. Rates on a
60 Hz display cap at 60 fps; the original report was a 120 Hz 4K display.

## Before and after

| Area | Change | Before | After |
|---|---|---|---|
| Frame pacing | Send the window's display ID and refresh rate to Engine | Compositor ticked a 60 Hz timer; `requestAnimationFrame` capped at 60 | Display-linked ticks at 120 Hz; animation frames ~115–120/s on a 120 Hz display |
| Frame pool | Four backings for clients that sample on the GPU | Ticks deferred for lack of a backing: 470 in ~6 s | 0 |
| Frame release | Release a replaced frame once a newer one is submitted and its own sampling completes | Previous frame released 11.4 ms (p50) / 20.1 ms (p90) after the next was published | 8.0 ms / 8.7 ms |
| Frame rate | Pacing, pool and release together | 30 fps on 120 Hz 4K (reported); best 1 s window 97 fps | Steady display rate: 60 fps at 60 Hz with no deferrals; best 1 s window 115 fps at 120 Hz |
| Covered window | Mark the active tab hidden while the window is occluded | Engine kept rendering at 60 fps with nothing drawn | 0 fps while covered; resumes on uncover |
| Resize | Keep the presentation generation across resizes | 20 resizes: 21 generations, 66 backing registrations (2 synchronous XPC calls each) | 3 generations, 10 registrations |
| Resize memory | Release replaced backings (shell map, broker send rights, GPUI-CE texture cache) | Compositor's unused surfaces: 139 MB after 20 resizes, 232 MB after 40, still growing | 73 MB after 20, ~53 MB after 40 and 60 |
| UI draw | Cache the browser chrome; stop refreshing the window per Engine frame | Draw per page frame 1.26 ms (p50) / 1.76 ms (p90) | 0.84 ms / 1.03 ms |
| Per-frame overhead | Cache `PHOTON_VERBOSE`; track frame gaps only with the overlay shown; wake the release drain only when needed | `getenv` on every trace, gap scan of up to ~1,200 entries, drain wake on every frame | None of these per frame; not measurable at frame level |
| Idle pages | Repaint for video paint facts only when they change (Ladybird bug) | Any page with a `<video>`, even paused, rendered at display rate forever; Wikipedia at 60 fps indefinitely | Idle (0 fps) a few seconds after load; a playing video still presents at 60 fps |
| Page load | Stop loading SVG images requesting frames of every SVG image (Ladybird bug) | ~13,000 image invalidations while Wikipedia loads (206 SVG images) | 0 |
| Idle GPU memory | Purge Skia resources unused for 5 s once no frame has completed for 5 s | Compositor on idle Wikipedia: 230 MB footprint, 114 MB GPU memory | 122 MB footprint, 17 MB GPU memory |

## Investigated without a change

| Area | Finding |
|---|---|
| Frame delivery hop | The `app.spawn` hop before presenting a frame takes 31 µs (p50) / 76 µs (p90); a frame waits for GPUI-CE's next vsync either way. |
| Apparent 150–600 ms stalls | A test animation moved content off-screen; the damage diff correctly skipped those frames. |
| Blank launch | Not reproduced in 20 launches (12 local page, 8 Wikipedia); every launch presented frames. |
| Scrolling | Steady 60 fps at 60 Hz with no deferrals. The compositor re-rasterizes the whole viewport each scroll frame; its GPU time is 4.1 / 5.3 / 5.7 ms (p50 / p90 / p99) at 3.3 MP and 5.8 / 9.1 / 12.5 ms at 6.2 MP. Other stages stay under 1 ms. |

## Remaining

- Confirm 120 fps on the 120 Hz 4K display.
- Scrolling a full-screen 4K page at 120 Hz: extrapolated compositor GPU time is about 7.7 ms (p50) and 12 ms (p90) against an 8.3 ms budget. Caching rasterized scroll content (tiles or layers) instead of repainting the viewport each frame would remove most of it.
- First paint of heavy pages: on Google search results the Compositor's Skia flush took 461, 195 and 191 ms for the first frames, then spiked to 21–84 ms (34 fps, frame interval p95 100 ms). Cache hits rule out shader compilation; see [section 9](CompositorPerformance.md#9-slow-first-paint-on-heavy-pages--open).
- Send the two Ladybird fixes upstream: branches `libweb-video-paint-facts-repaint` and `libweb-svg-image-load-frame-requests` on `TheoSlater/ladybird`.

## Engine and GPUI-CE commits

| Repository | Commit | Change |
|---|---|---|
| Engine | `864c2a0892` | LibPhotonEmbedder: Accept display metadata from the embedder |
| Engine | `37e4b20582` | Compositor: Allocate a fourth backing store for GPU-sampling clients |
| Engine | `74d3338ba6` | LibPhotonEmbedder: Keep the presentation generation across resizes |
| Engine | `e2746e5c0a` | LibWeb: Repaint for video paint facts only when they change |
| Engine | `967707f712` | LibWeb: Keep loading SVG images from requesting frames of every image |
| Engine | `1c9ff08eb5` | Compositor: Purge unused GPU cache once a page stops drawing |
| GPUI-CE | `fdd3033141` | gpui_apple: Release external surface textures that are no longer drawn |
