# PhotonWebView

`PhotonWebView` is the shell's page view. Each browser tab owns one view and one
Photon Engine session. The shell window provides a tab strip, address toolbar,
omnibox, browser menu, and an optional performance overlay around the active
page view.

Omnibox submissions and startup addresses use the same Rust
[address and search rules](Omnibox.md). Browser state stays in Rust; there is no
JavaScript UI runtime.

The view converts its GPUI-CE layout bounds and scale factor to the Engine
viewport, resizing only when physical dimensions or display scale change.
Engine callbacks replace a single latest-frame slot so stale frames do not
queue up. Keyboard, mouse, pointer, and wheel events are forwarded through the
narrow native embedder API. The active tab receives Engine visibility and focus
updates; inactive tabs retain their page state without rendering frames.

On macOS the Engine publishes IOSurface-backed BGRA frames with producer
`MTLSharedEvent` values. Photon imports and caches each backing, draws it through
GPUI-CE's generic external Metal surface, and releases old Engine leases only
after GPU completion. During shutdown Photon disables new publications, waits
for in-flight sampling command buffers, drains all leases, and reports/asserts
zero outstanding frames.

Shell color roles and measurements live in the shared
[theme tokens](Theme.md). The app currently targets macOS for external
IOSurface presentation.
