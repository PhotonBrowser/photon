# Contributor setup

## First checkout

Install Git, Rust/Cargo (Rust 1.85 or newer), a C++23-capable compiler, CMake 3.30 or newer, Ninja 1.10 or newer, Python 3, and the system development packages required by the pinned Engine. See [Ladybird build instructions](Engine/Documentation/BuildInstructionsLadybird.md).

```bash
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
./photon doctor
./photon build
./photon run
```

`./photon setup` initializes the pinned Engine and GPUI-CE submodules, configures their Photon `origin` and upstream remotes from `photon.toml`, then switches both to their persistent development branches. Setup preserves initialized worktrees and does not advance dependency pins.

## Existing checkout

```bash
git submodule sync --recursive
./photon setup
git submodule status --recursive
```

The leading status character should be a space for every initialized, correctly pinned submodule. `-` means a submodule is not initialized; `+` means its checkout differs from the recorded commit. Setup initializes missing submodules but keeps existing worktrees intact.

Do not use `git submodule update --remote` for routine setup. Merge and test upstream changes in the Photon-owned Engine or GPUI-CE repository, push the tested dependency commit, then update its parent gitlink. See [Upstream maintenance](Documentation/Upstream.md).
