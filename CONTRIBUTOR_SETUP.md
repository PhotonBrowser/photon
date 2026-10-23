# Contributor setup

## Install prerequisites

Photon's native desktop app currently targets macOS. Linux and WSL2 can be used for Rust development and checks, but the Photon desktop shell is not implemented there yet. Native Windows builds are not supported.

Install Git, Rust through [rustup](https://rustup.rs/), and Python 3 with pip. Rust 1.85 or newer, `rustfmt`, and `clippy` are selected by the checkout's `rust-toolchain.toml`.

Make sure Cargo's bin directory (`~/.cargo/bin` by default) is on `PATH`.

On macOS, install Xcode Command Line Tools (run `xcode-select --install` if they are missing) and [Homebrew](https://brew.sh). The first setup command installs `autoconf`, `autoconf-archive`, `automake`, `ccache`, CMake, `libtool`, `nasm`, Ninja, `pkg-config`, Python, and Qt through Homebrew.

For Linux or WSL2 system packages, use the instructions for your distribution in [Ladybird's build guide](Engine/Documentation/BuildInstructionsLadybird.md). Also install Python's pip package (`python3-pip` on Debian/Ubuntu). Photon requires CMake 3.30 or newer and Ninja 1.10 or newer; some distributions need newer packages than their default repositories provide.

On macOS, `photon setup --install-system-deps` installs the listed native packages with Homebrew, then verifies tools, initializes pinned submodules, configures remotes and persistent development branches, and prepares the pinned clang-format version under `build/`. It does not install Xcode Command Line Tools, Homebrew, Rust, or Python; install those first. On Linux/WSL2, install system packages from Ladybird's guide, then run `photon setup` without the flag.

## First checkout

On macOS, use `photon setup --install-system-deps` to install build packages with Homebrew. On Linux/WSL2, install the documented packages first, then use `photon setup`.

```bash
git clone https://github.com/PhotonBrowser/photon.git
cd photon
cargo install --path crates/photon-cli --bin photon --locked
photon setup
photon doctor
photon check
photon test
```

After the one-time install, `photon` works from anywhere inside this checkout. Run `photon install` after changing the CLI to refresh the installed command.

The first `photon check` verifies formatting, linting, Rust compilation, and architecture rules. `photon test` runs the workspace tests. Use `photon run` to build and launch the app after those checks pass.

Continue with [Building Photon](Documentation/Building.md) for build, run, and development commands.

## Existing checkout

If `photon` is not installed yet, run `cargo install --path crates/photon-cli --bin photon --locked` from the checkout root.

```bash
photon setup
photon deps status
```

Setup initializes missing submodules, refreshes their configured HTTPS clone URLs, and keeps existing worktrees intact. `photon deps status` reports the branch, root pin, worktree cleanliness, and cached upstream counts for each dependency.

Do not use `git submodule update --remote` for routine setup. Merge and test upstream changes in the Photon-owned Engine or GPUI-CE repository, push the tested dependency commit, then update its parent gitlink. See [Upstream maintenance](Documentation/Upstream.md).
