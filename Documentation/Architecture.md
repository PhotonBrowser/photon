# Architecture

```text
photon-app
    └── photon-shell
        ├── platform/ui/ (shared GPUI browser chrome and WebView)
        └── platform target selection
            ├── macos/ (native controls, Engine session, Metal renderer)
            └── linux/ (opaque window and blank renderer placeholders)

photon-ffi ── photon-core ── photon-omnibox
                    ↑
photon-shell ── photon-storage
            ├── gpui-motion
            └── photon-haptics
photon-core ── photon-tab-sleep
```

The desktop app uses the framework-independent Rust browser model directly.
`photon-ffi` is the separate static library for native callers that need the C
API; it is not part of the desktop shell's internal state path.

## Platform split and Linux landing point

`crates/photon-shell/src/platform/mod.rs` selects target adapters while the
shared GPUI browser chrome and `PhotonWebView` live under
[`platform/ui/`](../crates/photon-shell/src/platform/ui/mod.rs). macOS supplies
native controls and its Metal page renderer in
[`platform/macos/`](../crates/photon-shell/src/platform/macos/mod.rs). Linux's
window adapter and page-renderer landing point are under
[`platform/linux/`](../crates/photon-shell/src/platform/linux/mod.rs). Its page
area is deliberately blank and displays a bottom-right “Linux renderer not
implemented yet” notice until a Linux Engine surface is connected. The Linux
application shell and Engine session are not wired up yet, so the target still
exits with an implementation notice.

Linux window settings request client-side decorations and keep windows opaque.
The shared titlebar reserves a right-side slot for GPUI minimize,
maximize/restore, and close buttons; macOS continues to use its native traffic
lights. The Linux renderer stub is
[`platform/linux/renderer.rs`](../crates/photon-shell/src/platform/linux/renderer.rs);
the active macOS renderer is
[`platform/macos/renderer.rs`](../crates/photon-shell/src/platform/macos/renderer.rs).
Put Linux native embedder sources under
[`native/embedder/linux/`](../native/embedder/linux/README.md), and macOS
Objective-C++ helpers under [`native/embedder/macos/`](../native/embedder/macos/).
The shell build script only builds and links the current Engine bridge on macOS.

Linux will reuse the browser rules and services in `photon-core`,
`photon-omnibox`, `photon-storage`, `photon-brand`, `photon-shortcuts`,
`photon-performance`, and `photon-tab-sleep`, plus the same GPUI views under
`platform/ui`. To complete
the Linux shell, implement its Engine session, system services, window adapter,
and page renderer under `platform/linux`; the shared WebView view should continue
to own page input and browser UI while each adapter provides its native
presentation surface.

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
- `crates/photon-performance` owns performance snapshots, monitor state, bounded
  chart history, current-process sampling, value formatting, chart drawing, and
  the GPUI diagnostics overlay. It has no Engine IPC, native embedder, or
  window-lifecycle dependencies.
- `crates/gpui-motion` provides reusable GPUI entrance and exit effects,
  presence, tweens, spring helpers, and control transitions. It depends on
  GPUI-CE and has no Photon, browser, or native-platform dependencies. The shell
  owns its visual presets and adapts persisted browser preferences to the
  crate's `MotionConfig`.
- `crates/photon-haptics` provides a semantic haptic API for Photon UI. It maps
  taps, control state changes, and alignment feedback to GPUI-CE styles, which
  handles platform support.
- `crates/photon-tab-sleep` owns the framework-independent tab sleep modes and
  adaptive idle-time policy. The shell hides rendering for quiet inactive
  Engine views while retaining their live sessions, so selection restores the
  exact page state without reloading. Audio and recent Engine CPU or network
  activity keep a tab out of the sleeping state. On macOS, the shell samples
  Engine memory per tab at a low rate and asks hidden, safe pages to release
  rebuildable layout and unreachable heap memory when OS pressure rises; page
  sessions remain live.
