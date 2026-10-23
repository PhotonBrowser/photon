<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-cli`

The command-line interface for setting up pinned checkouts, building and running Photon, and maintaining upstream dependencies.

## Use

Install the command once from the Photon repository root:

```sh
cargo install --path crates/photon-cli --bin photon --locked
```

Then use it from anywhere inside the checkout:

```sh
photon setup
photon run --url https://ladybird.org
photon check
photon test
photon deps status
```

Use `photon install` to refresh the installed command after changing it. `photon deps status`, `photon deps sync`, `photon deps restore`, and `photon deps pin` cover the common submodule workflow. Command behavior lives in one module per command under [`src/commands`](src/commands/).
