# Contributor setup

Photon's primary development platform is macOS. Linux support is experimental: Ubuntu 24.04 CI validates workspace checks, CLI smoke commands, and tests in the [latest passing run](https://github.com/PhotonBrowser/photon/actions/runs/40202068735), but a full interactive browser run is not currently validated in CI.

## Bootstrap prerequisites

Install Rust with [rustup](https://rustup.rs) before running `./photon setup`. The `./photon` launcher starts the CLI with `cargo run`, so it cannot install Rust for you. In the repository, rustup selects the pinned Rust 1.99.0 toolchain from `rust-toolchain.toml` (and installs it on first use). Ensure `~/.cargo/bin` is on `PATH` so `cargo`, `rustc`, and `rustup` are available.

## macOS

Install Xcode Command Line Tools (`xcode-select --install`), [Homebrew](https://brew.sh), and Python 3 with pip, in addition to the Rust bootstrap prerequisites above. The pinned toolchain includes `rustfmt` and Clippy.

Clone and set up Photon:

```sh
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup --install-system-deps
photon check
photon test
photon build
photon run
```

The system dependency option installs Ladybird's Homebrew build packages. Setup verifies tools, initializes missing pinned submodules, configures their remotes and persistent development branches, prepares formatting tools, and installs the `photon` command and `ph` alias in Cargo's bin directory. It does not install Xcode Command Line Tools, Homebrew itself, Rust, or Python.

## Linux

Ubuntu 24.04 is the CI-checked distribution; other Linux distributions may work if they meet Ladybird's current requirements.

To work on the GPUI shell without downloading or preparing the Engine, install Git and the Rust bootstrap prerequisites above, then run:

```sh
git clone https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup --ui-only
photon run --ui
```

This initializes GPUI-CE and skips Engine, Qt, Python, CMake, Ninja, C++ compiler, and clang-format setup. It still needs the native desktop libraries required by GPUI-CE and a graphical Linux session. Web pages and Engine-backed form controls are unavailable in this mode. You can run the full setup later; it initializes the Engine and keeps the existing GPUI-CE worktree.

For the full browser, install Python 3 with pip, a C++23-capable compiler, CMake 3.30+, Ninja 1.10+, Qt 6.10+ development packages, NASM, and the distribution-specific libraries listed in [Ladybird's Linux build instructions](Engine/Documentation/BuildInstructionsLadybird.md). Some distributions need newer compiler, CMake, or Qt packages than their standard repositories provide.

```sh
git clone https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
photon check
photon test
photon build
photon run
```

Setup does not install Linux packages. `photon check` and `photon test` pass in Ubuntu CI. `photon build` and full `photon run` use the portable Linux Engine and GPUI paths, but need a working graphical session and have not yet been verified as end-to-end runtime paths by CI. `photon run --ui` launches the shell without the Engine and cannot load web pages.

## Recover an existing checkout

If cloning was done without submodules, initialize the pinned dependencies and then rerun setup:

```sh
git submodule update --init --recursive
./photon setup
```

If setup reports a missing tool, install the named prerequisite and retry. Setup can be rerun: it initializes missing submodules and preserves initialized dependency branches and commits. It does configure remotes to the HTTPS URLs in `photon.toml`; it does not advance dependency pins. Check `photon deps status` before dependency work. Do not use `git submodule update --remote` for routine setup.

See [Building Photon](Documentation/Building.md) for command behavior and detailed troubleshooting, and [Upstream maintenance](Documentation/Upstream.md) for dependency branch and pin workflows.
