# photon-shell

Keep GPUI views, browser-window behavior, Photon Engine sessions, and frame
presentation lifecycle in this crate. Use `photon-presentation-ipc` for XPC
messages and descriptor transport. Keep Engine behavior in `Engine/` and
generic GPUI-CE behavior in `vendor/gpui-ce/`.

Use `ui/theme.rs` for semantic color roles and appearance mapping. Use
`ui/metrics.rs` for shared dimensions, spacing, corner radii, and type sizes.
Avoid color or dimension literals in individual views. Keep the Rust exported
browser C ABI in `photon-ffi`; `platform/ffi.rs` only declares the native
embedder functions consumed by the shell.
