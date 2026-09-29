# GPUIX integration

Photon pins GPUIX 0.10.0 through `vendor/gpuix` at commit `7bb2382f64ca5c1be3b8490c8a7242f634553016`. The submodule remote is `git@github.com:PhotonBrowser/gpuix.git`; the corresponding upstream is `https://github.com/remorses/gpuix.git`. Rust and the local `@gpuix/native` file dependency use this same checkout.

The pinned fork adds a generic public `CustomElement` and `CustomElementFactory` contract, `CustomRenderContext` accessors, deterministic static factory registration, and renderer registry integration. GPUIX retains ownership of its custom-element registry and instances, and preserves built-in dispatch. Its element trait no longer requires an external crate to name the private renderer context. A yielding update task lets custom elements request native polling without a busy loop. Photon registers `PhotonWebViewElement` from the source-built addon; no Photon code is in GPUIX.

The addon links GPUIX's N-API implementation and Photon registrations into one native module. The React adapter loads that module using `NAPI_RS_NATIVE_LIBRARY_PATH`; Photon does not load the published native binary alongside it.

The page frame path is native throughout:

```text
Ladybird → LibPhotonEmbedder PresentedFrame → Photon Rust → GPUI RenderImage
```

Frame bytes never pass through JavaScript. The transitional presentation path uses an owned BGRA frame; it is not zero-copy. The Photon element measures its laid-out GPUI bounds, converts them to physical pixels using the GPUI scale factor, and resizes the engine view when that size changes. Image painting is inside the normal GPUI element hierarchy, where the host style applies rounded clipping.

To update GPUIX, choose and commit a revision in `PhotonBrowser/gpuix`, update this repository's submodule pointer, and validate the GPUIX Rust/TypeScript tests and Photon build. Do not add a separate patch application step or package-resolution fallback that can load another GPUIX native addon.
