# Linux support

Linux is **experimental**. The shared GPUI shell, Engine adapter and Photon
pages build for Linux. Ubuntu 24.04 CI runs workspace checks and tests, and the
workflow includes a desktop-entry installation smoke check. A full Engine
build and a graphical browser run have not yet been verified.

Linux code lives in `crates/photon-shell/src/platform/linux/`; code shared with
macOS lives in `platform/unix/`, and code shared with Windows in
`platform/portable/`.

## Prioritized implementation checklist

Issues are tracked under [Implement Linux support](https://linear.app/photonbrowser/issue/PHTN-86/implement-linux-support). The PHTN-229 clipboard bridge and PHTN-230 desktop integration implementations are complete. Live graphical verification, including clipboard flows on X11 and Wayland, remains under PHTN-231.

### Completed

- [x] **High** — [PHTN-229: Bridge the system clipboard into Engine pages](https://linear.app/photonbrowser/issue/PHTN-229/bridge-linux-system-clipboard-into-photon-engine-pages).
- [x] **High** — [PHTN-230: Ship the desktop entry and register web URL handlers](https://linear.app/photonbrowser/issue/PHTN-230/ship-linux-desktop-entry-and-register-web-url-handlers).

### Remaining implementation priorities

1. **High** — [PHTN-231: Build and exercise the Engine and graphical browser in CI](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).
2. **Medium** — [PHTN-232: Observe window visibility and display changes](https://linear.app/photonbrowser/issue/PHTN-232/observe-linux-window-visibility-and-display-changes).
3. **Medium** — [PHTN-234: Propagate reduced-motion preferences to web pages](https://linear.app/photonbrowser/issue/PHTN-234/propagate-linux-reduced-motion-preferences-to-web-pages).
4. **Medium** — [PHTN-233: Report Linux memory pressure to tab sleeping](https://linear.app/photonbrowser/issue/PHTN-233/report-linux-memory-pressure-to-photon-tab-sleeping).
5. **Medium** — [PHTN-235: Use the active display refresh rate for Engine pacing](https://linear.app/photonbrowser/issue/PHTN-235/use-the-active-linux-display-refresh-rate-for-engine-pacing).
6. **Low** — [PHTN-238: Add Linux dependency setup guidance and checks](https://linear.app/photonbrowser/issue/PHTN-238/add-linux-system-dependency-setup-guidance-and-checks).
7. **Low** — [PHTN-237: Align appearance settings with opaque Linux windows](https://linear.app/photonbrowser/issue/PHTN-237/align-linux-appearance-settings-with-opaque-window-support).
8. **Low** — [PHTN-236: Import DMA-BUF page frames without CPU copies](https://linear.app/photonbrowser/issue/PHTN-236/import-linux-dma-buf-page-frames-without-cpu-copies).
9. **Low** — [PHTN-239: Package Photon for supported Linux distributions](https://linear.app/photonbrowser/issue/PHTN-239/package-photon-for-supported-linux-distributions).

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
- [ ] Resizing, input, scrolling, IME and clipboard verified with real pages; [PHTN-231](https://linear.app/photonbrowser/issue/PHTN-231/build-and-exercise-the-linux-engine-and-graphical-browser-in-ci).
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
- [x] Install the `org.photonbrowser.photon.desktop` entry and icon, and declare HTTP/HTTPS handlers; [PHTN-230](https://linear.app/photonbrowser/issue/PHTN-230/ship-linux-desktop-entry-and-register-web-url-handlers).
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

## Install and remove Linux desktop integration

On Linux, `./photon install` installs or refreshes only the `photon` and `ph`
CLI binaries in Cargo's bin directory. Full `./photon setup` then installs the
desktop entry and icon in the user's XDG data directory. The entry declares
HTTP, HTTPS, HTML and XHTML support and runs `photon run URL` for links.
`./photon setup --ui-only` does not install or update desktop integration
because it does not prepare Engine web-page loading. If an entry was installed
by an earlier full setup, UI-only setup leaves it in place.

No separate command is needed after full setup. `photon desktop install` is
available for manual refresh and requires the CLI to be installed first.
Choosing Photon as the default browser remains a user decision in desktop
settings.

The desktop launcher calls the CLI under Cargo's install root. Opening Photon
from the launcher builds the browser from this checkout before starting it. If
you move the checkout or change Cargo's install root, run `./photon install`
followed by `photon desktop install` to refresh both paths. This is developer
integration, not a packaged end-user build; see
[PHTN-239](https://linear.app/photonbrowser/issue/PHTN-239/package-photon-for-supported-linux-distributions)
for distribution packaging.

To remove the desktop entry and icon, then remove the installed CLI if desired:

```sh
photon desktop uninstall
cargo uninstall photon-cli
```

If Photon is your current default browser, choose another default in desktop
settings before removing the entry.
