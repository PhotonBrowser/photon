# Photon

Photon is a native browser shell built with React/TSX, GPUIX and GPUI. Photon Engine provides the Ladybird-derived web platform and rendering engine.

```text
React / TSX (ui/)
      ↓ GPUIX React reconciler
Photon source-built N-API addon
      ↓ GPUI native scene
PhotonWebView (Rust)
      ↓ LibPhotonEmbedder
Photon Engine (Engine/ submodule)
      ↓
Ladybird
```

The shell is intentionally minimal. React controls layout and properties; page frames stay in native code and are presented as GPUI images. There is no Qt application shell.

## Get started

```bash
git clone --recurse-submodules git@github.com:PhotonBrowser/photon.git
cd photon
./photon setup
./photon run
```

For prerequisites and environment setup, see [Contributor setup](CONTRIBUTOR_SETUP.md). For everyday commands and build behavior, see [Building Photon](Documentation/Building.md). [Architecture](Documentation/Architecture.md), [GPUIX integration](Documentation/GPUIX.md), and [PhotonWebView](Documentation/WebView.md) describe the native data flow and presentation. The [upstream maintenance tutorial](Documentation/Upstream.md) explains how to bring Ladybird and GPUIX changes into Photon, resolve conflicts, and update the pinned submodules.

Photon Engine remains a separate, actively maintained downstream of Ladybird in [PhotonBrowser/photon-engine](https://github.com/PhotonBrowser/photon-engine). GPUIX is maintained in [PhotonBrowser/gpuix](https://github.com/PhotonBrowser/gpuix), based on [remorses/gpuix](https://github.com/remorses/gpuix).
