# Building

Requirements: Git, Rust/Cargo, CMake 3.25+, Ninja, Clang, Qt 6 Quick/Qml development packages, and the platform libraries required by Ladybird. `./photon setup` checks the main tools and initializes `Engine/`; it does not install system packages.

```sh
./photon setup
./photon doctor
./photon build
./photon run
./photon check
./photon format
./photon test
./photon clean
```

Builds are out of tree in `build/engine-debug`, `build/app-debug` and corresponding release directories. On the first engine build, the CLI bootstraps the vcpkg revision pinned by Ladybird and downloads/builds its third-party dependencies under `build/`. Use `--release` on build/run and `--verbose` to show configure/build output. Engine-only commands are `./photon engine status`, `./photon engine build`, and `./photon engine sync`.

`./photon clean` removes generated application, engine and helper binaries while keeping the vcpkg checkout and dependency caches for faster rebuilds. `./photon clean engine` removes only the engine build tree and helper binaries.

For an engine sync, ensure `Engine/` is clean, then run `./photon engine sync`. This fetches `upstream` and merges `upstream/master` into the current engine branch. Resolve conflicts in `Engine/`, validate the result, publish it through the normal engine repository review process, and then update and commit the `Engine` gitlink in Photon. Neither repository is pushed by the CLI.
