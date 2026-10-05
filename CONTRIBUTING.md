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

The runnable `crates/photon-app` package delegates to the `crates/photon-shell` library, which owns GPUI-CE and Photon-specific native shell integration. Framework-independent browser state belongs in `crates/photon-core`. Engine behavior belongs in the separate `PhotonBrowser/photon-engine` repository, pinned at `Engine/` as a submodule.

`vendor/gpui-ce` is the pinned `PhotonBrowser/gpui-ce` submodule. Keep generic framework changes there and Photon-specific behavior in this repository.

For changes from Ladybird or upstream GPUI-CE, follow [Upstream maintenance](Documentation/Upstream.md). That tutorial covers the required downstream merge, conflict resolution, validation, and updating the exact submodule commit pinned by Photon.

For Engine changes, work and commit inside `Engine/`, push or merge the change into `PhotonBrowser/photon-engine` `master`, then update the `Engine` gitlink here. Preserve `upstream` as Ladybird and `origin` as Photon Engine. For generic GPUI-CE changes, work in `vendor/gpui-ce`, push or merge the change into `PhotonBrowser/gpui-ce` `main`, then update its gitlink here. Photon-specific code stays in this repository.
