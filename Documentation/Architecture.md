# Architecture

```text
crates/photon-app
    Photon Rust application and native GPUI-CE window
        ↓
    PhotonWebView
        ↓ narrow C ABI
native/embedder + LibPhotonEmbedder
        ↓
Engine/ (Photon Engine / Ladybird)

Engine IOSurface + MTLSharedEvent
        ↓ XPC descriptor broker
GPUI-CE external Metal surface
        ↓
native CAMetalLayer
```

## Ownership

- `crates/photon-core` contains framework-independent browser state and command rules. It does not depend on GPUI, GPUI-CE, or Ladybird types.
- `crates/photon-omnibox` decides whether typed text is an address or a search query.
- `crates/photon-app` owns the application, native window, `PhotonWebView`, presentation state, frame ordering, basic keyboard/pointer forwarding, and Engine release delivery. The current frame pump polls at 16 ms and remains a follow-up for event-driven invalidation.
- `crates/photon-presentation-broker` and `native/presentation` own macOS XPC service startup and IOSurface/shared-event descriptor transport.
- `native/embedder` is the narrow C++ bridge to LibPhotonEmbedder. Engine implementation changes remain in the Photon Engine submodule.
- `vendor/gpui-ce` contains only generic external Metal surface rendering and platform capabilities. Browser and Ladybird lifecycle policy stays in Photon.

## Frame and lifetime path

Ladybird's Skia compositor writes a leased IOSurface and signals an `MTLSharedEvent`. Photon receives the frame descriptor through its XPC broker, resolves the IOSurface once, and passes an external surface to GPUI-CE. The Metal renderer caches the texture by resource, generation, and actual IOSurface identity; it encodes the producer wait in the command buffer that samples the texture.

When a replacement surface's command buffer completes, Photon releases the retired Engine frame. On shutdown it stops new presentation, waits for tracked sampling command buffers, releases the current and queued leases, and asserts that outstanding leases are zero.

Photon UI code never receives page pixel buffers. Normal rendering uses native Metal sampling with no CPU full-frame copies or GPUI image uploads. The app currently targets macOS for external IOSurface presentation.

See [PhotonWebView](WebView.md), [native GPU presentation](NativeGpuPresentation.md), and [upstream maintenance](Upstream.md).
