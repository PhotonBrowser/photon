# photon-shell

Keep window-bound GPUI views, browser-window behavior, Photon Engine sessions,
and frame presentation lifecycle in this crate. Use `photon-performance` for
performance diagnostics and the reusable overlay, and use
`photon-presentation-ipc` for XPC messages and descriptor transport. Keep
Engine behavior in `Engine/` and generic GPUI-CE behavior in `vendor/gpui-ce/`.

Use `ui/theme.rs` for semantic color roles and get them with
`theme::palette(window, cx)`, which follows the appearance and transparency
settings. Use `ui/metrics.rs` for shared measurements and `Elevation` for
shadows; overlay-specific measurements belong to `photon-performance`. Avoid
color, dimension, shadow, or brand literals in views. Build menus and popovers
from `ui/menu.rs` and controls from `ui/controls.rs`. Animate with
`ui/motion.rs`, and only when something changes, never just because a view
reappears.

Read and change settings through the `Settings` global and the history
through `BrowsingHistory`; observe `Settings` to follow changes made in other
windows. Add the browser's own pages under `ui/pages/` and list them in
`pages/registry.rs` (see `Documentation/Pages.md`).

Keep the Rust exported browser C ABI in `photon-ffi`; `platform/ffi.rs` only
declares the native embedder functions consumed by the shell.
