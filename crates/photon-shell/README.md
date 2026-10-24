<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-shell`

The shared GPUI-CE browser window and platform adapters. It owns browser views, window lifecycle, native system integration, Engine session adaptation, and page-frame presentation for supported shell targets. Browser actions and presentation behavior are shared; OS-specific window and renderer implementations live under `src/platform/<target>/`.

## Run

The shell is started by `photon-app`; use the repository command so the pinned Engine and GPUI-CE dependencies are prepared:

```sh
photon setup
photon run
```

The shell composes browser rules from [`photon-core`](../photon-core/README.md) and profile files from [`photon-storage`](../photon-storage/README.md). See the [architecture guide](../../Documentation/Architecture.md) for its platform boundaries.
