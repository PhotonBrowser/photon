# Contributing

Run `./photon setup` after cloning. Use the repository entry point for everyday work:

```bash
./photon format
./photon check
./photon test
./photon build
./photon run
```

Pass `--verbose` to `check`, `build`, or `run` to see underlying tool output. Keep generated output under `build/` and `target/`.

Photon UI composition belongs in `ui/` as React/TSX. Native GPUI integration belongs in `crates/photon-gpui` and `crates/photon-native-addon`; framework-independent browser state belongs in `crates/photon-core`. Engine behavior belongs in the separate `PhotonBrowser/photon-engine` repository, pinned at `Engine/` as a submodule.

`vendor/gpuix` is the pinned `PhotonBrowser/gpuix` submodule. It is the single canonical GPUIX source used by Rust and the local `@gpuix/native` package. Keep generic framework changes there and Photon-specific behavior in this repository.

For changes from Ladybird or upstream GPUIX, follow [Upstream maintenance](Documentation/Upstream.md). That tutorial covers the required downstream merge, conflict resolution, validation, and updating the exact submodule commit pinned by Photon.

For Engine changes, work and commit inside `Engine/`, push or merge the change into `PhotonBrowser/photon-engine` `master`, then update the `Engine` gitlink here. Preserve `upstream` as Ladybird and `origin` as Photon Engine. For generic GPUIX changes, work in `vendor/gpuix`, push or merge the change into `PhotonBrowser/gpuix` `main`, then update its gitlink here. Photon-specific code stays in this repository.
