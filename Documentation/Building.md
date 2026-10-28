# Building Photon

Use the `photon` CLI for setup, formatting, linting, building, and running Photon. The repository pins Rust/Cargo 1.99.0, a C++23-capable compiler, CMake 3.30+, Ninja 1.10+, Python 3 with pip, and the native packages required by the pinned Engine. macOS is the primary development platform. Linux is experimental; Ubuntu 24.04 CI validates workspace checks, CLI smoke commands, and tests in the [latest passing run](https://github.com/PhotonBrowser/photon/actions/runs/40202068735). A complete Engine build and graphical browser run are not CI-verified. See [contributor setup](../CONTRIBUTOR_SETUP.md) for platform prerequisites and first checkout.

## Platform status

| Command | macOS | Linux |
| --- | --- | --- |
| `photon setup` | Implemented but unverified | Full setup is implemented but unverified; Linux system packages are manual |
| `photon setup --ui-only` | Implemented; Engine setup is skipped | Initializes GPUI and prepares the shell without Engine or Ladybird build prerequisites |
| `photon check` | Verified working in the current macOS checkout | Verified working by Ubuntu 24.04 CI |
| `photon test` | Verified working in the current macOS checkout | Verified working by Ubuntu 24.04 CI |
| `photon build` | Implemented but unverified; Engine build was not run in this audit | Partially implemented; Linux Engine build path exists, but full Engine build is not verified here or by CI |
| `photon run` | Implemented but unverified; native run path was not launched in this audit | Partially implemented; portable run path exists, but end-to-end graphical runtime is not CI-verified |

The CLI's macOS hot-reload path is macOS-only. Linux `photon run` uses the direct Cargo path; `photon run --ui` builds the shell without Engine and cannot open web pages.

## Common commands

```sh
photon setup
photon doctor
photon format
photon lint
photon check
photon test
photon build
photon run
```

- `photon setup` checks required tools, initializes missing pinned dependencies, prepares Rust formatting/lint tools and clang-format 22, and installs or refreshes the `photon` command in Cargo's bin directory. On macOS, add `--install-system-deps` to install the required Homebrew packages. On Linux, install packages from Ladybird's guide first. Setup preserves initialized dependency branches and does not advance root pins. The pinned clang-format 22.1.8 stays under the ignored `build/tools/` directory.
- `photon setup --ui-only` initializes the pinned GPUI-CE checkout and installs the CLI while skipping the Engine submodule, C++ compiler, Python, CMake, Ninja, and clang-format setup. It supports `photon run --ui`; it does not prepare `photon build`, full `photon check`, or web-page loading. The mode preserves any existing Engine checkout.
- `photon doctor` reports tool versions, Rust components, dependency revisions, and build directories. `photon setup` is the strict prerequisite check.
- `photon format` formats Rust and Photon-owned native code, including the Engine embedder with its matching style version.
- `photon lint` checks Rust formatting, runs Clippy with warnings treated as errors, and checks Photon and Engine embedder C++ formatting.
- `photon check` runs linting, compiles the Cargo workspace, and verifies architecture and dependency checkout rules.
- `photon test` runs the Rust workspace tests.
- `photon build` builds the Engine and the GPUI-CE application. It requires initialized Engine and GPUI-CE submodules and a full set of Ladybird build prerequisites; it can take a long time on the first run.
- `photon run` builds and launches the native app. Direct app builds use Cargo. On macOS, `photon run` uses Dioxus hot reloading by default; pass `--no-hot-reload` for a direct build. Hot reloading reuses the existing Engine build without building Engine; run `photon build` first if that Engine build is missing or out of date. It currently supports macOS only. Use `--url URL` to choose the initial page. `--shutdown-after-seconds N` works on the direct path; `--verbose` prints output from the underlying build tools.
- Hot reload launches the debug shell through Dioxus CLI and patches GPUI rendering code without restarting the app. Photon installs Dioxus CLI 0.7.10 with Cargo on first use if it is missing or has an incompatible version. Use `--hot-reload` to enable it explicitly, or `--ui` to hot-patch the UI-only shell without requiring Photon Engine.
- `photon run --ui` builds and launches the shared browser window with the New Tab page, without building, linking, or starting Photon Engine. Shell-owned pages remain available; web pages cannot be opened in this mode. The path is available on macOS, Linux, and Windows.
- Browser context menus and page `<select>` controls use the shared GPUI menu surface and work without platform-native menu hooks. They inherit the semantic theme roles, menu metrics, and motion used by Photon popovers.
- `photon clean` removes generated build trees and stops the checkout-specific presentation broker.

## Troubleshooting

- **Missing Engine or GPUI-CE:** From the checkout root, run `git submodule update --init --recursive`, then `./photon setup`. Setup initializes missing submodules without checking out over initialized worktrees.
- **Setup reports a missing prerequisite:** Install the named tool or library. On Linux, follow the package section for your distribution in [Ladybird's build guide](../Engine/Documentation/BuildInstructionsLadybird.md); check the documented Qt, compiler, CMake, and Ninja minimum versions.
- **Linux CMake cannot find Qt:** Ladybird requires Qt 6.10 or newer. Older distribution packages need a newer Qt installation and an appropriate `CMAKE_PREFIX_PATH`.
- **`photon doctor` reports missing values:** Doctor is informational and does not fail merely because a tool or submodule is missing. Use its output to locate the problem, then rerun setup after correcting it.
- **Setup changed a remote URL:** Setup intentionally enforces HTTPS remotes from `photon.toml`; adjust that configuration for a persistent SSH preference. It does not move a dependency commit or advance a root gitlink.

`photon check` includes the lint checks, so run it before sharing a change. Build and test commands use release mode by default; `--release` is accepted for compatibility.

## Zed and C++ diagnostics

Zed uses clangd with the root `.clangd` configuration for Photon C++, Objective-C++, and Objective-C files. The configuration supplies the native bridge's include paths and generated Engine headers. Run `photon build` once to generate those headers. After changing `.clangd` or `.zed/settings.json`, run **zed: restart language server** from Zed's command palette.

## Build behavior

The first Engine build configures Ladybird dependencies under `build/` and compiles Photon Engine helper processes plus LibPhotonEmbedder. Incremental builds use an Engine fingerprint to rebuild Ladybird only when Engine sources or configuration change. Rust app or GPUI-CE changes rebuild the application. Build products and downloaded tools stay under ignored build directories.

On macOS, `photon run` starts the Photon-owned presentation broker before opening the native window. Set `PHOTON_WINDOW_BACKGROUND` to `opaque`, `blurred` (the default), or `liquid-glass` before launching to choose the window background. Liquid Glass uses AppKit's regular glass effect on macOS 26 and later and falls back to blurred material on earlier versions. Linux uses the shared GPUI browser shell and CPU-backed Engine frame path; its system integration and full runtime validation remain incomplete. Windows remains experimental: Ladybird supports WSL2 as its build route, while native Clang-CL builds are experimental.

Hot reloading uses GPUI-CE's experimental Subsecond integration. Changes to GPUI render functions and direct workspace dependencies can be patched into the running process. Changes to indirect workspace dependencies, application startup, and unsupported code paths require a rebuild or restart. `--shutdown-after-seconds` is unavailable in hot-reload mode because Dioxus CLI keeps the development server running.

Platform-specific behavior belongs under `crates/photon-shell/src/platform/<target>/`. Keep the GPUI views in `platform/ui/`, the shared Engine adapter in `platform/engine/`, and the Apple-only XPC/IOSurface presentation path in `platform/macos/`. Linux and Windows currently use the portable CPU frame adapter; future DMA-BUF or DXGI imports belong in their target renderers without changing shared WebView code.

## Upstream dependencies

Use `photon deps status` to see each submodule's branch, pin, worktree state, and cached upstream counts. `photon deps sync --check` fetches upstream tips and previews both merges. Add `engine` or `gpui` to operate on one checkout. A normal `photon deps sync` merges locally without requiring GitHub authentication; pass `--review` to also create the GitHub review issue. Use `photon deps edit` to switch clean checkouts to persistent Photon branches, and `photon deps restore` to return them to the commits recorded by this root checkout. After validating and pushing dependency commits, `photon deps pin` validates and commits updated root gitlinks. See [Upstream maintenance](Upstream.md).
