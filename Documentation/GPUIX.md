# GPUIX integration

Photon pins GPUIX `@gpuix/native` 0.10.0 through `vendor/gpuix` at commit `6fa1d25f7f45a5e7e5d61a8b2ad17473595b5f41`. The submodule's `origin` is `git@github.com:PhotonBrowser/gpuix.git`; `upstream` is `https://github.com/remorses/gpuix.git`. Rust and the local `@gpuix/native` file dependency use this one checkout.

The pinned fork adds a generic public `CustomElement` and `CustomElementFactory` contract, `CustomRenderContext` accessors, deterministic static factory registration, and renderer registry integration. GPUIX retains ownership of its custom-element registry and instances, and preserves built-in dispatch. Its element trait no longer requires an external crate to name the private renderer context. A yielding update task lets custom elements request native polling without a busy loop. Photon registers `PhotonWebViewElement` from the source-built addon; no Photon code is in GPUIX.

The addon links GPUIX's N-API implementation and Photon registrations into one native module. The React adapter loads that module using `NAPI_RS_NATIVE_LIBRARY_PATH`; Photon does not load the published native binary alongside it.

The page frame path is native throughout:

```text
Ladybird → LibPhotonEmbedder PresentedFrame → Photon Rust → GPUI RenderImage
```

Frame bytes never pass through JavaScript. The transitional presentation path uses an owned BGRA frame; it is not zero-copy. The Photon element measures its laid-out GPUI bounds, converts them to physical pixels using the GPUI scale factor, and resizes the engine view when that size changes. Image painting is inside the normal GPUI element hierarchy, where the host style applies rounded clipping.

To bring upstream changes into the fork or update Photon’s pin, follow [Upstream maintenance](Upstream.md). Do not add a separate patch application step or package-resolution fallback that can load another GPUIX native addon.
