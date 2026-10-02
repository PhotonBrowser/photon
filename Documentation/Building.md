# Building Photon

Photon's `./photon` script is the developer entry point. It requires Git, Rust/Cargo 1.85 or newer, a C++23-capable compiler, CMake 3.30 or newer, Ninja 1.10 or newer, Python 3, and the platform packages required by the pinned Engine. Bun and a browser-based UI host are not used.

```bash
./photon setup
./photon check
./photon test
./photon build
./photon run --verbose
```

`./photon run` builds the Engine and the direct GPUI-CE application, starts the Photon-owned macOS presentation broker, then opens the native window. Use `--url` to choose the initial page. `--shutdown-after-seconds N` runs for a fixed interval and then drains Metal work and frame leases; verbose mode prints the final submitted/completed/released/outstanding counts.

The first Engine build configures Ladybird dependencies under `build/` and compiles Photon Engine helper processes plus LibPhotonEmbedder. The app remains Rust, with C++ limited to the narrow LibPhotonEmbedder bridge and required Apple XPC glue.

Incremental builds use the Engine fingerprint to rebuild Ladybird only when Engine sources or configuration change. Rust app or GPUI-CE changes rebuild the application. Build products and downloaded dependencies stay under ignored build directories.

Use `./photon clean` to remove generated build trees and stop the checkout-specific presentation broker. `./photon setup` selects the persistent Engine and GPUI-CE development branches; `./photon sync --check` previews ahead/behind counts and conflicts, and `./photon sync` merges both upstreams. The `engine sync` and `gpui sync` subcommands work on one dependency and also accept `--check`. After testing and pushing dependency commits, `./photon pin` records their current commit IDs in Photon. See [Upstream maintenance](Upstream.md) for the distinction between local branches and recorded pins.
