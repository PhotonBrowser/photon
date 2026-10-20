<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-shell`

The native GPUI-CE browser window and Photon Engine adapter. It owns browser views, window lifecycle, platform integration, and page-frame presentation for supported shell targets.

## Run

The shell is started by `photon-app`; use the repository command so the pinned Engine and GPUI-CE dependencies are prepared:

```sh
./photon setup
./photon run
```

The shell composes browser rules from [`photon-core`](../photon-core/README.md), address handling from [`photon-omnibox`](../photon-omnibox/README.md), and profile files from [`photon-storage`](../photon-storage/README.md). See the [architecture guide](../../Documentation/Architecture.md) for its platform boundaries.
