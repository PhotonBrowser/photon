# Contributor setup

## First checkout

Install Git, Rust/Cargo (Rust 1.85 or newer), a C++23-capable compiler, CMake 3.30 or newer, Ninja 1.10 or newer, Python 3, and Bun. Install the system development packages required by the pinned Engine for your platform; see [Ladybird's build instructions](Engine/Documentation/BuildInstructionsLadybird.md). Clone Photon with its pinned dependencies and let the CLI finish setup:

```bash
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
./photon doctor
```

`./photon setup` synchronizes the configured HTTPS submodule URLs, initializes the pinned Engine and GPUIX checkouts (including nested submodules), configures the Photon `origin` and upstream remotes from `photon.toml`, fetches Zed's configured `gpuix` upstream branch, and installs the pinned JavaScript dependencies. It does not use Git's `--force` option or advance a pinned submodule. If setup cannot safely switch a submodule because it contains local edits, commit or save that work before retrying.

Run `./photon build` to build Photon Engine and the source-built GPUIX addon. `./photon run` starts the GPUIX React development runtime. For commands and incremental build behavior, see [Building](Documentation/Building.md). For architecture and ownership boundaries, see [Architecture](Documentation/Architecture.md).

## Existing checkout or missing submodules

From the Photon repository root:

```bash
git submodule sync --recursive
./photon setup
git submodule status --recursive
```

The leading status character should be a space for every initialized, correctly pinned submodule. `-` means a submodule is not initialized; `+` means its checkout differs from the commit recorded by Photon. Setup initializes missing submodules but does not discard local changes to fix a `+` checkout. Review and commit or save submodule work first, then restore the commit recorded by the parent repository when appropriate:

```bash
git submodule update --init --recursive Engine vendor/gpuix
```

Do not use `git submodule update --remote` for normal setup. Photon records exact tested commits; upstream updates are merged into the Photon-owned Engine/GPUIX repositories first, then the tested submodule commit is recorded here. Follow [Upstream maintenance](Documentation/Upstream.md) for that process and conflict handling.