- `crates/photon-shortcuts` owns one registry of browser shortcuts: each
  entry supplies its action, key sequences, group, and display label. The
  registry builds both the keymap and the grouped shortcuts dialog; native
  menus reuse its labels for shared actions. Browser commands and their
  availability metadata live in `photon-core`'s command registry.
  `BrowserWindow::execute_command` carries them out from shortcuts, menus,
  and controls.
  Views show an action's effective keys with `ui/kbd.rs`, which reads the
  keymap.
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
crates/photon-shell/src/
├── lib.rs                         # Public run entry point
└── platform/
    ├── mod.rs                     # Selects the target adapter and shared contracts
    ├── theme_config.rs            # Loads semantic values from theme.toml
    ├── app_update.rs              # Safe GPUI update retries for async callbacks
    ├── ui/                         # Shared browser chrome and PhotonWebView source
    │   ├── window/                 # Browser layout, tabs, commands and overlays
    │   ├── webview/                # Shared page state, input and browser UI
    │   ├── pages/                  # Photon pages, including new tab and settings
    │   ├── tabs/, sidebar/         # Shared browser controls and layouts
    │   ├── theme.rs                # Resolves shared semantic color roles
    │   └── metrics.rs              # Shared dimensions and type sizes
    ├── macos/                      # Native controls and current Engine backend
    │   ├── engine/                 # Engine runtime and per-tab sessions
    │   ├── presentation.rs         # IOSurface frames, leases and GPU completion
    │   ├── renderer.rs             # External Metal surface element
    │   ├── window_settings.rs      # Traffic lights and macOS window materials
    │   ├── app_menus.rs            # Menu bar and Dock integration
    │   └── ffi.rs                  # macOS native embedder declarations
    └── linux/                      # Linux backend landing point
        ├── renderer.rs             # Blank page with bottom-right implementation notice
        └── window_settings.rs      # Client decorations, opaque window, GPUI controls

