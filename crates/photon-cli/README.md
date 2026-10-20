<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-cli`

The command-line interface behind the repository’s `./photon` script. It sets up the pinned checkouts, builds and runs Photon, and provides the project’s format, check, test, and upstream maintenance commands.

## Use

Run commands from the Photon repository root:

```sh
./photon setup
./photon run --url https://ladybird.org
./photon check
./photon test
```

The shell script at the repository root builds and invokes this crate. Command behavior lives in one module per command under [`src/commands`](src/commands/).
