# Building Photon

Use the `photon` CLI for setup, formatting, linting, building, and running Photon. The toolchain requires Git, Rust/Cargo 1.85 or newer, a C++23-capable compiler, CMake 3.30 or newer, Ninja 1.10 or newer, Python 3 with pip, and the platform packages required by the pinned Engine. `photon setup` checks these prerequisites but does not install OS packages; install those using your platform's package manager. See [Ladybird build instructions](../Engine/Documentation/BuildInstructionsLadybird.md) for system packages.

## Common commands

```sh
photon setup
photon format
photon lint
photon check
photon test
photon build
photon run
```

- `photon setup` initializes pinned dependencies, checks system tools, and prepares clang-format 22 for Photon and Photon Engine sources. It uses a matching system formatter when available, otherwise it installs clang-format 22.1.8 under the ignored `build/tools/` directory for this checkout.
- `photon format` formats Rust and Photon-owned native code, including the Engine embedder with its matching style version.
- `photon lint` checks Rust formatting, runs Clippy with warnings treated as errors, and checks Photon and Engine embedder C++ formatting.
- `photon check` runs linting, compiles the Cargo workspace, and verifies architecture and dependency checkout rules.
- `photon test` runs the Rust workspace tests.
- `photon build` builds the Engine and the GPUI-CE application.
- `photon run` builds and launches the native app. Use `--url URL` to choose the initial page. Use `--shutdown-after-seconds N` to exit after a fixed interval; `--verbose` prints the final presentation frame counts.
- `photon run --ui` builds and launches the same browser window with the New Tab page, without building, linking, or starting Photon Engine. Shell-owned pages remain available; web pages cannot be opened in this mode. This path currently supports macOS only.
- `photon clean` removes generated build trees and stops the checkout-specific presentation broker.

`photon check` includes the lint checks, so run it before sharing a change. Build and test commands use release mode by default; `--release` is accepted for compatibility.

## Zed and C++ diagnostics

Zed uses clangd with the root `.clangd` configuration for Photon C++, Objective-C++, and Objective-C files. The configuration supplies the native bridge's include paths and generated Engine headers. Run `photon build` once to generate those headers. After changing `.clangd` or `.zed/settings.json`, run **zed: restart language server** from Zed's command palette.

## Build behavior

The first Engine build configures Ladybird dependencies under `build/` and compiles Photon Engine helper processes plus LibPhotonEmbedder. Incremental builds use an Engine fingerprint to rebuild Ladybird only when Engine sources or configuration change. Rust app or GPUI-CE changes rebuild the application. Build products and downloaded tools stay under ignored build directories.

On macOS, `photon run` starts the Photon-owned presentation broker before opening the native window. Set `PHOTON_WINDOW_BACKGROUND` to `opaque`, `blurred` (the default), or `liquid-glass` before launching to choose the window background. Liquid Glass uses AppKit's regular glass effect on macOS 26 and later and falls back to blurred material on earlier versions.

## Release channels (planned)

Photon should add a separate canary channel for nightly builds when its release
pipeline can produce, sign, and publish installable builds with a reliable
update path. Canary should have its own channel identity and use the yellow
logo variants in `crates/photon-brand/assets/`. Stable remains the default
channel. Local development or debug builds are not canary releases; dev mode
describes a build workflow, while canary identifies a distributed update
channel. A local canary build may be useful for validating channel-specific
behavior, but it should be selected explicitly.

Linux shell work belongs under `crates/photon-shell/src/platform/linux/`, and native integration belongs under `native/embedder/linux/`. Those adapters are not implemented yet.

## Upstream dependencies

Use `photon deps status` to see each dependency's branch, pin, worktree state, and cached upstream counts. `photon deps sync --check` refreshes upstream tips and previews every merge. Add a dependency name (`engine`, `gpui`, or `motion`) to operate on one checkout. A normal `photon deps sync` merges locally without requiring GitHub authentication; pass `--review` to also create the GitHub review issue. Use `photon deps edit` to switch clean checkouts to persistent Photon branches, and `photon deps restore` to return them to the commits recorded by this root checkout. After validating and pushing dependency commits, `photon deps pin` validates and commits updated root gitlinks. See [Upstream maintenance](Upstream.md).
