# PhotonWebView bootstrap

## Runtime and view lifecycle

`PhotonWebView` creates one `Photon::Runtime`, then one `Photon::View`. Runtime creation delegates to Ladybird's existing `WebView::Application` initialization, which starts the helper process infrastructure. The view wraps a `HeadlessWebView` and owns page/WebContent state. For this integration milestone the view navigates to `https://example.com` at startup.

The Qt shell calls `Runtime::pump()` from a 5 ms GUI-thread timer. This dispatches the current engine event loop; callbacks are synchronous on that same thread. `ViewCallbacks` currently carries basic URL/title/loading/history state, errors, and immutable shared frame values. `PhotonWebView` destroys the view before the runtime.

## Presentation contract and ownership

The temporary presentation path is CPU-backed:

```text
WebContent / Compositor CPU bitmap
  -> LibPhotonEmbedder copies the bitmap during its paint callback
  -> owned PresentedFrame (BGRA8888 premultiplied)
  -> Qt callback copies to QImage under a mutex
  -> PhotonWebView::update()
  -> updatePaintNode() creates a QSGTexture from the QImage
  -> QQuick scene graph
```

This is not zero-copy or GPU-shared. Photon sets `WebContentOptions::force_cpu_painting` to `Yes`; the shared cross-process backing store is therefore a CPU shareable bitmap. Ladybird copies the current bitmap synchronously into `PresentedFrame`, whose pixel vector owns the bytes. Qt then copies those bytes into an owned `QImage`. The mutex protects the pending image slot. `updatePaintNode()` creates a scene-graph texture through the Qt Quick update lifecycle. Qt owns the scene-graph texture; Photon owns the copied image/frame bytes.

### Native GPU investigation (Qt 6.11.2, Engine `a89f1bae13450067d8abc1bdcf2da408d19951f8`)

The pinned Engine contains Vulkan DMA-BUF backing-store support, but Photon does not use it. When enabled, `BackingStoreManager` allocates two `VK_FORMAT_B8G8R8A8_UNORM` Vulkan images with `VK_IMAGE_TILING_DRM_FORMAT_MODIFIER_EXT`, requests only `DRM_FORMAT_MOD_LINEAR`, and requires one plane. It prefers host-visible/coherent/cached memory for linear allocations, otherwise host-visible/coherent memory; allocation is dedicated and exports `VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT`. Export metadata currently contains BGRA/alpha type, size, DRM format, one pitch, modifier and an fd. It does not describe plane count/offsets or an acquire/release synchronization object. `VulkanImage::transition_layout()` transitions `UNDEFINED` to `GENERAL` on the producer graphics queue with both queue-family indices `VK_QUEUE_FAMILY_IGNORED`. It waits that queue idle for this initial transition. Rendering later writes through Skia; this publication path does not signal/export a semaphore or fence and does not transfer ownership to `VK_QUEUE_FAMILY_EXTERNAL`/`FOREIGN_EXT`. It makes no documented producer-completion guarantee for an external consumer.

The producer selects a discrete Vulkan GPU if present, otherwise the first enumerated device, then its first graphics queue family. Qt Quick is Qt 6.11.2 in this checkout and independently initializes its scene graph; Photon does not set a `QQuickGraphicsDevice` or constrain Qt's adapter. Same physical device is therefore not guaranteed, including on multi-GPU systems. Qt exposes `QSGVulkanTexture::fromNative(VkImage, VkImageLayout, QQuickWindow*, QSize, options)` and `QQuickGraphicsDevice::fromDeviceObjects(...)`, but the native texture API accepts neither imported memory ownership nor producer synchronization. A DMA-BUF would first need to be imported as a VkImage on Qt's VkDevice; Qt does not offer a supported API here to inject producer semaphore waits or coordinate external queue ownership while tracking that image in its scene graph. Using this API with the Engine's image/fd would be unsafe.

The Engine's current compositor backing-store pool has two images. The initial published image is reserved as `Presented`; subsequent rendering marks an image `Presented`. The UI releases an old image when it switches front bitmap, and compositor callbacks release unused/dropped images. That acknowledgement is tied to the engine's bitmap swap, not to completion of Qt's later scene-graph sampling. Photon currently copies pixels synchronously inside the callback, so the engine backing is no longer referenced when callback handling returns. A native path must introduce an explicit lease/release boundary covering actual Qt GPU consumption, including frames dropped during coalescing and old resize generations.

Accordingly, Photon keeps the CPU fallback. The next safe native step is an Engine embedder contract that exports a platform-neutral presentation lease and reports producer completion plus consumer release. On Linux the native backend must also provide/import the image on Qt's exact Vulkan device, describe all plane metadata and layout, and transfer queue-family ownership with explicit acquire/release synchronization. Until Qt offers a supported synchronization point for sampling/retirement (or the Engine provides a supported shared-device integration), no Linux native import should be enabled. macOS should map Engine IOSurface/Metal presentation into Qt's Metal RHI; Windows should map a shared D3D resource into Qt's D3D RHI. Those platform handles and synchronization details belong in native presentation backends, never QML, Rust browser state, or generic PhotonView APIs.

The fallback remains observable with verbose metrics: Engine CPU copy duration, Qt image-copy duration, texture upload duration, received/presented/coalesced counts, frame dimensions and DPR. It currently performs two full-frame CPU copies (Engine bitmap to owned frame, owned frame to `QImage`) and one GPU texture upload. Zero-copy is not achieved.

## Sizing and input

The Qt item reports logical dimensions and the window's device-pixel ratio. LibPhotonEmbedder computes physical pixels as rounded `logical_size * DPR` and resets the underlying WebView viewport. Item geometry and screen changes trigger resize.

`PhotonWebView` translates Qt mouse move/press/release, hover, wheel, key press/release, text code points, modifiers, auto-repeat, and focus changes into the corresponding LibPhotonEmbedder event types. Item-local logical coordinates are converted to device pixels in the engine boundary; screen coordinates are currently passed through the same conversion. A dedicated end-to-end interaction regression check is still needed.

## State and diagnostics

Engine callbacks expose URL, title, loading, can-go-back, can-go-forward and WebContent failure. `PhotonWebView` forwards these callbacks through `BrowserController` into Rust `BrowserState`; QML reads the Rust-backed properties through that adapter. With `PHOTON_VERBOSE=1` (or `./photon --verbose run`), the Qt shell reports five-second aggregates for frames received/presented/coalesced, the latest frame size and DPR, engine copy time, Qt image-copy time, and texture-upload time.

## Current limitations

- The first URL is fixed to `https://example.com`.
- Presentation is CPU-backed with full-frame copies and Qt texture uploads.
- No GPU-native shared presentation path exists yet.
- Rust owns state and commands for the single active view; C++ still owns the `Runtime` and `View` objects.
- Search queries, suggestions, and multiple tabs are not implemented.
- Input translation compiles but needs focused manual interaction verification.
- The QML language server needs the generated `ui/.qmlls.ini` build-directory hint. CMake refreshes this ignored file during configuration so editors can resolve the `Photon` module and `PhotonWebView` type.
