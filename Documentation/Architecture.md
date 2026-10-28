# Architecture

Photon is a Rust browser shell built with GPUI-CE and a Ladybird-derived
Engine. The shell talks to the Engine through Photon's explicit embedder API.
macOS is the primary development platform. Linux has a shell-only GPUI path and
a portable Engine renderer; end-to-end Linux browser runtime validation is
still outstanding. Windows Engine builds remain experimental. See
[Building Photon](Building.md) for platform validation status.

## How the pieces fit

```text
photon-app
└── photon-shell ── Photon Engine embedder API ── Engine (Ladybird)
    ├── photon-core          browser rules and state
    ├── photon-storage       profile, history, settings, and session files
    ├── photon-performance   performance samples and diagnostics
    ├── photon-brand         shared brand assets
    ├── gpui-motion          GPUI motion library
    └── platform adapters   macOS, Linux, and Windows integration

photon-ffi ── photon-core    browser-model C ABI for native clients
photon-cli                  setup, checks, build, run, and dependency tools
photon-presentation-broker ── photon-presentation-ipc  macOS presentation transport
```

`photon-core` contains browser policy and serializable models. It has no GPUI,
native window, or Engine dependencies. `photon-shell` owns the browser window,
views, live tab coordination, and Engine sessions. `photon-storage` handles
profile file I/O.

## Engine and platform boundaries

The shell calls the public C++ `Photon::Runtime` and `Photon::View` API from
[`PhotonEmbedder.h`](../Engine/Libraries/LibPhotonEmbedder/include/LibPhotonEmbedder/PhotonEmbedder.h).
[`PhotonEmbedderBridge.cpp`](../native/embedder/PhotonEmbedderBridge.cpp) adapts
that API to a private C ABI consumed by Rust. The separate
[`photon-ffi`](../crates/photon-ffi/include/photon_ffi.h) interface exposes
lightweight browser-model functions; it does not start the Engine.

Keep Engine implementation details behind the embedder API. Do not pass
Ladybird objects or page pixel buffers into shared UI code. Keep native window
and system behavior in `crates/photon-shell/src/platform/<target>/`; keep
shared views in `platform/ui/` and shared Engine calls in `platform/engine/`.
Generic framework behavior belongs in the pinned `vendor/gpui-ce` submodule.

| Platform | Current implementation |
| --- | --- |
| macOS | AppKit integration and IOSurface/Metal presentation; primary development target |
| Linux | Shared GPUI shell, custom in-window menus and `<select>` popovers, and CPU-backed page frames; the UI-only build skips Engine setup; full browser runtime remains unverified |
| Windows | Shared shell and CPU-backed frames; native Engine build remains experimental |

The Linux renderer uses the same opaque `PresentedSurface` consumed by the
shared WebView. `platform/linux/renderer.rs` selects the portable CPU-frame
handoff; the shared UI never sees Engine surfaces or page pixel buffers. A
future DMA-BUF presenter can replace that target adapter behind the same
boundary. Shell-only startup constructs no Engine runtime and opens Photon’s
New Tab page so GPUI chrome and internal pages can be developed independently.

## Where to start

| Change | Location |
| --- | --- |
| Browser rules, navigation, or state | [`crates/photon-core`](../crates/photon-core/src/lib.rs) |
| Profile and persisted data | [`crates/photon-storage`](../crates/photon-storage/src/lib.rs) |
| Shared browser views and interactions | [`crates/photon-shell/src/platform/ui`](../crates/photon-shell/src/platform/ui/mod.rs) |
| Engine sessions and callbacks | [`crates/photon-shell/src/platform/engine`](../crates/photon-shell/src/platform/engine/mod.rs) |
| Native platform behavior | [`crates/photon-shell/src/platform`](../crates/photon-shell/src/platform/mod.rs) |
| Engine bridge | [`native/embedder`](../native/embedder/PhotonEmbedderBridge.cpp) and [`engine_ffi.rs`](../crates/photon-shell/src/platform/engine_ffi.rs) |
| Shared UI styling | [`theme.toml`](../crates/photon-shell/theme.toml), [`theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs), and [`metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) |
| Generic GPUI-CE behavior | [`vendor/gpui-ce`](../vendor/gpui-ce/README.md) |

For dependency branches and pins, see [Upstream maintenance](Upstream.md). For
browser UI details, see [PhotonWebView](WebView.md), [Photon pages](Pages.md),
[profile and settings](Profile.md), and [theme tokens](Theme.md).
