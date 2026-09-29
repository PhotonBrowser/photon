# Architecture

Photon separates browser state, native presentation, and the web engine:

```text
ui/src/App.tsx
    React composition and layout
        ↓
@gpuix/react reconciler
        ↓
Photon source-built N-API addon
    GPUIX renderer and Photon factory registration
        ↓
crates/photon-gpui
    PhotonWebViewElement and engine adapter
        ↓ narrow C ABI
native/gpui + LibPhotonEmbedder
        ↓
Engine/ (Photon Engine / Ladybird)
```

## Ownership

- `ui/` contains the small React/TSX shell. It sets the initial URL and expresses layout; it has no engine or frame-buffer logic.
- `crates/photon-core` owns framework-independent browser state and command rules. It does not depend on GPUI, GPUIX, or Ladybird types.
- `crates/photon-gpui` owns the native web element, its GPUI presentation state, and the safe Rust-facing use of Photon Engine.
- `crates/photon-native-addon` is the composition root. It exports GPUIX's N-API API and links Photon factory registration into the same addon.
- `native/gpui` is the narrow C++ bridge to LibPhotonEmbedder. Engine implementation changes remain in `PhotonBrowser/photon-engine`.
- `vendor/gpuix` is the exact `PhotonBrowser/gpuix` git submodule revision shared by Rust and the `@gpuix/native` local package.

## Frame and layout path

`PresentedFrame` bytes remain native: Ladybird → LibPhotonEmbedder → Photon Rust → GPUI `RenderImage`. The React tree receives only element properties. Photon keeps the latest complete frame rather than queuing every frame.

The element measures its laid-out GPUI bounds and uses GPUI's scale factor to derive the engine's physical viewport. It resizes only when dimensions or scale change. The frame image remains in the GPUI scene, so normal scene clipping, transforms, and z-order apply.

The current frame path copies an owned BGRA bitmap into GPUI. It is a correct transitional CPU-backed presentation path, not zero-copy GPU sharing.

## Event processing and lifetime

The single `PhotonWebViewElement` owns the current engine session and view for this one-window spike. The session pumps the embedder as part of GPUIX's yielding custom-element update task; it does not spin on the UI thread. Frame callbacks coalesce into one latest-frame slot. When the native element is removed, it shuts down and destroys the view before destroying the runtime.

See [GPUIX integration](GPUIX.md) for the fork extension and addon loading model.

The Engine and GPUIX submodules point to Photon-maintained downstream repositories, not directly to their upstreams. Follow [Upstream maintenance](Upstream.md) to sync either upstream and update the tested commit pinned here.
