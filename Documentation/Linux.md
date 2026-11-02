# Linux support

Linux is **experimental**. The shared GPUI shell, Engine adapter and Photon
pages build for Linux, and Ubuntu 24.04 CI runs workspace checks and tests. A
full Engine build and a graphical browser run have not yet been verified.

Linux code lives in `crates/photon-shell/src/platform/linux/`; code shared with
macOS lives in `platform/unix/`, and code shared with Windows in
`platform/portable/`.

## Prioritized implementation checklist

Issues are tracked under [Implement Linux support](https://linear.app/photonbrowser/issue/PHTN-86/implement-linux-support). PHTN-229 is in progress; the remaining issues are in Todo. Priorities below determine implementation order; completed checks remain in their sections below.

1. **High** — [PHTN-229: Bridge the system clipboard into Engine pages](https://linear.app/photonbrowser/issue/PHTN-229/bridge-linux-system-clipboard-into-photon-engine-pages).
2. **High** — [PHTN-230: Ship the desktop entry and register web URL handlers](https://linear.app/photonbrowser/issue/PHTN-230/ship-linux-desktop-entry-and-register-web-url-handlers).
3. **High** — [PHTN-231: Build and exercise the Engine and graphical browser in CI](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).
4. **Medium** — [PHTN-232: Observe window visibility and display changes](https://linear.app/photonbrowser/issue/PHTN-232/observe-linux-window-visibility-and-display-changes).
5. **Medium** — [PHTN-234: Propagate reduced-motion preferences to web pages](https://linear.app/photonbrowser/issue/PHTN-234/propagate-linux-reduced-motion-preferences-to-web-pages).
6. **Medium** — [PHTN-233: Report Linux memory pressure to tab sleeping](https://linear.app/photonbrowser/issue/PHTN-233/report-linux-memory-pressure-to-photon-tab-sleeping).
7. **Medium** — [PHTN-235: Use the active display refresh rate for Engine pacing](https://linear.app/photonbrowser/issue/PHTN-235/use-the-active-linux-display-refresh-rate-for-engine-pacing).
8. **Low** — [PHTN-238: Add Linux dependency setup guidance and checks](https://linear.app/photonbrowser/issue/PHTN-238/add-linux-system-dependency-setup-guidance-and-checks).
9. **Low** — [PHTN-237: Align appearance settings with opaque Linux windows](https://linear.app/photonbrowser/issue/PHTN-237/align-linux-appearance-settings-with-opaque-window-support).
10. **Low** — [PHTN-236: Import DMA-BUF page frames without CPU copies](https://linear.app/photonbrowser/issue/PHTN-236/import-linux-dma-buf-page-frames-without-cpu-copies).
11. **Low** — [PHTN-239: Package Photon for supported Linux distributions](https://linear.app/photonbrowser/issue/PHTN-239/package-photon-for-supported-linux-distributions).

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
      [Ladybird's build guide](../Engine/Documentation/BuildInstructionsLadybird.md); [PHTN-238](https://linear.app/photonbrowser/issue/PHTN-238/add-linux-system-dependency-setup-guidance-and-checks)).
- [ ] Full `photon build` of the Engine on Ubuntu 24.04 (needs Qt 6.10+); [PHTN-231](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).
- [ ] CI job that builds the Engine on Linux; [PHTN-231](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).

### Running

- [ ] `photon run --ui` (shell without Engine) verified on X11.
- [ ] `photon run --ui` verified on Wayland.
- [ ] `photon run` loads web pages through the CPU-frame path.
- [ ] Resizing, input, scrolling, IME and clipboard verified with real pages; [PHTN-229](https://linear.app/photonbrowser/issue/PHTN-229/bridge-linux-system-clipboard-into-photon-engine-pages).
- [ ] Multiple windows, window visibility, and tab sleep verified; [PHTN-231](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci), [PHTN-232](https://linear.app/photonbrowser/issue/PHTN-232/observe-linux-window-visibility-and-display-changes).

### Presentation

- [x] CPU-frame path marked interim.
- [ ] Engine DMA-BUF export through `LibPhotonEmbedder`; [PHTN-236](https://linear.app/photonbrowser/issue/PHTN-236/import-linux-dma-buf-page-frames-without-cpu-copies).
- [ ] DMA-BUF import in `platform/linux/renderer.rs`; [PHTN-236](https://linear.app/photonbrowser/issue/PHTN-236/import-linux-dma-buf-page-frames-without-cpu-copies).
- [ ] CPU-frame path removed from Linux once DMA-BUF is verified.

### Desktop integration (`platform/linux/`)

- [x] Reduced motion follows GNOME's `enable-animations` (`motion_observer.rs`).
- [x] Default browser through `xdg-settings` once a desktop entry is installed
      (`default_browser.rs`).
- [ ] Ship and install the `org.photonbrowser.photon.desktop` entry and icon; [PHTN-230](https://linear.app/photonbrowser/issue/PHTN-230/ship-linux-desktop-entry-and-register-web-url-handlers).
- [ ] Reduced motion on KDE and other desktops (XDG settings portal), and pass the preference to Engine pages; [PHTN-234](https://linear.app/photonbrowser/issue/PHTN-234/propagate-linux-reduced-motion-preferences-to-web-pages).
- [ ] Window visibility and display observer (`window_observer.rs`), so hidden windows stop rendering; [PHTN-232](https://linear.app/photonbrowser/issue/PHTN-232/observe-linux-window-visibility-and-display-changes).
- [ ] Memory-pressure signal (PSI or cgroup) instead of the portable fallback; [PHTN-233](https://linear.app/photonbrowser/issue/PHTN-233/report-linux-memory-pressure-to-photon-tab-sleeping).
- [ ] Active display refresh rate instead of a fixed 60 Hz; [PHTN-235](https://linear.app/photonbrowser/issue/PHTN-235/use-the-active-linux-display-refresh-rate-for-engine-pacing).
- [ ] Native window decorations and controls verified under GNOME and KDE; [PHTN-231](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).
- [ ] Appearance controls reflect opaque Linux window support; [PHTN-237](https://linear.app/photonbrowser/issue/PHTN-237/align-linux-appearance-settings-with-opaque-window-support).
- [ ] Linux distribution packaging (Flatpak or distribution packages); [PHTN-239](https://linear.app/photonbrowser/issue/PHTN-239/package-photon-for-supported-linux-distributions).

### Not planned for Linux

Liquid Glass, native AppKit popovers and the macOS presentation broker are
macOS-only. Linux uses the shared GPUI versions of those surfaces.
