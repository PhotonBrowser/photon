# photon-performance

Owns Photon performance data and diagnostics: the snapshot model, thread-safe
monitor, timing accumulator, process sampling, and formatting. Keep this crate
independent of UI frameworks. GPUI charts and overlays belong in
`photon-performance-ui`; Engine IPC and native embedder details belong in the
shell/Engine adapter.
