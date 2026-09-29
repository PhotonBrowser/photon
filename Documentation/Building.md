# Building Photon

Photon's `./photon` script is the developer entry point. It requires Git, Rust/Cargo 1.85 or newer, a C++ compiler, CMake 3.25 or newer, Ninja 1.10 or newer, Python 3, and Bun. `./photon setup` initializes the `Engine/` and `vendor/gpuix` submodules recursively, configures their remotes, and installs the pinned JavaScript dependencies. See [Contributor setup](../CONTRIBUTOR_SETUP.md) for first checkout and recovery steps.

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

To update Ladybird or GPUIX, first merge the upstream changes into Photon’s downstream dependency repository, test them there, then update Photon’s pinned submodule commit. See [Upstream maintenance](Upstream.md); do not advance dependencies with `git submodule update --remote` as part of an ordinary Photon build.
