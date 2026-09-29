# Contributor setup

Clone the browser and its pinned dependencies, then initialize the development environment:

```bash
git clone --recurse-submodules git@github.com:PhotonBrowser/photon.git
cd photon
./photon setup
./photon doctor
```

Run `./photon build` to build Photon Engine and the source-built GPUIX addon. `./photon run` launches the GPUIX React development runtime. See [Building](Documentation/Building.md) for incremental build behavior and [Architecture](Documentation/Architecture.md) for ownership boundaries.

`Engine/` uses `PhotonBrowser/photon-engine` as `origin`; its `upstream` should point to `LadybirdBrowser/ladybird`. `vendor/gpuix` uses `PhotonBrowser/gpuix` as its pinned source. Both dependencies are initialized through Git submodules.
