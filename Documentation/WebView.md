# PhotonWebView

`PhotonWebView` is the shell's web page view. A browser tab shows either one
web view, with its own Photon Engine session, or one of
[the browser's own pages](Pages.md). The shell window provides the
[tab strip and toolbar, or sidebar](Sidebar.md) (navigation, omnibox, tabs
and the browser menu) and an optional performance overlay around the active tab's view.

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
`MTLSharedEvent` values. Once a frame's producer work is done, Photon shows it
in the view's own Core Animation layer beneath GPUI-CE, which leaves a hole
where the page is, so browser UI drawn over the page stays on top. See
[native GPU presentation](NativeGpuPresentation.md). During shutdown Photon
disables new publications, drains all leases, and reports zero outstanding
frames.

Linux and Windows currently use the portable CPU-frame path: the Engine hands
frames to the shell, the shared presenter wraps them as GPUI images, and GPUI
uploads and draws them. The platform `renderer` adapter selects this presenter;
the shared `PhotonWebView` consumes the same `PresentedSurface` contract on
every target. Future Linux DMA-BUF import belongs behind
`platform/linux/renderer.rs` and the explicit embedder API, without adding
surface ownership or pixel buffers to shared UI state. A UI-only build starts
the shell without creating Engine sessions or frames.

A two-finger swipe sideways goes back or forward through the tab's history.
The Engine's compositor scrolls the page first, and only a sideways scroll
left over at the viewport's edge, where the page's `overscroll-behavior`
allows it, becomes a swipe: the view then slides the page with the fingers,
shows an arrow in the uncovered edge, and on release either navigates or
springs back. Mouse back and forward buttons navigate too.

Shift-clicking a link opens it in a Peek: the Engine reports the click
instead of following it, and the window shows the link in a new web view over
the page. Keeping it as a tab moves that same web view into the tab list, so
the page does not reload.

Each web view records its visits, titles and icons in the shared
[history](Profile.md#history), reporting only what changed. It follows the
appearance chosen in settings, telling pages whether to use their dark
color scheme.

Shell color roles and measurements live in the shared
[theme tokens](Theme.md). External IOSurface presentation currently works on
macOS.
