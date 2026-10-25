# Contributor setup

Photon's primary development platform is macOS. Linux support is experimental: the Rust shell and Engine build paths exist, and Ubuntu 24.04 is exercised by CI, but a full interactive browser run is not currently validated in CI.

## macOS

Install Xcode Command Line Tools (`xcode-select --install`), [Homebrew](https://brew.sh), Rust using [rustup](https://rustup.rs), and Python 3 with pip. The repository pins Rust 1.99.0 and includes `rustfmt` and Clippy in `rust-toolchain.toml`. Ensure `~/.cargo/bin` is on `PATH`.

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

The system dependency option installs Ladybird's Homebrew build packages. Setup verifies tools, initializes missing pinned submodules, configures their remotes and persistent development branches, prepares formatting tools, and installs the `photon` command in Cargo's bin directory. It does not install Xcode tools, Homebrew, Rust, or Python.

## Linux

Ubuntu 24.04 is the configured CI target; other Linux distributions may work if they meet Ladybird's current requirements. The latest CI run failed before Rust checks because its checkout did not contain the vendored `gpui-motion` package; the current revision includes it, but Linux validation is pending a new CI run. Install Rust with rustup, Python 3 with pip, Git, a C++23-capable compiler, CMake 3.30+, Ninja 1.10+, Qt 6.10+ development packages, NASM, and the distribution-specific libraries listed in [Ladybird's Linux build instructions](Engine/Documentation/BuildInstructionsLadybird.md). Some distributions need newer compiler, CMake, or Qt packages than their standard repositories provide.

```sh
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
photon check
photon test
photon build
photon run
```

Setup does not install Linux packages. `photon check` and `photon test` are included in the Ubuntu CI workflow but are not yet verified on Linux. `photon build` and `photon run` are implemented using the portable Linux Engine and GPUI paths, but need a working graphical session and have not yet been verified as end-to-end runtime paths by CI. `photon run --ui` launches the shell without the Engine and cannot load web pages.

## Recover an existing checkout

If cloning was done without submodules, initialize the pinned dependencies and then rerun setup:

```sh
git submodule update --init --recursive
./photon setup
```

If setup reports a missing tool, install the named prerequisite and retry. Setup can be rerun: it initializes missing submodules and preserves initialized dependency branches and commits. It does configure remotes to the HTTPS URLs in `photon.toml`; it does not advance dependency pins. Check `photon deps status` before dependency work. Do not use `git submodule update --remote` for routine setup.

See [Building Photon](Documentation/Building.md) for command behavior and detailed troubleshooting, and [Upstream maintenance](Documentation/Upstream.md) for dependency branch and pin workflows.