native/embedder/
├── PhotonEmbedderBridge.cpp       # Shared LibPhotonEmbedder C++ bridge
├── PhotonEmbedderBridge.h
├── linux/                         # Linux native integration landing point
└── macos/                         # Objective-C++ and AppKit helpers
```

`platform/ui/` is the single GPUI view source. The Linux application is not yet
wired to it because its Engine session backend is still missing; when that
backend is added, Linux should compile these same views and select only its own
window and page renderer adapters.

| Package | Owns | Depends on |
| --- | --- | --- |
| `photon-app` | Runnable Photon entry point | `photon-shell` |
| `photon-shell` | Window-bound GPUI views, Engine session adapter, native presentation lifecycle, browser input | GPUI-CE, `photon-core`, `gpui-motion`, `photon-haptics`, `photon-performance`, `photon-shortcuts`, `photon-storage`, `photon-tab-sleep`, `photon-brand`, `photon-presentation-ipc`, native embedder bridge |
| `photon-performance` | Performance snapshots, monitor state, timing accumulation, formatting, GPUI overlay | GPUI-CE |
| `gpui-motion` | Reusable GPUI motion primitives, transitions, presence, and tweens | GPUI-CE |
| `photon-haptics` | Semantic haptic feedback helpers for Photon controls | GPUI-CE |
| `photon-core` | Browser state, command registry, shared address normalization, history rules, settings, clearing requests | `photon-omnibox`, `photon-tab-sleep`, `photon-brand`, serde |
| `photon-omnibox` | Search engine list, address/query resolution, suggestion and word matching | `photon-brand`, URL parsing library |
| `photon-storage` | Profile folder, history and settings files, saved page icons, data usage | `photon-core`, `photon-brand`, serde |
| `photon-brand` | Browser name, engine name, internal page scheme, profile folder name, logo | none |
| `photon-ffi` | `photon_browser_*` C ABI and static library | `photon-core` |
| `photon-shortcuts` | Browser actions, shortcut registry, and default key bindings | GPUI-CE, `photon-core` |
| `photon-tab-sleep` | Tab sleep modes and adaptive idle timeout policy | serde |
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
| Semantic haptic feedback | [`photon-haptics`](../crates/photon-haptics/src/lib.rs) |
| Shared GPUI window size and layout measurements | [`platform/ui/metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) |
| Native window controls and materials | [`platform/macos/window_settings.rs`](../crates/photon-shell/src/platform/macos/window_settings.rs) and [`platform/linux/window_settings.rs`](../crates/photon-shell/src/platform/linux/window_settings.rs) |
| Shell theme values | [`theme.toml`](../crates/photon-shell/theme.toml) and [`ui/theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs) |
| Menus, popovers, and shared controls | [`ui/menu.rs`](../crates/photon-shell/src/platform/ui/menu.rs) and [`ui/controls.rs`](../crates/photon-shell/src/platform/ui/controls.rs) |
| The browser's name, bundle identifier, page scheme, profile folder, logo, or app icon | [`photon-brand`](../crates/photon-brand/src/lib.rs) |
| Shared layout, spacing, or type sizes | [`ui/metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) |
| Window layout or app startup | [`ui/window/`](../crates/photon-shell/src/platform/ui/window/mod.rs) |
| Dialogs, modals, and buttons | [`ui/js_dialog.rs`](../crates/photon-shell/src/platform/ui/js_dialog.rs), [`ui/modal.rs`](../crates/photon-shell/src/platform/ui/modal.rs), [`ui/button.rs`](../crates/photon-shell/src/platform/ui/button.rs), and [`photon-core/src/dialogs.rs`](../crates/photon-core/src/dialogs.rs) |
| Crash recovery and its notices | [`photon-core/src/crashes.rs`](../crates/photon-core/src/crashes.rs), [`ui/window/alerts.rs`](../crates/photon-shell/src/platform/ui/window/alerts.rs), and [`ui/status_chip.rs`](../crates/photon-shell/src/platform/ui/status_chip.rs) |
| Shared WebView component and input | [`platform/ui/webview/`](../crates/photon-shell/src/platform/ui/webview/mod.rs) |
| Page renderer | [`platform/macos/renderer.rs`](../crates/photon-shell/src/platform/macos/renderer.rs) or [`platform/linux/renderer.rs`](../crates/photon-shell/src/platform/linux/renderer.rs) |
| Keyboard, pointer, or scroll forwarding | [`ui/input.rs`](../crates/photon-shell/src/platform/ui/input.rs) |
| Engine session or callback behavior | [`platform/macos/engine/`](../crates/photon-shell/src/platform/macos/engine/mod.rs) |
| Frame acceptance, presentation order, or release lifetime | [`platform/macos/presentation.rs`](../crates/photon-shell/src/platform/macos/presentation.rs) |
| XPC messages, descriptor transport, or broker connection | [`photon-presentation-ipc`](../crates/photon-presentation-ipc/src/lib.rs) |
| Engine frame pacing or pausing Engine while the window is occluded | [`platform/macos/display.rs`](../crates/photon-shell/src/platform/macos/display.rs), [`platform/macos/window_observer.rs`](../crates/photon-shell/src/platform/macos/window_observer.rs), then [`ui/window/`](../crates/photon-shell/src/platform/ui/window/mod.rs) |
| Rust declarations for native embedder functions | [`platform/macos/ffi.rs`](../crates/photon-shell/src/platform/macos/ffi.rs) and [`PhotonEmbedderBridge.h`](../native/embedder/PhotonEmbedderBridge.h) |
| Browser state and command rules | [`photon-core/src/state.rs`](../crates/photon-core/src/state.rs) |
| Browser settings and profile persistence | [`photon-core/src/settings.rs`](../crates/photon-core/src/settings.rs), [`photon-core/src/workspace.rs`](../crates/photon-core/src/workspace.rs), [`ui/settings.rs`](../crates/photon-shell/src/platform/ui/settings.rs), and [`photon-storage/src/profile.rs`](../crates/photon-storage/src/profile.rs) |
| New tab page, settings page, and their controls | [`ui/pages/`](../crates/photon-shell/src/platform/ui/pages/mod.rs) and the rules in [`photon-core/src/new_tab.rs`](../crates/photon-core/src/new_tab.rs) |
| A new `photon://` page | A module in [`ui/pages/`](../crates/photon-shell/src/platform/ui/pages/mod.rs) with its view and `PageDefinition`, listed in [`pages/registry.rs`](../crates/photon-shell/src/platform/ui/pages/registry.rs) |
| Workspace presets, spaces, sidebar and favourites | [`ui/workspace_layout/`](../crates/photon-shell/src/platform/ui/workspace_layout/mod.rs), [`photon-core/src/workspace.rs`](../crates/photon-core/src/workspace.rs), [`photon-core/src/spaces.rs`](../crates/photon-core/src/spaces.rs), [`ui/tabs/`](../crates/photon-shell/src/platform/ui/tabs/mod.rs), [`ui/toolbar.rs`](../crates/photon-shell/src/platform/ui/toolbar.rs), [`ui/sidebar/`](../crates/photon-shell/src/platform/ui/sidebar/mod.rs), and [`ui/window/chrome.rs`](../crates/photon-shell/src/platform/ui/window/chrome.rs) |
| What a tab shows (web view or Photon page) | [`ui/window/content.rs`](../crates/photon-shell/src/platform/ui/window/content.rs) |
| Peek, the link preview over the page | [`ui/window/peek.rs`](../crates/photon-shell/src/platform/ui/window/peek.rs) |
| C ABI exposed to native callers | [`photon-ffi/include/photon_ffi.h`](../crates/photon-ffi/include/photon_ffi.h) and [`photon-ffi/src/api.rs`](../crates/photon-ffi/src/api.rs) |
| Search engines or address/query classification | [`photon-omnibox/src/`](../crates/photon-omnibox/src/lib.rs) |
| Omnibox suggestions and their panel | [`photon-omnibox/src/suggest.rs`](../crates/photon-omnibox/src/suggest.rs), and [`ui/omnibox/`](../crates/photon-shell/src/platform/ui/omnibox/mod.rs) |
| Browser commands and their availability | [`photon-core/src/commands.rs`](../crates/photon-core/src/commands.rs), [`ui/command_palette.rs`](../crates/photon-shell/src/platform/ui/command_palette.rs), and [`ui/window/commands.rs`](../crates/photon-shell/src/platform/ui/window/commands.rs) |
| History, saved data, and clearing it | [`photon-core/src/history.rs`](../crates/photon-core/src/history.rs), [`photon-storage`](../crates/photon-storage/src/lib.rs), and [`ui/history.rs`](../crates/photon-shell/src/platform/ui/history.rs) |
| `./photon` command behavior | [`photon-cli/src/commands/`](../crates/photon-cli/src/commands/) |
| The development `Photon.app` bundle `./photon run` builds in `~/Library/Application Support/Photon/Development/`: Info.plist, icon, web-link registration | [`photon-cli/src/commands/app_bundle.rs`](../crates/photon-cli/src/commands/app_bundle.rs) |

Keep macOS window policy in `platform/macos/window_settings.rs`, Linux window
policy in `platform/linux/window_settings.rs`, shared measurements in
`platform/ui/metrics.rs`, shared shell color roles in `platform/ui/theme.rs` and
their values in `theme.toml`,
browser rules in core and omnibox, frame lifecycle in the macOS presentation
module, and XPC transport in
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

See [tab layouts and the sidebar](Sidebar.md), [PhotonWebView](WebView.md), [the browser's own pages](Pages.md),
[profile, settings, history and branding](Profile.md), [the omnibox](Omnibox.md),
[native GPU presentation](NativeGpuPresentation.md),
[theme and style tokens](Theme.md), the [customizable workspace architecture](Customization.md),
and [upstream maintenance](Upstream.md).
