# Contributor setup

## Install prerequisites

The shared GPUI shell targets macOS, Linux, and Windows. Linux builds Ladybird
using the distribution packages in its build guide. Ladybird documents WSL2 as
the supported Windows build path; its native Windows/Clang-CL build is still
experimental, so the Windows shell target is wired but a full native Windows
Engine build needs that upstream toolchain to work.

Install Git, Rust through [rustup](https://rustup.rs/), and Python 3 with pip. Rust 1.85 or newer, `rustfmt`, and `clippy` are selected by the checkout's `rust-toolchain.toml`.

Make sure Cargo's bin directory (`~/.cargo/bin` by default) is on `PATH`.

On macOS, install Xcode Command Line Tools (run `xcode-select --install` if they are missing) and [Homebrew](https://brew.sh). The first setup command installs `autoconf`, `autoconf-archive`, `automake`, `ccache`, CMake, `libtool`, `nasm`, Ninja, `pkg-config`, Python, and Qt through Homebrew.

For Linux or WSL2 system packages, use the instructions for your distribution in [Ladybird's build guide](Engine/Documentation/BuildInstructionsLadybird.md). Also install Python's pip package (`python3-pip` on Debian/Ubuntu). Photon requires CMake 3.30 or newer and Ninja 1.10 or newer; some distributions need newer packages than their default repositories provide. Native Windows builds need the Visual Studio C++ developer environment, CMake, Ninja, Python, and `pkg-config`; follow the Windows section of Ladybird's guide and treat that path as experimental. `photon run --ui` builds the Windows shell without the Engine.

On macOS, `./photon setup --install-system-deps` installs the listed native packages with Homebrew, then verifies tools, initializes pinned submodules, configures remotes and persistent development branches, prepares the pinned clang-format version under `build/`, and installs or refreshes the `photon` command in Cargo's bin directory. It does not install Xcode Command Line Tools, Homebrew, Rust, or Python; install those first. On Linux/WSL2, install system packages from Ladybird's guide, then run `./photon setup` without the flag. On Windows, run setup from the configured developer environment; `--install-system-deps` is only implemented for macOS.

## First checkout

On macOS, use `./photon setup --install-system-deps` to install build packages with Homebrew. On Linux/WSL2, install the documented packages first, then use `./photon setup`. On native Windows, use the upstream experimental toolchain described above.

```bash
git clone https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
photon doctor
photon check
photon test
```

Setup installs or refreshes `photon` in Cargo's bin directory. Make sure `~/.cargo/bin` is on `PATH` to use `photon` without the `./` prefix. Run `photon install` to refresh the installed command after changing the CLI without rerunning setup.

The first `photon check` verifies formatting, linting, Rust compilation, and architecture rules. `photon test` runs the workspace tests. On macOS, run `photon build` before `photon run`; the default hot-reload path reuses Engine and does not build it. On Linux and Windows, `photon run` builds and launches the app directly.

Continue with [Building Photon](Documentation/Building.md) for build, run, and development commands.

## Existing checkout

If `photon` is not installed yet, run `./photon setup` from the checkout root; setup installs the command when it completes.

```bash
photon setup
photon deps status
```

Setup initializes missing submodules, refreshes their configured HTTPS clone URLs, and keeps existing worktrees intact. `photon deps status` reports the branch, root pin, worktree cleanliness, and cached upstream counts for each dependency.

Do not use `git submodule update --remote` for routine setup. Merge and test upstream changes in the Photon-owned Engine or GPUI-CE repository, push the tested dependency commit, then update its parent gitlink. See [Upstream maintenance](Documentation/Upstream.md).
