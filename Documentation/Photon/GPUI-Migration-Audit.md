# GPUI-CE migration audit

This inventory records the current runtime and the Photon changes that must be
accounted for before replacing GPUIX or the nested Zed checkout. The working
tree was clean at the start of the migration audit. No runtime or vendored
directory has been removed.

## Current runtime trace

`./photon run` is dispatched by `crates/photon-cli/src/cli.rs` to the run
command. Build/check commands prepare the Rust N-API addon, copy it into the
app build directory, build Engine when enabled, compile the UI TypeScript, and
start Bun. `ui/` hosts the React application and calls the native addon.
`crates/photon-native-addon` re-exports `gpuix-native`; Photon custom elements
are registered by `crates/photon-gpui/src/engine.rs` and
`crates/photon-gpui/src/titlebar.rs`.

The native `PhotonWebView` element and its callback state live in
`crates/photon-gpui/src/engine.rs`. It owns the Engine view bridge, accepts
bitmap fallback frames and macOS native frame descriptors, converts layout
bounds to physical viewport dimensions, forwards input, tracks frame order,
requests GPUIX redraws, and turns GPUI external-image completion into Engine
frame release. The presentation broker is launched by
`crates/photon-cli/src/commands/presentation_broker.rs`; its XPC implementation
currently resides in the nested Zed fork's `gpui_apple` crate. The compositor
and Photon exchange IOSurface and shared-event descriptors through this
broker. GPUI's scene carries external frames to `gpui_apple`'s Metal renderer,
which imports/caches IOSurface textures, encodes the producer event wait in the
sampling command buffer, and reports GPU completion. The existing Engine
callbacks and release queues retain Engine-owner-thread affinity.

The temporary direct path is `./photon run-gpui-ce [--url URL]`. It builds the
Engine and `crates/photon-app`, starts the Photon-owned broker, then launches a
plain GPUI-CE AppKit window containing `BrowserWindow → PhotonWebView`. The new
app does not depend on `photon-gpui`, GPUIX, or the Zed GPUI crate. Its
presentation client lives in `crates/photon-app/src/platform/`; the shared
broker implementation lives in `native/presentation/`. The legacy
`./photon run` path remains unchanged at this milestone.

## Existing Zed/GPUI change classification

Compared with `upstream/gpuix` at the current
`vendor/gpuix/zed` branch, the Photon branch changes 23 files. The current
dependency commit is `77530cc5d7` (`gpui: improve cross-process presentation`).

| Current file(s) | Classification | Migration destination / evidence required |
| --- | --- | --- |
| `crates/gpui/src/elements/surface.rs`, `scene.rs`, `window.rs` | Generic GPUI renderer/platform extension | Re-express external resource descriptor, scene primitive, clip, synchronization metadata, and completion lifetime against GPUI-CE's current APIs. Focused renderer tests. |
| `crates/gpui/src/elements/div.rs`, `style.rs` | Generic GPUI renderer/platform extension | Preserve rounded content-mask semantics only if GPUI-CE lacks equivalent nested rounded clipping. Add rounded external-surface regression coverage. Separate formatting-only changes. |
| `crates/gpui/src/assets.rs` | GPUIX-only / CPU fallback support | `LiveImage` is a mutable CPU image route and is not part of the desired native zero-copy path. Reassess only for an explicitly supported fallback; do not port by default. |
| `crates/gpui_apple/src/metal_renderer.rs`, `shaders.metal`, `gpui_apple/Cargo.toml`, `build.rs` | Generic renderer extension mixed with Photon presentation transport | Port IOSurface import, actual IOSurface identity cache key, shared-event wait in the consuming command buffer, external draw/clipping, device registry ID, and generic completion into the Photon GPUI-CE fork. Move browser/broker policy and transport out. |
| `crates/gpui_apple/src/presentation_xpc.rs`, `presentation_xpc.m`, presentation XPC examples | Presentation/XPC broker | Keep broker protocol, service registration, and Photon descriptor policy in Photon-owned platform/runtime code. Replace only if GPUI-CE needs a generic OS primitive. |
| `crates/gpui_apple/examples/iosurface.rs` | Profiling/debugging | Retain only if it remains a useful generic GPUI-CE example; otherwise replace with focused tests. |
| `crates/gpui_macos/src/dispatcher.rs`, `gpui_macos.rs`, `platform.rs` | macOS platform integration | Audit each small change against GPUI-CE's AppKit loop and wake API. Preserve generic thread/wake correctness there; keep Core/Engine integration in Photon. |
| `crates/gpui_macos/src/display_link.rs` | macOS platform integration / GPUIX scheduling interaction | Port only display scheduling behavior that remains independently required by direct GPUI; remove Bun/GPUIX wake compatibility. Validate event-driven redraw without a 16 ms timer. |
| `crates/gpui_macos/src/window.rs` | macOS platform integration | Port native material/titlebar behavior only if GPUI-CE's NSWindow API cannot provide it. Keep Photon window configuration in the shell adapter. |

The broad patch includes formatting churn, headless smoke tests, XPC transport,
external frame lifecycle code, and generic Metal rendering changes together.
These are distinct work areas and must be separated during the port rather
than applied as a single patch.

## Photon-owned runtime responsibilities to retain

