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
for distribution packaging, and [Experimental package](#experimental-package)
for the experimental archive tooling.

To remove the desktop entry and icon, then remove the installed CLI if desired:

```sh
photon desktop uninstall
cargo uninstall photon-cli
```

If Photon is your current default browser, choose another default in desktop
settings before removing the entry.

## Experimental package

`photon package` turns a completed build into a relocatable archive for
[issue #220](https://github.com/PhotonBrowser/photon/issues/220). The first
target is Ubuntu 24.04 on x86_64 with its glibc. Other distributions, older
glibc releases, musl and ARM are untested.

**Status: NOT VERIFIED at runtime.** Fixture tests cover the staging tool. No
real package has yet rendered a page on a machine without the checkout.

```sh
photon build
photon package                          # build/package/photon-<version>-<commit>-linux-x86_64.tar.xz
photon package --debug --output /tmp/photon-debug
```

`photon package` never builds. It requires `target/<profile>/Photon` and an
Engine build whose recorded fingerprint matches the Engine checkout. The app
also needs the provenance stamp written by `photon build`; packaging rejects a
binary from older Rust or GPUI sources or a binary changed after that stamp.
Run it after `photon build` finishes and never beside a running build. It reads
the Engine build's `CMakeCache.txt` for the build type and the library, helper and
vcpkg folders, and refuses a build whose type differs from the requested
profile. Debug archives end in `-debug`. It refuses to replace an existing
archive, and it deletes only the staging folder it created. It checks input
hashes again after staging to reject files changed while copying.

The archive holds one folder, and `<archive>.sha256` sits next to it.

| Path | Contents |
| --- | --- |
| `bin/` | `Photon`, every helper listed in `Engine/Meta/CMake/ladybird_helper_processes.cmake`, and `photon-desktop` |
| `lib/` | The Engine and vcpkg libraries that `Photon` and the helpers load, and every top-level vcpkg shared library |
| `share/Lagom/` | The Engine resources |
| `README` | Launch, registration, updates, removal and profile instructions |
| `LICENSE`, `share/licenses/` | The Photon, Engine, GPUI-CE and vcpkg port notices |
| `HOST-LIBRARIES` | The libraries the host must provide |
| `VERSION` | The Photon, Engine and GPUI-CE commits, marked `-dirty` for uncommitted changes, and the profile |

Staged executables get the RUNPATH `$ORIGIN/../lib` and staged libraries get
`$ORIGIN`. The build tree is not modified. `Photon` finds its helpers beside
its own executable and opens the address given as its first argument, so the
package has no launcher script.

### Library policy

The package bundles only libraries from the selected Engine build and its
matching vcpkg profile. Libraries from the system library folders stay on the
host and are listed in `HOST-LIBRARIES`. They include glibc and the dynamic
loader, the C++ runtime, graphics drivers and their dispatch libraries. The
exact desktop-library requirements depend on the completed build and are
recorded in `HOST-LIBRARIES`. Build packages on
Ubuntu 24.04 so that their glibc and C++ runtime requirements match the
target.

Packaging fails when any of these holds:

- A library is unresolved, or one name resolves to two different bundled files. When a host library also needs a library supplied by the selected vcpkg profile, the package keeps the bundled copy.
- A library comes from another profile's build or from an unsupported folder such as `/usr/local/lib`.
- A file is not an x86-64 ELF file.
- A file needs a bundled library but has no RUNPATH entry to rewrite.
- A helper or the resources are missing, or `Photon` is a UI-only build.

### Install, update and remove

```sh
sha256sum --check photon-<version>-<commit>-linux-x86_64.tar.xz.sha256
mkdir -p ~/.local/opt
tar -xf photon-<version>-<commit>-linux-x86_64.tar.xz -C ~/.local/opt
~/.local/opt/photon-<version>-<commit>-linux-x86_64/bin/Photon https://example.org/
~/.local/opt/photon-<version>-<commit>-linux-x86_64/bin/photon-desktop install
```

`bin/photon-desktop install` writes
`${XDG_DATA_HOME:-~/.local/share}/applications/org.photonbrowser.photon.desktop`.
The entry starts that folder's `bin/Photon` with the link and uses the icon in
the folder. Folder paths may contain spaces. Registration refuses a path that
contains `"`, `` ` ``, `$`, `\`, `%` or a control character, because the entry
cannot hold those characters unescaped. It never changes the default browser.
Choose Photon in desktop settings or run
`xdg-settings set default-web-browser org.photonbrowser.photon.desktop`. The
package and `photon desktop install` share one desktop ID, so the most recent
registration wins.

To update, quit Photon. Extract the new archive beside the old folder, run the
new folder's `bin/photon-desktop install`, then delete the old folder. There is
no automatic updater. An old folder still runs for a rollback, but a newer
profile may not open in an older version.

To remove the package, choose another default browser if Photon is the
default. Run the folder's `bin/photon-desktop uninstall`, then delete the
folder. Uninstall removes the entry only when it points at that folder, so an
older folder cannot remove a newer registration. Removal keeps the profile in
`${XDG_DATA_HOME:-~/.local/share}/Photon/Default` and the settings in
`${XDG_CONFIG_HOME:-~/.config}/Photon/config.toml`. Delete both to reset
Photon.

### Known limitations

- Opening a link while Photon runs starts a second process on the same profile. There is no single-instance handoff.
- A `.lagom` folder in `$XDG_CONFIG_HOME`, or in `$HOME` when that is unset, replaces the bundled resources.
- vcpkg's OpenSSL records its module and engine folders inside the build tree. The default provider is built in. Whether anything loads from those folders at runtime is unverified.
- Only top-level vcpkg libraries are bundled. Subfolders such as `ossl-modules/` are not.

### Before publishing an archive

1. Render a page from the real archive directly and through its registered URL handler in a runtime-only Ubuntu 24.04 container with no checkout, compiler or build tree:

   ```sh
   docker build -t photon-linux220-runtime -f scripts/linux_runtime_smoke.Dockerfile scripts
   python3 scripts/linux_package_smoke.py build/package/photon-<version>-<commit>-linux-x86_64.tar.xz
   python3 scripts/linux_package_smoke.py build/package/photon-<version>-<commit>-linux-x86_64.tar.xz --desktop --evidence build/package-desktop-smoke
   ```

   The desktop check installs the entry and selects its HTTP/HTTPS handlers only inside a temporary home. It uses GIO to open the URL and checks the same rendered page. Your normal default browser stays unchanged.

2. Exercise the desktop's default-browser settings, an update beside the old folder, removal that keeps the profile, and a WebRTC page, which starts the `WebRTCClient` helper.
3. Collect the unresolved notices. The archive does not yet carry the licenses of the Rust crates linked into `Photon`, including GPUI-CE's dependencies, or of the bundled fonts, such as Noto Emoji under the SIL Open Font License.
