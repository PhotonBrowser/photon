# Photon

Photon is a native Rust browser shell around the Ladybird-derived Photon Engine. GPUI-CE owns the native window and samples real Engine IOSurfaces through Metal.

```text
Photon Rust application
        ↓
     GPUI-CE
        ↓
   PhotonWebView
        ↓
Photon presentation broker
        ↓ IOSurface + MTLSharedEvent
Ladybird / Skia
```

The shell is native GPUI throughout: tabs, an omnibox with history suggestions, menus, and the browser's own pages such as the new tab page and settings. Each web page is an Engine view. Browser state stays in Rust; page pixels and platform handles never enter a JavaScript runtime.

## Get started

```bash
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup
./photon run
```

See [Contributor setup](CONTRIBUTOR_SETUP.md), [Building Photon](Documentation/Building.md), [Architecture](Documentation/Architecture.md), [theme and style tokens](Documentation/Theme.md), [PhotonWebView](Documentation/WebView.md), [the browser's own pages](Documentation/Pages.md), [profile, settings, history and branding](Documentation/Profile.md), [the omnibox](Documentation/Omnibox.md), and [upstream maintenance](Documentation/Upstream.md). Performance results are in [Performance](Documentation/Performance.md). Current features and what comes next are in the [Roadmap](Documentation/Roadmap.md).

Photon Engine is maintained in [PhotonBrowser/photon-engine](https://github.com/PhotonBrowser/photon-engine), downstream of Ladybird. Photon maintains its GPUI-CE fork at [PhotonBrowser/gpui-ce](https://github.com/PhotonBrowser/gpui-ce), upstream of [gpui-ce/gpui-ce](https://github.com/gpui-ce/gpui-ce).
