# Linux support

Linux is **experimental**. The shared GPUI shell, Engine adapter and Photon
pages build for Linux, and Ubuntu 24.04 CI runs workspace checks and tests. A
full Engine build and a graphical browser run have not yet been verified.

Linux code lives in `crates/photon-shell/src/platform/linux/`; code shared with
macOS lives in `platform/unix/`, and code shared with Windows in
`platform/portable/`.

## Page presentation

Linux currently uses the **interim CPU-frame path**
(`platform/portable/`): the Engine copies every frame into CPU memory, and GPUI
uploads it to the GPU and draws it. This is correct and keeps the shell free of
pixel-buffer ownership, but it costs a full-frame copy and texture upload per
frame, which grows with resolution and refresh rate. Treat it as a bring-up
path only; do not optimise or build features on top of it.

The intended path is **DMA-BUF import**, matching macOS's IOSurface design:

1. The Engine exports page frames as DMA-BUF file descriptors through an
   explicit `LibPhotonEmbedder` API, alongside the existing macOS IOSurface API.
2. `platform/linux/renderer.rs` provides its own `PagePresenter` and
   `page_surface`, importing those buffers as GPU textures without a copy.
3. Shared `PhotonWebView` code keeps consuming the same `PresentedSurface`
   contract; no pixel buffers reach shared UI state.

## Checklist

### Build and CI

- [x] Shared shell, Engine adapter and pages compile for Linux.
- [x] CI installs the GPUI, font and audio (`libasound2-dev`) packages.
- [x] CI is green again on `main`.
- [ ] `photon setup` installs Linux system packages (currently manual; follow
      [Ladybird's build guide](../Engine/Documentation/BuildInstructionsLadybird.md)).
- [ ] Full `photon build` of the Engine on Ubuntu 24.04 (needs Qt 6.10+).
- [ ] CI job that builds the Engine on Linux.

### Running

- [ ] `photon run --ui` (shell without Engine) verified on X11.
- [ ] `photon run --ui` verified on Wayland.
- [ ] `photon run` loads web pages through the CPU-frame path.
- [ ] Resizing, input, scrolling, IME and clipboard verified with real pages.
- [ ] Multiple windows and tab sleep verified.

### Presentation

- [x] CPU-frame path marked interim.
- [ ] Engine DMA-BUF export through `LibPhotonEmbedder`.
- [ ] DMA-BUF import in `platform/linux/renderer.rs`.
- [ ] CPU-frame path removed from Linux once DMA-BUF is verified.

### Desktop integration (`platform/linux/`)

- [x] Reduced motion follows GNOME's `enable-animations` (`motion_observer.rs`).
- [x] Default browser through `xdg-settings` once a desktop entry is installed
      (`default_browser.rs`).
- [ ] Ship and install the `org.photonbrowser.photon.desktop` entry and icon.
- [ ] Reduced motion on KDE and other desktops (XDG settings portal).
- [ ] Window visibility and display observer (`window_observer.rs`), so hidden
      windows stop rendering.
- [ ] Memory-pressure signal (PSI or cgroup) instead of the portable fallback.
- [ ] Native window decorations and controls verified under GNOME and KDE.
- [ ] Packaging (Flatpak or distribution packages).

### Not planned for Linux

Liquid Glass, native AppKit popovers and the macOS presentation broker are
macOS-only. Linux uses the shared GPUI versions of those surfaces.
