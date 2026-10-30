<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-cli`

The command-line interface for setting up pinned checkouts, building and running Photon, and maintaining upstream dependencies.

## Use

Install [Rust with rustup](https://rustup.rs) first; `./photon` runs Cargo and cannot install it. See the [contributor setup guide](../../CONTRIBUTOR_SETUP.md) for platform prerequisites. Then bootstrap from the repository root:

```sh
./photon setup
```

Setup installs or refreshes `photon` and its `ph` alias in Cargo's bin directory. Make sure `~/.cargo/bin` is on `PATH`, then use either command inside the checkout:

```sh
photon doctor
photon run --url https://ladybird.org
photon check
photon test
photon deps status
```

For example, `ph check` and `ph run` work the same as `photon check` and `photon run`.

Build and run commands use optimized builds by default; pass `--debug` for faster, unoptimized builds. `photon setup --install-system-deps` on macOS installs build packages with Homebrew; on Linux, install packages from the contributor guide first. Use `photon install` to refresh the installed command after changing it without rerunning setup. `photon deps status`, `photon deps sync`, `photon deps restore`, and `photon deps pin` cover the common submodule workflow. Command behavior lives in one module per command under [`src/commands`](src/commands/).
