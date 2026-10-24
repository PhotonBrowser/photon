<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-cli`

The command-line interface for setting up pinned checkouts, building and running Photon, and maintaining upstream dependencies.

## Use

Bootstrap the command from the Photon repository root:

```sh
./photon setup
```

Setup installs or refreshes `photon` in Cargo's bin directory. Make sure `~/.cargo/bin` is on `PATH`, then use it inside the checkout:

```sh
photon setup
photon doctor
photon run --url https://ladybird.org
photon check
photon test
photon deps status
```

Use `photon setup --install-system-deps` on macOS to install build packages with Homebrew; on Linux, install packages from the contributor guide first. Use `photon install` to refresh the installed command after changing it without rerunning setup. `photon deps status`, `photon deps sync`, `photon deps restore`, and `photon deps pin` cover the common submodule workflow. Command behavior lives in one module per command under [`src/commands`](src/commands/).
