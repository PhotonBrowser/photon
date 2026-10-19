# photon-shell

Keep window-bound GPUI views, browser-window behavior, Photon Engine sessions,
and frame presentation lifecycle in this crate. Use `photon-performance` for
performance diagnostics and the reusable overlay, and use
`photon-presentation-ipc` for XPC messages and descriptor transport. Keep
Engine behavior in `Engine/` and generic GPUI-CE behavior in `vendor/gpui-ce/`.

Keep shared GPUI views under `src/platform/ui/` so macOS and Linux use the same
browser chrome and WebView component. Keep native window controls, system
services, Engine adapters, and page renderers under `src/platform/<target>/`. The
macOS adapter lives in `src/platform/macos/`; Linux integration belongs in
`src/platform/linux/` and native Linux glue in `native/embedder/linux/`.

Use `theme.toml` for shared semantic color values, opacity tokens, and corner
radii. `platform/ui/theme.rs` resolves semantic color roles; get them with
`theme::palette(window, cx)`, which follows appearance and transparency settings.
Use `platform/ui/metrics.rs` for shared measurements, `Elevation` for
shadows, and `raised` for the background of anything floating over the page;
overlay-specific measurements belong to `photon-performance`. Avoid color,
dimension, shadow, blur, or brand literals in views. Build menus and popovers
from `platform/ui/menu.rs` and controls from `platform/ui/controls.rs`. Animate
with `platform/ui/motion.rs`, and only when something changes, never just because
a view reappears.

Read and change settings through the `Settings` global and the history
through `BrowsingHistory`; observe `Settings` to follow changes made in other
windows. Add the browser's own pages under `platform/ui/pages/` and list them in
`platform/ui/pages/registry.rs` (see `Documentation/Pages.md`).

Keep the Rust exported browser C ABI in `photon-ffi`; `platform/macos/ffi.rs` only
declares the native embedder functions consumed by the shell.

Use `photon-haptics` for semantic haptic feedback and keep GPUI's platform
haptic API behind that crate.
