# Native GPU presentation

## Current macOS baseline

The checked-out Mac is a MacBook Air (Mac17,4) with an Apple M5 GPU. CoreGraphics reports the built-in display's current mode as 3420x2214 at 60 Hz. Metal reports one available device, Apple M5 with registry ID `4294968262`; `MTLCreateSystemDefaultDevice()` returns that same ID. The mode is queried at runtime; it is not a Photon frame-rate setting.

The Mac build baseline was checked with `./photon setup`, `./photon format`, `./photon check`, `./photon test`, and `./photon build`. Setup keeps Engine on its persistent `master` branch. The GPUIX native test suite needed a headless-safe macOS controls registry: `TestAppContext` does not run on AppKit's main thread, so registry creation must not require `MainThreadMarker` until a native control is actually created.

## Existing presentation paths

Ladybird already has the useful macOS producer pieces:

- `LibGfx::SharedImageBuffer` owns a BGRA8 premultiplied IOSurface and exports it through a Mach send right.
- The Compositor paints published backing stores through Skia's Metal-backed `PaintingSurface` when Metal is available.
- Published backing stores have stable bitmap IDs, are pooled across frames, and macOS starts with three stores. The AppKit frontend presents an IOSurface to Core Animation and uses IOSurface usage to avoid reusing a backing still held by WindowServer.
- `Gfx::MetalContext` currently creates its own default Metal device and command queue. It does not expose the device registry ID through the Photon embedder API.

The Photon embedder currently opts out of this native presentation path with `notify_compositor_gpu_presentation_unavailable()`. `on_ready_to_paint` reads the bitmap and copies every row into `PresentedFrame::pixels`; GPUI then copies it into a `LiveImage` for upload. That remains a fallback path, not the target path.

GPUI has a native scene surface primitive. On macOS it accepts `CVPixelBuffer`; Linux accepts a type-erased wgpu texture. The Metal surface path supports an ordered rounded-clip stack and the offset nested-clip headless scene test passes. GPUI's Metal renderer owns a CAMetalLayer, selects a device by enumerating available devices, creates a command queue, encodes the scene, and presents a drawable. Its macOS frame scheduler uses per-display `CVDisplayLink` subscriptions. The renderer exposes its selected device name and registry ID. The application still needs to compare that ID with the Engine's producer device before native presentation is enabled.

On macOS, Photon no longer asks GPUIX to run its recurring 16 ms custom-element poll. Core's Unix event-loop backend registers its wake pipe and notifier file descriptors as `CFFileDescriptor` sources on the current CFRunLoop, and mirrors Core timers with `CFRunLoopTimer`s. Those callbacks call `EventLoop::pump(PollForEvents)` on the event-loop owner thread. PhotonWebView processes a nonblocking batch during render; Engine frame/cursor callbacks refresh GPUI through `AsyncApp`, so a new frame requests a scene redraw without a React state update. Non-macOS platforms retain the existing custom-element polling path. GPUI's own window scheduling remains driven by its per-display `CVDisplayLink`.

This integration has passed the macOS Engine build and root check/test commands and was exercised by launching Photon without the XPC broker: navigation and Engine frame callbacks arrived with the custom-element poll disabled. That run used CPU fallback and was only a startup smoke test; it does not establish native Metal frame pacing or benchmark performance. Core's event loop remains pumpable as a fallback. Native producer signaling from Ladybird's Skia command buffer and Engine compositor device identity are still unresolved.

## Standalone Metal milestone status

The in-process producer path is proven on the current Mac: GPUI selected Apple M5, registry ID `4294968262`; the producer GPU-rendered a BGRA8 IOSurface; GPUI imported and cached the backing; the scene command buffer waited on the producer's `MTLSharedEvent`; and Metal completion released the frame lease. A GPUI scene capture verified the four quadrants, orientation marker, radius-40 clipping, and resize behavior at scale 2. Five earlier example launch/close runs succeeded. These results prove only the in-process prototype; they do not prove cross-process Metal event transport or Ladybird integration.

The current patch adds a persistent IOSurface descriptor carrying a Mach send right and imports that right back to the same process, where identity and dimensions are verified. It intentionally does not serialize `MTLSharedEventHandle`: Apple requires that handle to be transferred as an object over `NSXPCConnection`; keyed archiving it to bytes fails. Ladybird's existing Mach IPC can carry the IOSurface port but cannot substitute for the required NSXPC event-handle transport. A broker/service and a two-process producer-consumer test remain necessary before Engine integration.

## Required native frame contract

The common presentation boundary should carry a platform frame descriptor, dimensions, format, generation, frame ID, and an owned lease. macOS descriptors carry an IOSurface reference and producer synchronization. Linux descriptors retain the existing Vulkan/DMA-BUF and sync-file representation. Browser state and React only see a frame-ready notification and viewport geometry; they never receive pixels or platform handles.

On macOS, GPUI must import the BGRA IOSurface into a Metal texture on the same `MTLDevice` registry ID used by Ladybird's producer. The compositor's MTLSharedEvent value must gate scene sampling, and GPUI's command-buffer completion must release the exact backing lease to Engine. A backing remains alive until that completion, including across resize generations. If native import or device matching fails, the existing CPU/LiveImage path is the fallback with a logged reason.

No native-presentation implementation is considered verified until a GPU-rendered four-quadrant IOSurface pattern is sampled in the GPUI scene, with real rounded clipping and scale tests. Engine FPS, accepted-frame FPS, and GPUI-presented FPS must be measured independently; display-link callback frequency is not evidence of rendered FPS.

## Ladybird adapter checkpoint (2026-09-30)

With `PHOTON_PRESENTATION_XPC_SERVICE=com.openai.codex.photon.metalprototype` and the GPUI example XPC broker bootstrapped as a per-user LaunchAgent, the Photon app entered native Metal mode and registered/imported Ladybird-owned BGRA IOSurfaces. At logical 1092x683 and DPR 2, GPUI accepted/presented those external surfaces. The observed page-load interval reported 1.8 Engine frames/sec and 1.8 GPUI-presented frames/sec; the Compositor reported 1.9 FPS. This was a short navigation/load observation, not the animated benchmark.

The profile for that run reported zero C++ frame-vector allocations, zero Rust callback-vector allocations, zero LiveImage vector allocations/updates, zero GPUI image uploads, and zero compositor GPU-to-CPU readbacks. A macOS desktop capture was unavailable in this environment, so the real webpage output was not visually inspected. The standalone GPUI scene screenshot remains the visual proof for the texture/shader path.

The manual broker setup above records the earlier adapter checkpoint. Normal `./photon run` now builds and registers a checkout-specific presentation broker automatically, then passes its XPC service and a fresh channel ID to Photon and the Engine helpers. The broker remains registered for subsequent runs and `./photon clean` removes it. Verbose output reports GPUI and the Photon event-signal device identity. The Engine compositor's MTLDevice registry ID is not exposed, so it is not yet independently compared. Ladybird calls the Photon adapter only after its Compositor GPU completion callback; Photon then signals its own shared event before GPUI submission. This is a safe post-completion handoff, not a signal encoded in Ladybird's Metal command buffer, and it does not permit producer/consumer GPU overlap.

Frame releases return to the embedder and defer reuse of leased bitmap IDs. Imported backing and texture caches do not yet have a complete generation-retirement path; repeated resize can retain old resources until the presentation/view is destroyed. GPUIX still wakes PhotonWebView's Engine pump on its existing 16 ms polling loop. Those are remaining lifecycle and scheduling tasks before sustained-animation acceptance. No Engine FPS claim beyond the short run above is made.
