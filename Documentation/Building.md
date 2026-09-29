# Building Photon

Photon's `./photon` script is the developer entry point. It requires Git, Rust/Cargo, a C++ compiler, CMake, Ninja, Python 3, and Bun. Setup initializes the `Engine/` and `vendor/gpuix` submodules and installs the pinned JS dependencies.

```bash
./photon setup
./photon check
./photon test
./photon build
./photon run --verbose
```

The first Engine build configures Ladybird's pinned dependencies under `build/` and compiles Photon Engine helper processes plus LibPhotonEmbedder. The Photon Rust addon is built from source and staged as `photon-native-addon.node`. `run` launches Bun's GPUIX development runtime with that addon selected explicitly.

Incremental behavior:

- A TSX-only edit is picked up by Bun hot reload and does not rebuild Engine or the Rust addon.
- A Photon Rust/native edit rebuilds the addon and does not rebuild Engine unless Engine sources or configuration changed.
- A GPUIX submodule revision change rebuilds its Rust addon dependency.
- An Engine source/revision/configuration change invalidates the Engine build fingerprint and rebuilds Engine.

Use `./photon clean` to remove generated build trees. Build products, lock caches, and downloaded dependencies stay out of tracked source directories.
