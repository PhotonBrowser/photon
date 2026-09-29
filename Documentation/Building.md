# Building Photon

Photon's `./photon` script is the developer entry point. It requires Git, Rust/Cargo 1.85 or newer, a C++23-capable compiler, CMake 3.30 or newer, Ninja 1.10 or newer, Python 3, Bun, and the system development packages required by the pinned Engine. `./photon setup` initializes the pinned submodules recursively, configures remotes from `photon.toml`, fetches the configured Zed upstream branch, and installs the pinned JavaScript dependencies. It prepares the checkout; `./photon build` performs the first Engine and native addon build. See [Contributor setup](../CONTRIBUTOR_SETUP.md) for first checkout and recovery steps, and [Ladybird's build instructions](../Engine/Documentation/BuildInstructionsLadybird.md) for platform packages.

```bash
./photon setup
./photon check
./photon test
./photon build
./photon run --verbose
```

Use `./photon run --ui` to work on the shell without building or starting Photon Engine. This mode still checks TypeScript and Rust and builds the source GPUIX addon with the `engine` feature disabled. The `PhotonWebView` host element remains in the React tree and paints a plain white surface; it does not create a runtime or view, load a URL, link the Engine bridge, or use Engine helper processes. It launches the same GPUIX window and hot-reload runtime.

The first Engine build configures Ladybird's pinned dependencies under `build/` and compiles Photon Engine helper processes plus LibPhotonEmbedder. The Photon Rust addon is built from source and staged as `photon-native-addon.node`. `run` launches Bun's GPUIX development runtime with that addon selected explicitly.

Incremental behavior:

- A TSX-only edit is picked up by Bun hot reload and does not rebuild Engine or the Rust addon.
- A Photon Rust/native edit rebuilds the addon and does not rebuild Engine unless Engine sources or configuration changed.
- A GPUIX submodule revision change rebuilds its Rust addon dependency.
- An Engine source/revision/configuration change invalidates the Engine build fingerprint and rebuilds Engine.

The UI-only run compiles the Rust shell addon but skips the Engine build entirely. A later normal `./photon run` still builds Engine as needed and rebuilds the addon with Engine support enabled.

Use `./photon clean` to remove generated build trees. Build products, lock caches, and downloaded dependencies stay out of tracked source directories.

To update Ladybird or GPUIX, first merge the upstream changes into Photon’s downstream dependency repository, test them there, then update Photon’s pinned submodule commit. See [Upstream maintenance](Upstream.md); do not advance dependencies with `git submodule update --remote` as part of an ordinary Photon build.
