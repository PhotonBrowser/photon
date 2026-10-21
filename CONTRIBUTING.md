# Contributing

Start with [contributor setup](CONTRIBUTOR_SETUP.md). Use the repository entry point for everyday work:

```bash
./photon format
./photon check
./photon test
./photon build
./photon run
```

See [Building Photon](Documentation/Building.md) for command details. Keep generated output under `build/` and `target/`.

The runnable `crates/photon-app` package delegates to `crates/photon-shell`, which owns GPUI-CE and Photon-specific native shell integration. Framework-independent browser state and rules belong in `crates/photon-core`; the native C ABI belongs in `crates/photon-ffi`. Shared GPUI shell colors, measurements, and views live under `crates/photon-shell/src/platform/ui/`; target-specific window controls, system services, and renderers belong under `crates/photon-shell/src/platform/<target>/`. Linux integration belongs under `crates/photon-shell/src/platform/linux/`. Engine behavior belongs in the separate `PhotonBrowser/photon-engine` repository, pinned at `Engine/` as a submodule.

`./photon format` and `./photon check` cover every Rust package in the workspace. Keep crate ownership focused: add a package when it has an independent boundary or a useful consumer outside its current crate.

`vendor/gpui-ce` is the pinned `PhotonBrowser/gpui-ce` submodule. Keep generic framework changes there and Photon-specific behavior in this repository.

For Engine or GPUI-CE changes, follow [Upstream maintenance](Documentation/Upstream.md) for dependency branches, remotes, validation, and updating the pinned commit.
