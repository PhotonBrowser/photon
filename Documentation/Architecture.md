# Photon architecture

Photon is a desktop browser built from a Rust application and the Ladybird
browser Engine. GPUI-CE draws Photon’s interface; the Engine loads and renders
web pages. Photon connects the two through a small, explicit embedder API.

This guide is for people who want to understand how Photon fits together or
find the right place to make a change. For installation and build steps, see
[Building Photon](Building.md).

## The short version

```text
You use Photon
     │
     ▼
Photon shell ───── Photon Engine API ───── Ladybird Engine
  window and UI       page lifecycle          web platform
     │
     ├── photon-core       browser rules and state
     ├── photon-storage    profile data on disk
     └── platform adapters macOS, Linux, and Windows
```

The **shell** owns the browser window and coordinates tabs, views, and Engine
sessions. **Core** owns browser rules that should work without a window. The
**Engine** implements web standards and page behavior. Platform adapters
connect the shared shell to operating-system features.

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
| `photon-cli` | Provides setup, build, run, and development commands. |

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
`crates/photon-shell/src/platform/<target>/`.

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

## Platform support

macOS is Photon’s primary development platform, with AppKit integration and
IOSurface/Metal page presentation. Linux has a shared GPUI shell and a
portable CPU-frame renderer; its shell-only mode can run without the Engine,
while full browser runtime validation is still outstanding. Windows support is
experimental, and native Engine builds are not yet established. See
[Building Photon](Building.md) for current build and validation details.

## Related guides

- [Build and run Photon](Building.md)
- [Browser views](WebView.md)
- [Photon pages](Pages.md)
- [Profiles and settings](Profile.md)
- [Theme tokens](Theme.md)
- [Maintain upstream dependencies](Upstream.md)
