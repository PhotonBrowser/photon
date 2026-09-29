# PhotonWebView

`<PhotonWebView url="https://example.com" />` is the only Photon-specific React component in this spike. Its wrapper maps directly to GPUIX's registered `photon-webview` host element. React controls its URL and style; native code owns the engine view and frame presentation.

The view is created after the element has a non-zero GPUI layout size. It navigates once for the initial URL and only navigates again when a committed URL property changes. Layout bounds are converted from logical GPUI pixels to physical engine pixels using the current display scale factor. Resize calls are sent only when the resulting viewport changes.

LibPhotonEmbedder delivers an owned `PresentedFrame` in BGRA format. The frame callback replaces the previous pending frame; the next native update converts the latest frame to GPUI `RenderImage`. Page bytes do not cross JavaScript or N-API. This CPU-backed image path is intentional for the spike and does not claim zero-copy presentation.

The image is painted as part of the GPUI element. `borderRadius` and `overflow: hidden` are implemented by GPUI's scene clipping around the image, rather than a child OS window or a bitmap mask. Parent layout padding controls the outer inset.

A short yielding GPUIX update interval pumps LibPhotonEmbedder while the element is mounted. This keeps engine IPC callbacks responsive without a busy loop. Element removal shuts down the view and runtime in order.
