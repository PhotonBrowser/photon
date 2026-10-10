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
                    ↑
photon-shell ── photon-storage
```

The desktop app uses the framework-independent Rust browser model directly.
`photon-ffi` is the separate static library for native callers that need the C
API; it is not part of the desktop shell's internal state path.

## Ownership

- `crates/photon-core` contains framework-independent browser state, commands,
  address normalization, browser settings, history, and the new tab page's
  rules. It has no GPUI, GPUI-CE, native, or Ladybird types.
- `crates/photon-omnibox` owns address-versus-search classification, search
  engine data, and how typed text is matched against visited pages and past
  searches for suggestions. Core and the shell use the same rules.
- `crates/photon-brand` owns the browser's name, engine name, internal page
  scheme, profile folder name and logo. Anything people see that names the
  browser comes from it, so renaming is one edit there.
- `crates/photon-storage` owns the on-disk profile: the history and settings
  files, saved page icons, data usage, and the Engine's website data folder.
  It is plain file access over core types, with no GPUI or Engine dependency,
  so settings and internal pages can list and delete data through the same
  calls. Core owns the history rules and `ClearBrowsingData`; the shell applies
  a clear to both the profile and the Engine.
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
│           │   ├── tab_menu.rs       # Tab menu, reordering and bulk tab actions
│           │   ├── menu.rs           # Browser menu and theme choice
│           │   ├── find.rs           # Opening and closing the find bar
│           │   ├── actions.rs        # Keyboard shortcuts
│           │   └── alerts.rs         # Crash and restart notices, JavaScript dialogs
│           ├── webview/              # Page surface composition and tab state
│           │   ├── favicon.rs        # The page icon
│           │   ├── crashes.rs        # Crash recovery notices
│           │   ├── dialogs.rs        # JavaScript dialog requests and replies
│           │   └── find.rs           # Find-in-page state
│           ├── pages/                # Photon's own pages at photon:// addresses, drawn natively in a tab
│           │   ├── registry.rs       # Every page, found by its photon:// name
│           │   ├── new_tab/          # Logo, shortcut tiles, customise panel, adding a shortcut
│           │   ├── settings/         # Sidebar and one module per section
│           │   ├── layout.rs         # Page columns, headings and groups
│           │   └── controls.rs       # Choices, switches and checkboxes
│           ├── omnibox/              # Address field and its suggestion panel
│           ├── history.rs            # Shared browsing history, saved to the profile
│           ├── settings.rs           # Shared settings, saved to the profile
│           ├── js_dialog.rs          # JavaScript alert, confirm and prompt
│           ├── modal.rs              # Reusable centered modal
│           ├── motion.rs             # Entrance animations and presets
│           ├── button.rs             # Shared text buttons
│           ├── status_chip.rs        # Bottom-right crash and restart chips
│           ├── find_bar.rs           # Find-in-page bar
│           ├── input.rs              # Keyboard, pointer, and scroll forwarding
│           ├── theme.rs              # Appearance and semantic color tokens
│           └── metrics.rs            # Shared UI dimensions and typography
├── photon-performance/               # Performance diagnostics model and overlay
├── photon-core/                      # Framework-independent browser model
├── photon-omnibox/                   # Search engines, address resolution, suggestions
├── photon-storage/                   # On-disk profile: history, icons, website data
├── photon-brand/                     # Browser name, page scheme, profile folder and logo
├── photon-ffi/                       # Exported C API and static library
├── photon-shortcuts/                 # Browser actions and key bindings
├── photon-cli/                       # `./photon` developer commands
├── photon-presentation-ipc/          # XPC protocol, client, and native transport
└── photon-presentation-broker/       # macOS XPC service executable
```

