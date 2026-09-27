# Contributing

Clone with `git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git`, then run `./photon setup` and `./photon doctor`.

`./photon check` runs the Rust checks, QML lint, and architecture checks. QML lint warnings and errors fail the command and print a copyable AI-agent prompt containing the diagnostics and project instructions. Run `./photon fix-prompt` to print the prompt on demand. `./photon format` formats Rust and QML; Qt 6 `qmllint` and `qmlformat` must be installed.

Run `./photon --help` for commands and options, or `./photon <command> --help` for command details. `./photon build` and `./photon run` run the full check first and stop if it fails. Build commands show phase progress in interactive terminals; pass `--verbose` to display the underlying tool output.

Photon UI changes belong in `ui/` as QML. Browser/application domain and future state belong in Rust crates. `native/qt/` is limited to Qt Quick types and engine integration. Engine behavior belongs in the separate `PhotonBrowser/photon-engine` repository, checked out at `Engine/` here as a submodule.

For engine work, create a branch in `Engine/`, modify and commit there, then update the `Engine` gitlink in the Photon repository after the engine commit is available from its origin. Keep generated output in `build/`.
