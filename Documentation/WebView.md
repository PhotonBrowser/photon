# PhotonWebView bootstrap

## Runtime and view lifecycle

`PhotonWebView` creates one `Photon::Runtime`, then one `Photon::View`. Runtime creation delegates to Ladybird's existing `WebView::Application` initialization, which starts the helper process infrastructure. The view wraps a `HeadlessWebView` and owns page/WebContent state. For this integration milestone the view navigates to `https://example.com` at startup.

The Qt shell calls `Runtime::pump()` from a 5 ms GUI-thread timer. This dispatches the current engine event loop; callbacks are synchronous on that same thread. `ViewCallbacks` currently carries basic URL/title/loading/history state, errors, and immutable shared frame values. `PhotonWebView` destroys the view before the runtime.

## Presentation and ownership

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

This is not zero-copy or GPU-shared. Ladybird bitmap memory is read synchronously in the callback and never retained. `PresentedFrame` owns its pixel vector; Qt copies those bytes into `QImage` before the callback returns. The mutex protects the pending image slot. Texture creation and scene-graph node changes occur in `updatePaintNode()`, following Qt Quick's update lifecycle. Qt owns the displayed texture node resource.

The CPU copy/upload path is a correctness bridge for proving scene composition. A future GPU path should replace the `PresentedFrame` payload behind LibPhotonEmbedder with an explicit native presentation object and a documented synchronization/ownership contract. No DMA-BUF or cross-API handle sharing is implemented here.

## Sizing and input

The Qt item reports logical dimensions and the window's device-pixel ratio. LibPhotonEmbedder computes physical pixels as rounded `logical_size * DPR` and resets the underlying WebView viewport. Item geometry and screen changes trigger resize.

`PhotonWebView` translates Qt mouse move/press/release, hover, wheel, key press/release, text code points, modifiers, auto-repeat, and focus changes into the corresponding LibPhotonEmbedder event types. Item-local logical coordinates are converted to device pixels in the engine boundary; screen coordinates are currently passed through the same conversion. A dedicated end-to-end interaction regression check is still needed.

## State and diagnostics

Engine callbacks expose URL, title, loading, can-go-back, can-go-forward and WebContent failure. The current shell logs state changes. With `PHOTON_VERBOSE=1` (or `./photon run --verbose`), the Qt shell reports five-second aggregates for frames received/presented/coalesced, the latest frame size and DPR, engine copy time, Qt image-copy time, and texture-upload time.

## Current limitations

- The first URL is fixed to `https://example.com`.
- Presentation is CPU-backed with full-frame copies and Qt texture uploads.
- No GPU-native shared presentation path exists yet.
- Rust browser state does not yet own or command the runtime/view.
- Input translation compiles but needs focused manual interaction verification.
- The QML language server needs the generated `ui/.qmlls.ini` build-directory hint. CMake refreshes this ignored file during configuration so editors can resolve the `Photon` module and `PhotonWebView` type.
