# Contributing

Clone with `git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git`, then run `./photon setup` and `./photon doctor`.

Photon UI changes belong in `ui/` as QML. Browser/application domain and future state belong in Rust crates. `native/qt/` is limited to Qt Quick types and engine integration. Engine behavior belongs in the separate `PhotonBrowser/photon-engine` repository, checked out at `Engine/` here as a submodule.

For engine work, create a branch in `Engine/`, modify and commit there, then update the `Engine` gitlink in the Photon repository after the engine commit is available from its origin. Keep generated output in `build/`.