- `crates/photon-gpui/src/engine.rs`: current WebView/Engine integration,
  presentation generation/order, input, viewport sizing, redraw wakeups, and
  release forwarding. Split the reusable lifecycle/presentation concerns from
  GPUIX element glue before deleting the addon.
- `crates/photon-cli/src/commands/presentation_broker.rs` and its build/run
  integration: automatic checkout-specific broker startup and service-name
  propagation.
- Engine/Ladybird embedder API and compositor IPC under `Engine/`.
- Photon frame lifecycle: ACTIVE → RETIRED → DRAINING → DESTROYED, duplicate
  release handling, stale-generation rejection, fallback selection, and
  Engine-owner-thread release delivery.
- Photon window policy and `PhotonWebView`, including the 4 logical pixel
  inset and browser-local input coordinates.

## GPUI-CE source audit and migration gate

The Photon-owned fork is available and is checked out as a root submodule at
`vendor/gpui-ce`, revision `94db357231cad1f2c38d2c3821e728e7c77070f7`
(`Add fluent WindowOptions builder methods (#301)`). Its `origin` is
`PhotonBrowser/gpui-ce`; `upstream` is `gpui-ce/gpui-ce` `main`.

Source inspection confirms GPUI-CE currently has a `SurfaceSource::Surface`
CoreVideo path for YUV surfaces and a generic scene `PaintSurface`, but that
Metal path creates CoreVideo textures during drawing and has no external
BGRA/IOSurface cache identity, producer shared-event wait, or per-resource GPU
completion callback. `Surface::paint` currently notes that corner radii are
unsupported; the renderer's existing surface shader consumes rectangular
content masks. GPUI-CE already exposes `WindowOptions` for transparent titlebar,
AppKit titlebar drag ownership, resizability, and window background appearance;
its macOS renderer/window code supports transparent and blurred backgrounds.
These APIs should be reused before adding AppKit fork changes.

Do not remove `vendor/gpuix`, its Zed submodule, `ui/`, or the native addon
until a direct GPUI-CE build has demonstrated the complete native surface path
through GPU completion and Engine release. Required port checklist:

| Behavior | Photon Zed implementation | GPUI-CE target | Required proof |
| --- | --- | --- | --- |
| IOSurface import / BGRA sample | `gpui_apple/src/metal_renderer.rs`, `shaders.metal` | `crates/gpui/src/scene.rs`, surface element and `crates/gpui_apple/src/metal_renderer.rs` | Imported surface draws with zero pixel/atlas copies |
| Actual IOSurface identity cache key | Metal renderer external texture cache | GPUI-CE Apple renderer resource cache | Same logical descriptor ID with a different IOSurface imports a new texture |
| Shared-event wait | external frame metadata and Metal renderer | scene resource metadata and consuming `MTLCommandBuffer` | Wait value is encoded in the command buffer that samples the texture |
| GPU completion | external image lease lifecycle / command-buffer completion | generic surface completion callback in Apple renderer | Callbacks follow each sampling command buffer; Photon retains the displayed backing and releases it after a replacement frame completes |
| Rounded clipping | scene bounds, content mask, shader | `Surface` element + surface shader | Rectangular, rounded, and nested clipping tests |
| Metal registry ID | Apple device access | generic Apple renderer accessor | Photon compares Engine and GPUI IDs |
| Frame tracing | `EXTERNAL_IMAGE_LEASE_TRACE` instrumentation | generic renderer tracing points plus Photon event tracing | Trace follows import → wait → draw → commit → complete |
| XPC broker / descriptor policy | `presentation_xpc.rs` and Photon CLI | Photon presentation runtime | Broker auto-start, service propagation, and release remain outside GPUI-CE |
| AppKit wake / display scheduling | `gpui_macos` small changes | GPUI-CE macOS platform only if generic | No Bun wake path and no recurring 16 ms poll |
| AppKit material/titlebar | Photon window patch | reuse GPUI-CE `WindowOptions` and existing macOS window/background API | Native controls, blur, drag, resize, fullscreen, minimize |

Migration progress: the direct GPUI-CE app, PhotonWebView, and Photon-owned
broker client run behind `./photon run-gpui-ce`; the old GPUIX/Zed path remains
available. A plain GPUI-CE window visibly presented the localized Example
Domain page through the native IOSurface/Metal path. Runtime tracing confirmed
matching Engine and GPUI-CE Metal registry IDs, producer event values observed
by the consuming command buffers, texture imports followed by cache hits,
draw/commit/GPU completion, and `FrameReleased` delivered back to Engine only
after the replacement frame completed. Engine backing IDs 13, 14, and 15 then
cycled repeatedly. The compositor reported zero GPU-to-CPU bitmap readbacks.

The first runtime attempt exposed a sandbox crash: the Compositor was killed
when a Skia persistent-cache hit called `utimensat` to update file recency.
Cache reads now leave file metadata unchanged; eviction continues to use the
last successful store time. The focused regression test checks that a cache
read does not modify the entry timestamp.

The visual, shared-event, GPU-completion, release, cache-reuse, and backing
reuse gates are now demonstrated on the direct path. GPUIX/Zed removal and the
permanent `./photon run` switch remain deferred until remaining migration
checks (including input/redraw scheduling and packaging) are complete.