| Package | Owns | Depends on |
| --- | --- | --- |
| `photon-app` | Runnable Photon entry point | `photon-shell` |
| `photon-shell` | Window-bound GPUI views, Engine session adapter, native presentation lifecycle, browser input | GPUI-CE, `photon-core`, `photon-performance`, `photon-shortcuts`, `photon-storage`, `photon-brand`, `photon-presentation-ipc`, native embedder bridge |
| `photon-performance` | Performance snapshots, monitor state, timing accumulation, formatting, GPUI overlay | GPUI-CE |
| `photon-core` | Browser state, commands, shared address normalization, history rules, settings, clearing requests | `photon-omnibox`, `photon-brand`, serde |
| `photon-omnibox` | Search engine list, address/query resolution, suggestion matching | `photon-brand`, URL parsing library |
| `photon-storage` | Profile folder, history and settings files, saved page icons, data usage | `photon-core`, `photon-brand`, serde |
| `photon-brand` | Browser name, engine name, internal page scheme, profile folder name, logo | none |
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
| Page surface composition | [`ui/webview/`](../crates/photon-shell/src/platform/ui/webview/mod.rs) |
| Keyboard, pointer, or scroll forwarding | [`ui/input.rs`](../crates/photon-shell/src/platform/ui/input.rs) |
| Engine session or callback behavior | [`platform/engine/`](../crates/photon-shell/src/platform/engine/mod.rs) |
| Frame acceptance, presentation order, or release lifetime | [`platform/presentation.rs`](../crates/photon-shell/src/platform/presentation.rs) |
| XPC messages, descriptor transport, or broker connection | [`photon-presentation-ipc`](../crates/photon-presentation-ipc/src/lib.rs) |
| Engine frame pacing or pausing Engine while the window is occluded | [`platform/display.rs`](../crates/photon-shell/src/platform/display.rs), [`platform/window_observer.rs`](../crates/photon-shell/src/platform/window_observer.rs), then [`ui/window/`](../crates/photon-shell/src/platform/ui/window/mod.rs) |
| Rust declarations for native embedder functions | [`platform/ffi.rs`](../crates/photon-shell/src/platform/ffi.rs) and [`PhotonEmbedderBridge.h`](../native/embedder/PhotonEmbedderBridge.h) |
| Browser state and command rules | [`photon-core/src/state.rs`](../crates/photon-core/src/state.rs) |
| Browser settings and profile persistence | [`photon-core/src/settings.rs`](../crates/photon-core/src/settings.rs), [`ui/settings.rs`](../crates/photon-shell/src/platform/ui/settings.rs), and [`photon-storage/src/profile.rs`](../crates/photon-storage/src/profile.rs) |
| New tab page, settings page, and their controls | [`ui/pages/`](../crates/photon-shell/src/platform/ui/pages/mod.rs) and the rules in [`photon-core/src/new_tab.rs`](../crates/photon-core/src/new_tab.rs) |
| A new `photon://` page | A module in [`ui/pages/`](../crates/photon-shell/src/platform/ui/pages/mod.rs) with its view and `PageDefinition`, listed in [`pages/registry.rs`](../crates/photon-shell/src/platform/ui/pages/registry.rs) |
| What a tab shows (web view or Photon page) | [`ui/window/content.rs`](../crates/photon-shell/src/platform/ui/window/content.rs) |
| C ABI exposed to native callers | [`photon-ffi/include/photon_ffi.h`](../crates/photon-ffi/include/photon_ffi.h) and [`photon-ffi/src/api.rs`](../crates/photon-ffi/src/api.rs) |
| Search engines or address/query classification | [`photon-omnibox/src/`](../crates/photon-omnibox/src/lib.rs) |
| Omnibox suggestions and their panel | [`photon-omnibox/src/suggest.rs`](../crates/photon-omnibox/src/suggest.rs), and [`ui/omnibox/`](../crates/photon-shell/src/platform/ui/omnibox/mod.rs) |
| History, saved data, and clearing it | [`photon-core/src/history.rs`](../crates/photon-core/src/history.rs), [`photon-storage`](../crates/photon-storage/src/lib.rs), and [`ui/history.rs`](../crates/photon-shell/src/platform/ui/history.rs) |
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
