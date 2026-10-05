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
- `crates/photon-app` is the runnable entry point. `crates/photon-shell` owns the native window, `PhotonWebView`, presentation state, frame ordering, keyboard/pointer forwarding, and Engine release delivery. Engine readiness uses Ladybird's Core CFRunLoop integration to schedule GPUI updates; Metal completion signals the native release-drain source.
- `crates/photon-presentation-broker` and `native/presentation` own macOS XPC service startup and IOSurface/shared-event descriptor transport.
- `native/embedder` is the narrow C++ bridge to LibPhotonEmbedder. Engine implementation changes remain in the Photon Engine submodule.
- `vendor/gpui-ce` contains only generic external Metal surface rendering and platform capabilities. Browser and Ladybird lifecycle policy stays in Photon.

## Rust package map

Photon's Rust workspace is split by ownership and dependency direction. These
six packages are the Photon-owned boundaries that currently have independent
responsibilities; keep new packages focused on a real reusable or separately
launched component rather than mirroring every Engine subsystem.

```text
photon-cli                     developer and runtime commands
photon-app ────────────────── photon-shell
                                 ├── GPUI-CE shell + native/embedder + Engine/
                                 └── photon-omnibox
photon-core ───────────────── photon-omnibox
photon-presentation-broker ─── native/presentation
```

`Engine/` and `vendor/gpui-ce/` are separately maintained submodules, not
Photon workspace packages. `native/embedder` and `native/presentation` are
small native build targets owned by their Rust shell/service packages.

## Crate and module map

```text
crates/
├── photon-app/                       # Runnable Photon entry point
│   └── src/main.rs                   # Launches photon-shell
├── photon-shell/                     # GPUI-CE shell and native Engine adapter
│   └── src/platform/
│           ├── mod.rs                # macOS shell module wiring and trace helper
│           ├── window_settings.rs    # GPUI window size, titlebar, blur, insets, radii
│           ├── ffi.rs                # Rust declarations for the native C ABI
│           ├── engine.rs             # Engine runtime/session, callbacks, input forwarding
│           ├── presentation.rs       # IOSurface frames, GPU completion, lease tracking
│           ├── presentation_xpc.rs   # XPC broker client and descriptor transport
│           └── ui/
│               ├── window.rs         # Top-level window layout and app bootstrap
│               ├── webview.rs        # Page surface composition
│               ├── input.rs          # Keyboard, pointer, and scroll forwarding
│               └── theme.rs          # Shell visual theme
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

| Package | Owns | Depends on |
| --- | --- | --- |
| `photon-app` | Runnable Photon entry point | `photon-shell` |
| `photon-shell` | GPUI-CE window, Engine session adapter, native presentation lifecycle, browser input forwarding | GPUI-CE, `photon-omnibox`, native embedder bridge |
| `photon-core` | Framework-independent browser state and exported C API | `photon-omnibox` |
| `photon-omnibox` | Search engines and address/query resolution | URL parsing library |
| `photon-cli` | `./photon` developer and runtime commands | CLI and command-line support libraries |
| `photon-presentation-broker` | macOS XPC service entry point and native descriptor transport | Native presentation service glue |

### Common edit points

| Change | Start here |
| --- | --- |
| GPUI-CE native window size, titlebar, traffic lights, blur | [`platform/window_settings.rs`](../crates/photon-shell/src/platform/window_settings.rs) |
| Page inset, rounded surface, pointer-coordinate mapping | [`platform/window_settings.rs`](../crates/photon-shell/src/platform/window_settings.rs), then [`platform/ui/webview.rs`](../crates/photon-shell/src/platform/ui/webview.rs) |
| Window layout or app startup | [`platform/ui/window.rs`](../crates/photon-shell/src/platform/ui/window.rs) |
| Page surface composition | [`platform/ui/webview.rs`](../crates/photon-shell/src/platform/ui/webview.rs) |
| Keyboard, pointer, or scroll forwarding | [`platform/ui/input.rs`](../crates/photon-shell/src/platform/ui/input.rs) |
| Engine session or callback behavior | [`platform/engine.rs`](../crates/photon-shell/src/platform/engine.rs) |
| Frame acceptance, presentation order, or release lifetime | [`platform/presentation.rs`](../crates/photon-shell/src/platform/presentation.rs) |
| Rust declarations for native embedder functions | [`platform/ffi.rs`](../crates/photon-shell/src/platform/ffi.rs) and [`native/embedder/PhotonEmbedderBridge.h`](../native/embedder/PhotonEmbedderBridge.h) |
| Browser state and command rules | [`photon-core/src/state.rs`](../crates/photon-core/src/state.rs) |
| C API exposed to the native bridge | [`photon-core/src/api.rs`](../crates/photon-core/src/api.rs) |
| Search engine list or default registry behavior | [`photon-omnibox/src/engines.rs`](../crates/photon-omnibox/src/engines.rs) |
| Address or query classification | [`photon-omnibox/src/resolve.rs`](../crates/photon-omnibox/src/resolve.rs) |
| `./photon` command behavior | [`photon-cli/src/commands/`](../crates/photon-cli/src/commands/) |

`photon-shell/src/platform/mod.rs` only wires the macOS implementation together. Keep window policy in `window_settings.rs`, visible composition in `ui/`, browser rules in `photon-core`/`photon-omnibox`, and native lifecycle code in the Engine/presentation modules.

## Frame and lifetime path

Ladybird's Skia compositor writes a leased IOSurface and signals an `MTLSharedEvent`. Photon receives the frame descriptor through its XPC broker, resolves the IOSurface once, and passes an external surface to GPUI-CE. The Metal renderer caches the texture by resource, generation, and actual IOSurface identity; it encodes the producer wait in the command buffer that samples the texture.

When a replacement surface's command buffer completes, Photon releases the retired Engine frame. On shutdown it stops new presentation, waits for tracked sampling command buffers, releases the current and queued leases, and asserts that outstanding leases are zero.

Photon UI code never receives page pixel buffers. Normal rendering uses native Metal sampling with no CPU full-frame copies or GPUI image uploads. The app currently targets macOS for external IOSurface presentation.

See [PhotonWebView](WebView.md), [native GPU presentation](NativeGpuPresentation.md), and [upstream maintenance](Upstream.md).
