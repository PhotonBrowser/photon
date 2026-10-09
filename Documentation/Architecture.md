# Architecture

```text
photon-app
    └── photon-shell
        ├── GPUI-CE window and Photon browser UI
        ├── photon-performance diagnostics and overlay
        ├── Photon Engine session adapter
        │       ↓ narrow native embedder API
        │   Photon Engine / Ladybird
        └── native presentation lifecycle
                ↓ uses
            photon-presentation-ipc ←── photon-presentation-broker
                ↓ XPC descriptors
            IOSurface + MTLSharedEvent → GPUI-CE external Metal surface

photon-ffi ── photon-core ── photon-omnibox
```

The desktop app uses the framework-independent Rust browser model directly.
`photon-ffi` is the separate static library for native callers that need the C
API; it is not part of the desktop shell's internal state path.

## Ownership

- `crates/photon-core` contains framework-independent browser state, commands,
  and address normalization. It has no GPUI, GPUI-CE, native, or Ladybird
  types.
- `crates/photon-omnibox` owns address-versus-search classification and search
  engine data. Core and the shell use the same rules.
- `crates/photon-ffi` adapts the safe core model to the exported
  `photon_browser_*` C ABI. It owns pointer validation, C strings, and ABI
  state, and builds as both an `rlib` and a static library.
- `crates/photon-app` is the runnable entry point. `crates/photon-shell` owns
  the native window, window-bound GPUI views, `PhotonWebView`, Engine session,
  presentation state, frame ordering, browser input, and shutdown lifecycle.
  The performance diagnostics model and GPUI overlay live in
  `crates/photon-performance`; the shell supplies Engine samples and theme
  colors.
- `crates/photon-performance` owns performance snapshots, monitor state, timing
  accumulation, value formatting, and the GPUI debug overlay. It has no Engine
  IPC, native embedder, or window-lifecycle dependencies.
- `crates/photon-shortcuts` owns browser-level GPUI actions and default key
  bindings.
- `crates/photon-presentation-ipc` owns the macOS XPC protocol, Rust client
  channel, native descriptor transport, and service entry point. The separate
  `crates/photon-presentation-broker` package is the small service executable.
- `native/embedder` is the narrow C++ bridge to LibPhotonEmbedder. Engine
  implementation changes belong in the Photon Engine submodule.
- `vendor/gpui-ce` contains generic external Metal surface rendering and
  platform capabilities. Browser and Ladybird lifecycle policy stays in
  Photon.

## Rust package map

```text
crates/
├── photon-app/                       # Runnable Photon entry point
├── photon-shell/                     # GPUI-CE shell and native Engine adapter
│   └── src/platform/
│       ├── engine/                   # Engine runtime and per-tab sessions
│       │   └── callbacks.rs          # Engine C callbacks and the state they update
│       ├── presentation.rs           # IOSurface frames, leases, GPU completion
│       ├── window_settings.rs        # Native window options and material choice
│       └── ui/
│           ├── window/               # Browser window: layout, commands, rendering
│           │   ├── app.rs            # App startup and the first window
│           │   ├── tabs.rs           # Tab lifecycle and the tab strip
│           │   ├── menu.rs           # Browser menu and theme choice
│           │   ├── actions.rs        # Keyboard shortcuts
│           │   └── alerts.rs         # Crash and restart notices, JavaScript dialogs
│           ├── webview.rs            # Page surface composition
│           ├── js_dialog.rs          # JavaScript alert, confirm and prompt
│           ├── modal.rs              # Reusable centered modal
│           ├── button.rs             # Shared text buttons
│           ├── status_chip.rs        # Bottom-right crash and restart chips
│           ├── input.rs              # Keyboard, pointer, and scroll forwarding
│           ├── theme.rs              # Appearance and semantic color tokens
│           └── metrics.rs            # Shared UI dimensions and typography
├── photon-performance/               # Performance diagnostics model and overlay
├── photon-core/                      # Framework-independent browser model
├── photon-omnibox/                   # Search engines and address resolution
├── photon-ffi/                       # Exported C API and static library
├── photon-shortcuts/                 # Browser actions and key bindings
├── photon-cli/                       # `./photon` developer commands
├── photon-presentation-ipc/          # XPC protocol, client, and native transport
└── photon-presentation-broker/       # macOS XPC service executable
```

| Package | Owns | Depends on |
| --- | --- | --- |
| `photon-app` | Runnable Photon entry point | `photon-shell` |
| `photon-shell` | Window-bound GPUI views, Engine session adapter, native presentation lifecycle, browser input | GPUI-CE, `photon-core`, `photon-performance`, `photon-shortcuts`, `photon-presentation-ipc`, native embedder bridge |
| `photon-performance` | Performance snapshots, monitor state, timing accumulation, formatting, GPUI overlay | GPUI-CE |
| `photon-core` | Browser state, commands, shared address normalization | `photon-omnibox` |
| `photon-omnibox` | Search engine list and address/query resolution | URL parsing library |
| `photon-ffi` | `photon_browser_*` C ABI and static library | `photon-core` |
| `photon-shortcuts` | Browser actions and default key bindings | GPUI-CE |
| `photon-cli` | `./photon` developer and runtime commands | CLI and command-line support libraries |
| `photon-presentation-ipc` | XPC protocol, client channel, descriptor transport, service entry point | macOS frameworks |
| `photon-presentation-broker` | macOS XPC service executable | `photon-presentation-ipc` |

