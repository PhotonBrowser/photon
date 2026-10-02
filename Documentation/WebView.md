# PhotonWebView

`PhotonWebView` is the shell's only page element. It owns one Photon Engine view and its native presentation session. Startup text is resolved by the Rust [omnibox rules](Omnibox.md); no JavaScript or N-API UI runtime participates.

The element converts its GPUI-CE layout bounds and scale factor to the Engine viewport, resizing only when the physical dimensions or display scale change. Engine callbacks replace a single latest-frame slot so stale frames do not queue up. GPUI-CE keyboard, mouse-button, pointer-move, and wheel events are forwarded through the narrow embedder API; interactive behavior still needs hands-on validation.

On macOS the Engine publishes IOSurface-backed BGRA frames with producer `MTLSharedEvent` values. Photon imports and caches each backing, draws it through GPUI-CE's generic external Metal surface, and releases old Engine leases only after GPU completion. During shutdown Photon disables new publications, waits for in-flight sampling command buffers, drains all leases, and reports/asserts zero outstanding frames.

The minimal shell has no tabs, address bar, toolbar, menus, or browser chrome. Window styling and the 4 logical pixel WebView inset are separate follow-up work.
