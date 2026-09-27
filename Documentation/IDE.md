# IDE setup

Photon keeps editor metadata in the same debug build tree used for normal development. After cloning with submodules, run:

```sh
./photon setup
code .
```

`./photon setup` configures `build/app-debug`, exports its C++ compilation database, generates the Photon QML module metadata, and writes the ignored `ui/.qmlls.ini` for the current checkout. It does not build Photon Engine. `./photon ide setup` repeats this metadata setup after a clean or build-directory change.

The tracked root `.clangd` points clangd at `build/app-debug/compile_commands.json`. This database carries Qt includes, generated headers, platform definitions, and the Photon Engine include paths from CMake. The app CMake project always exports this database; Engine's database remains separately at `build/engine-debug/compile_commands.json`.

Qt's QML language server reads `ui/.qmlls.ini`, then the build-generated `.qt/.qmlls.build.ini`, `Photon/qmldir`, and `Photon/photon.qmltypes`. The config file is ignored because its build directory is machine-specific. `./photon check` reconfigures this metadata and refreshes the QML type registration target before linting, so source changes do not get checked against stale QML copies.

Rust Analyzer can open the root `Cargo.toml` directly. It discovers the Cargo workspace containing `photon-cli`, `photon-core`, and `photon-qml-tools`; there is no editor-only manifest.

The root clangd database covers Photon app sources. Engine is a separate repository with its own `.clangd` and Cargo workspace; it is not added to the app compilation database or root Cargo workspace. For Engine work, open `Engine/` in a separate VS Code window and point clangd at the corresponding Photon build database, for example `../build/engine-debug/compile_commands.json` from the Engine directory. Build that tree with `./photon engine build` first. Rust Analyzer discovers Engine's `Cargo.toml` when Engine is opened separately. This keeps the submodule's upstream editor configuration separate from normal Photon app diagnostics.

If diagnostics look stale or a build tree was removed, run `./photon doctor` for paths and status, then `./photon ide setup` to regenerate the metadata. The debug build is canonical for the editor; release builds use a separate tree and do not change editor selection.
