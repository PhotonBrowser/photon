# Photon architecture

Photon’s desktop browser is built from a Rust application and the Ladybird
browser Engine. GPUI-CE draws the desktop interface; the Engine loads and
renders desktop web pages. The iOS target in [`ios/`](../ios/README.md) is a
native app host with a Rust-backed browser page adapter and WebKit for website
content. It does not use Ladybird or the desktop Engine embedder.

This guide is for people who want to understand how Photon fits together or
find the right place to make a change. For installation and build steps, see
[Building Photon](Building.md).

## The short version

```text
Photon desktop                              Photon iOS
     │                                           │
     ▼                                           ▼
Rust + GPUI-CE shell                   SwiftUI home surface
     │                                           │
     ├── photon-core                       AppServices
     ├── photon-storage                        │
     └── Photon Engine API          WebKitBrowserAdapter
                │                       ┌───────┴───────┐
                ▼                       ▼               ▼
          Ladybird Engine        photon-ffi       WebKitPage
                                        │               │
                                        ▼               ▼
                                   photon-core       WKWebView
```

On desktop, the **shell** owns browser windows and coordinates tabs, views,
and Engine sessions. **Core** owns browser rules that work without a window;
the **Engine** implements desktop web page behavior. On iOS, Swift is the
platform host: `WebKitBrowserAdapter` connects the existing Rust page-state API
to a WebKit page. The app currently has a dotted home surface and Liquid Glass
bottom actions, with no browser page UI yet. The
iOS guide documents the source layout and limits of the current FFI.

## What happens when you open a page

1. The shell receives your address or search text.
2. `photon-core` resolves it into a navigation request and updates browser
   state.
3. The shell sends the request through the Photon Engine API.
4. The Engine loads the page and reports navigation, title, and presentation
   updates back to the shell.
5. The shell displays the page in its shared WebView and updates browser
   controls.

The shell handles coordination and presentation; the Engine handles web page
behavior. Shared UI code must not depend on Ladybird implementation details or
receive page pixel buffers.

The iOS navigation path uses `photon-ffi` for address/search resolution and
page-state updates. `WebKitPage` drives `WKWebView` and maps its delegate events
back through the same Rust handle. The current FFI is per page and does not yet
provide tab/session management or profile persistence.

## Main parts

| Part | What it does |
| --- | --- |
| `photon-app` | Starts the desktop application. |
| `photon-shell` | Owns browser windows, shared GPUI views, tab coordination, Engine sessions, and platform adapters. |
| `photon-core` | Owns browser state and rules, including navigation, address/search resolution, and tab-sleep policy. It has no UI or Engine dependency. |
| `photon-storage` | Reads and writes profile data such as history, icons, settings, and website data. |
| `Engine/` | Ladybird and Photon’s embedder API for creating and controlling web views. |
| `native/embedder/` | Bridges the C++ embedder API to the private C ABI used by Rust. |
| `photon-ffi` | Exposes selected browser-model operations through a separate C ABI; it does not start the Engine. |
| `photon-cli` | Provides setup, build, run, and development commands. Cargo CLI installation stays separate from the Linux desktop integration commands that manage XDG launcher files. |
| `ios/` | Native app host with a Rust `photon-ffi` adapter and WebKit page surface; it is separate from the desktop Engine path. |

Other focused packages provide branding, performance diagnostics, sound,
motion, and macOS presentation transport. Performance sampling and data live
in `photon-performance`; its GPUI charts and overlays live in
`photon-performance-ui`. Their package READMEs describe those areas. Generic
GPUI-CE code lives in the pinned `vendor/gpui-ce` dependency.

## How Photon connects to the Engine

Rust code in `photon-shell` calls the public C++ `Photon::Runtime` and
`Photon::View` API in
[`PhotonEmbedder.h`](../Engine/Libraries/LibPhotonEmbedder/include/LibPhotonEmbedder/PhotonEmbedder.h).
[`PhotonEmbedderBridge.cpp`](../native/embedder/PhotonEmbedderBridge.cpp)
adapts that API to a private C interface, which Rust calls through
[`engine_ffi.rs`](../crates/photon-shell/src/platform/engine_ffi.rs).

This boundary keeps engine changes separate from normal browser UI work. Keep
Ladybird classes and engine implementation details inside `Engine/` and the
embedder integration. Keep shared views in
`crates/photon-shell/src/platform/ui/`, shared Engine calls in
`crates/photon-shell/src/platform/engine/`, and operating-system behavior in
`crates/photon-shell/src/platform/<target>/` (`macos/`, `linux/`, `windows/`).
Code shared by several targets has its own folder: `unix/` for macOS and Linux,
and `portable/` for fallbacks used where no native adapter exists (the CPU page
renderer, display metadata and memory pressure). A shared UI component with a
native half keeps it beside the component, as `ui/glass/macos.rs` does.

## Where to make a change

| If you are changing… | Start here |
| --- | --- |
| Address handling, navigation rules, or browser state | [`photon-core`](../crates/photon-core/README.md) |
| History, profile files, or persisted settings | [`photon-storage`](../crates/photon-storage/README.md) |
| Shared browser interface or interactions | [`photon-shell` UI](../crates/photon-shell/src/platform/ui/mod.rs) |
| Engine sessions and callbacks | [`photon-shell` Engine adapter](../crates/photon-shell/src/platform/engine/mod.rs) |
| Native window or operating-system behavior | [`photon-shell` platform adapters](../crates/photon-shell/src/platform/mod.rs) |
| Rust-to-Engine bridge | [`native/embedder`](../native/embedder/PhotonEmbedderBridge.cpp) and [`engine_ffi.rs`](../crates/photon-shell/src/platform/engine_ffi.rs) |
| Theme colors, spacing, or typography | [Theme guide](Theme.md) |
| Generic GPUI-CE behavior | [`vendor/gpui-ce`](../vendor/gpui-ce/README.md) |
| iOS app structure and ownership | [iOS architecture](../ios/README.md) |

## Platform support

macOS is Photon’s primary development platform, with AppKit integration and
IOSurface/Metal page presentation. Linux has a shared GPUI shell and an
interim portable CPU-frame renderer; its shell-only mode can run without the Engine,
while full browser runtime validation is still outstanding. Windows support is
experimental, and native Engine builds are not yet established. The iOS target
under `ios/` has a minimal home surface and a Rust/WebKit backend scaffold, with no
Ladybird or desktop Engine integration. See
[Linux support](Linux.md) for the Linux checklist and
[Building Photon](Building.md) for current build and validation details.

## Related guides

- [Build and run Photon](Building.md)
- [Browser views](WebView.md)
- [Photon pages](Pages.md)
- [Profiles and settings](Profile.md)
- [Theme tokens](Theme.md)
- [Maintain upstream dependencies](Upstream.md)