`Engine/` and `vendor/gpui-ce/` are separately maintained submodules, not
Photon workspace packages. `native/embedder` is the small native build target
owned by the shell; the presentation XPC implementation lives inside
`photon-presentation-ipc`.

## Common edit points

| Change | Start here |
| --- | --- |
| Performance diagnostics or overlay | [`photon-performance`](../crates/photon-performance/src/lib.rs) and the Engine adapter |
| GPUI-CE native window size, titlebar, traffic lights, blur | [`platform/window_settings.rs`](../crates/photon-shell/src/platform/window_settings.rs) and [`ui/metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) |
| Shell colors, appearance, or theme mapping | [`ui/theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs) |
| Shared layout, spacing, corner radii, or type sizes | [`ui/metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) |
| Window layout or app startup | [`ui/window/`](../crates/photon-shell/src/platform/ui/window/mod.rs) |
| Dialogs, modals, and buttons | [`ui/js_dialog.rs`](../crates/photon-shell/src/platform/ui/js_dialog.rs), [`ui/modal.rs`](../crates/photon-shell/src/platform/ui/modal.rs), [`ui/button.rs`](../crates/photon-shell/src/platform/ui/button.rs), and [`photon-core/src/dialogs.rs`](../crates/photon-core/src/dialogs.rs) |
| Crash recovery and its notices | [`photon-core/src/crashes.rs`](../crates/photon-core/src/crashes.rs), [`ui/window/alerts.rs`](../crates/photon-shell/src/platform/ui/window/alerts.rs), and [`ui/status_chip.rs`](../crates/photon-shell/src/platform/ui/status_chip.rs) |
| Page surface composition | [`ui/webview.rs`](../crates/photon-shell/src/platform/ui/webview.rs) |
| Keyboard, pointer, or scroll forwarding | [`ui/input.rs`](../crates/photon-shell/src/platform/ui/input.rs) |
| Engine session or callback behavior | [`platform/engine/`](../crates/photon-shell/src/platform/engine/mod.rs) |
| Frame acceptance, presentation order, or release lifetime | [`platform/presentation.rs`](../crates/photon-shell/src/platform/presentation.rs) |
| XPC messages, descriptor transport, or broker connection | [`photon-presentation-ipc`](../crates/photon-presentation-ipc/src/lib.rs) |
| Engine frame pacing or pausing Engine while the window is occluded | [`platform/display.rs`](../crates/photon-shell/src/platform/display.rs), [`platform/window_observer.rs`](../crates/photon-shell/src/platform/window_observer.rs), then [`ui/window/`](../crates/photon-shell/src/platform/ui/window/mod.rs) |
| Rust declarations for native embedder functions | [`platform/ffi.rs`](../crates/photon-shell/src/platform/ffi.rs) and [`PhotonEmbedderBridge.h`](../native/embedder/PhotonEmbedderBridge.h) |
| Browser state and command rules | [`photon-core/src/state.rs`](../crates/photon-core/src/state.rs) |
| C ABI exposed to native callers | [`photon-ffi/include/photon_ffi.h`](../crates/photon-ffi/include/photon_ffi.h) and [`photon-ffi/src/api.rs`](../crates/photon-ffi/src/api.rs) |
| Search engines or address/query classification | [`photon-omnibox/src/`](../crates/photon-omnibox/src/lib.rs) |
| `./photon` command behavior | [`photon-cli/src/commands/`](../crates/photon-cli/src/commands/) |

Keep window policy in `window_settings.rs`, shared presentation values in
`ui/metrics.rs`, shell color roles in `ui/theme.rs`, browser rules in core and
omnibox, frame lifecycle in the shell presentation module, and XPC transport in
`photon-presentation-ipc`. Add a crate only when the code has an independent
ownership boundary or a useful consumer outside its current crate.

## Frame and lifetime path

Ladybird's Skia compositor writes a leased IOSurface and signals an
`MTLSharedEvent`. Photon receives the frame descriptor through its XPC broker,
resolves the IOSurface once, and passes an external surface to GPUI-CE. The
Metal renderer caches the texture by resource, generation, and actual IOSurface
identity; it encodes the producer wait in the command buffer that samples the
texture.

When a replacement surface's command buffer completes, Photon releases the
retired Engine frame. On shutdown it stops new presentation, waits for tracked
sampling command buffers, releases the current and queued leases, and asserts
that outstanding leases are zero.

Photon UI code never receives page pixel buffers. Normal rendering uses native
Metal sampling with no CPU full-frame copies or GPUI image uploads. The app
currently targets macOS for external IOSurface presentation.

See [PhotonWebView](WebView.md), [native GPU presentation](NativeGpuPresentation.md),
[theme and style tokens](Theme.md), and [upstream maintenance](Upstream.md).
