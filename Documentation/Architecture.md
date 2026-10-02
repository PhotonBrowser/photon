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
- `crates/photon-app` owns the application, native window, `PhotonWebView`, presentation state, frame ordering, keyboard/pointer forwarding, and Engine release delivery. Engine readiness uses Ladybird's Core CFRunLoop integration to schedule GPUI updates; Metal completion signals the native release-drain source.
- `crates/photon-presentation-broker` and `native/presentation` own macOS XPC service startup and IOSurface/shared-event descriptor transport.
- `native/embedder` is the narrow C++ bridge to LibPhotonEmbedder. Engine implementation changes remain in the Photon Engine submodule.
- `vendor/gpui-ce` contains only generic external Metal surface rendering and platform capabilities. Browser and Ladybird lifecycle policy stays in Photon.

## Crate and module map

```text
crates/
├── photon-app/                       # Runnable GPUI-CE shell
│   └── src/
│       ├── main.rs                   # Platform gate and app entry point
│       └── platform/
│           ├── mod.rs                # macOS shell module wiring and trace helper
│           ├── window_settings.rs    # GPUI window size, titlebar, blur, insets, radii
│           ├── ffi.rs                # Rust declarations for the native C ABI
│           ├── engine.rs             # Engine runtime/session, callbacks, input forwarding
│           ├── presentation.rs       # IOSurface frames, GPU completion, lease tracking
│           ├── presentation_xpc.rs   # XPC broker client and descriptor transport
│           └── ui/
│               ├── window.rs         # Top-level window layout and app bootstrap
│               └── webview.rs        # Page surface and pointer/keyboard/scroll input
├── photon-core/                      # UI-independent browser state and C API
│   └── src/
│       ├── lib.rs                    # Public crate API
│       ├── state.rs                  # BrowserState, commands, Engine events, URL adapter
│       └── api.rs                    # Exported photon_browser_* C ABI implementation
├── photon-omnibox/                   # Address-vs-search rules, independent of the UI
│   └── src/
│       ├── lib.rs                    # Public crate API
│       ├── engines.rs                # SearchEngine values, built-ins, registry
│       └── resolve.rs                # Address classification and URL resolution
├── photon-cli/                       # `./photon` developer and runtime commands
│   └── src/
│       ├── cli.rs                    # CLI definition and dispatch
│       ├── support.rs                # Shared command helpers
│       └── commands/                 # One module per command
└── photon-presentation-broker/       # macOS XPC broker service executable
```

### Common edit points

| Change | Start here |
| --- | --- |
| GPUI-CE native window size, titlebar, traffic lights, blur | [`platform/window_settings.rs`](../crates/photon-app/src/platform/window_settings.rs) |
| Page inset, rounded surface, pointer-coordinate mapping | [`platform/window_settings.rs`](../crates/photon-app/src/platform/window_settings.rs), then [`platform/ui/webview.rs`](../crates/photon-app/src/platform/ui/webview.rs) |
| Window layout or app startup | [`platform/ui/window.rs`](../crates/photon-app/src/platform/ui/window.rs) |
| Page rendering or keyboard, pointer, scroll input | [`platform/ui/webview.rs`](../crates/photon-app/src/platform/ui/webview.rs) |
| Engine session or callback behavior | [`platform/engine.rs`](../crates/photon-app/src/platform/engine.rs) |
| Frame acceptance, presentation order, or release lifetime | [`platform/presentation.rs`](../crates/photon-app/src/platform/presentation.rs) |
| Rust declarations for native embedder functions | [`platform/ffi.rs`](../crates/photon-app/src/platform/ffi.rs) and [`native/embedder/PhotonEmbedderBridge.h`](../native/embedder/PhotonEmbedderBridge.h) |
| Browser state and command rules | [`photon-core/src/state.rs`](../crates/photon-core/src/state.rs) |
| C API exposed to the native bridge | [`photon-core/src/api.rs`](../crates/photon-core/src/api.rs) |
| Search engine list or default registry behavior | [`photon-omnibox/src/engines.rs`](../crates/photon-omnibox/src/engines.rs) |
| Address or query classification | [`photon-omnibox/src/resolve.rs`](../crates/photon-omnibox/src/resolve.rs) |
| `./photon` command behavior | [`photon-cli/src/commands/`](../crates/photon-cli/src/commands/) |

`photon-app/src/platform/mod.rs` only wires the macOS implementation together. Keep window policy in `window_settings.rs`, visible composition in `ui/`, browser rules in `photon-core`/`photon-omnibox`, and native lifecycle code in the Engine/presentation modules.

## Frame and lifetime path

Ladybird's Skia compositor writes a leased IOSurface and signals an `MTLSharedEvent`. Photon receives the frame descriptor through its XPC broker, resolves the IOSurface once, and passes an external surface to GPUI-CE. The Metal renderer caches the texture by resource, generation, and actual IOSurface identity; it encodes the producer wait in the command buffer that samples the texture.

When a replacement surface's command buffer completes, Photon releases the retired Engine frame. On shutdown it stops new presentation, waits for tracked sampling command buffers, releases the current and queued leases, and asserts that outstanding leases are zero.

Photon UI code never receives page pixel buffers. Normal rendering uses native Metal sampling with no CPU full-frame copies or GPUI image uploads. The app currently targets macOS for external IOSurface presentation.

See [PhotonWebView](WebView.md), [native GPU presentation](NativeGpuPresentation.md), and [upstream maintenance](Upstream.md).
