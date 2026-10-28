# photon-shell

Keep window-bound GPUI views, browser-window behavior, Photon Engine sessions,
and frame presentation lifecycle in this crate. Use `photon-performance` for
performance diagnostics and `photon-performance-ui` for GPUI charts and
overlays, and use
`photon-presentation-ipc` for XPC messages and descriptor transport. Keep
Engine behavior in `Engine/` and generic GPUI-CE behavior in `vendor/gpui-ce/`.

Keep shared GPUI views under `src/platform/ui/` so macOS, Linux, and Windows use
the same browser chrome and WebView component. Keep native window controls,
system services, and renderer selection under `src/platform/<target>/`. The
shared Engine runtime/session adapter lives in `src/platform/engine/`; the
private Rust bridge declarations live in `src/platform/engine_ffi.rs`. Keep
Apple-only IOSurface, XPC, Metal, and AppKit code in `src/platform/macos/`.
Linux and Windows native integration belongs in their respective adapter
directories; do not add OS-specific behavior throughout `platform/ui/`.

Use `theme.toml` for shared semantic color values, opacity tokens, and corner
radii. `platform/ui/theme.rs` resolves semantic color roles; get them with
`theme::palette(window, cx)`, which follows appearance and transparency settings.
Use `platform/ui/metrics.rs` for shared measurements, `Elevation` for
shadows, and `raised` for the background of anything floating over the page;
overlay-specific measurements belong to `photon-performance`. Avoid color,
dimension, shadow, blur, or brand literals in views. Build menus and popovers
from `platform/ui/menu.rs` and controls from `platform/ui/controls.rs`. Animate
with `platform/ui/motion/`, and only when something changes, never just because
a view reappears.

Read and change settings through the `Settings` global and the history
through `BrowsingHistory`; observe `Settings` to follow changes made in other
windows. Add the browser's own pages under `platform/ui/pages/` and list them in
`platform/ui/pages/registry.rs` (see `Documentation/Pages.md`).

Keep the public Rust browser-model C ABI in `photon-ffi`. The shell's Rust
declarations for its private Engine bridge live in `platform/engine_ffi.rs`; the
Engine's public C++ embedder contract remains `PhotonEmbedder.h`.

Keep semantic haptic feedback behind `src/platform/haptics.rs` and browser
keyboard bindings behind `src/platform/shortcuts/`.
