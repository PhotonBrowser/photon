# Architecture

Photon is split across two repositories. This browser repository owns application behavior, tooling, QML and the Qt Quick shell. `Engine/` pins one exact Photon Engine revision; `photon-engine` owns the Ladybird-derived engine, services, tests and Photon-specific engine interfaces.

Rust is the application layer and must not depend on Qt. QML owns presentation. C++ in `native/qt/` is limited to Qt classes and native engine glue. `PhotonWebView` is a `QQuickItem` that owns the Qt/engine lifetime bridge and uploads engine frames as scene-graph textures; no QWidget embedding is part of the architecture.

The Rust workspace keeps command concerns separate: `photon-cli` is a thin executable with command modules for setup, builds, checks, formatting, and Engine operations. `photon-qml-tools` owns Qt tool discovery plus QML linting and formatting, so those details do not spread through command code. `photon-core` remains the browser application domain crate.

`LibPhotonEmbedder` exposes Photon-owned `Runtime` and `View` types over the existing `LibWebView` lifecycle. Runtime initialization starts Ladybird services and `pump()` advances the engine event loop. A View owns one `HeadlessWebView`, supports navigation/history/resize/focus and translates pointer and key input. `ViewCallbacks` reports URL, title, loading/history state, failures and presented frames. The API header is the only engine surface included by the Qt shell.

For the bootstrap renderer, WebContent/Compositor use CPU painting. The engine copies its current shared-image bitmap synchronously during the paint callback into an owned BGRA premultiplied `PresentedFrame`. Qt copies that frame into an owned `QImage`, schedules `QQuickItem::update()`, then creates/uploads a `QSGTexture` in `updatePaintNode()`. This is CPU-backed and involves copies; it is not shared-GPU or zero-copy presentation. Callback and event-loop pumping currently run on the Qt GUI thread, while Qt texture creation happens on the scene-graph render thread through Qt's normal update lifecycle.

Logical item dimensions and device-pixel ratio are sent to the engine on initial creation and geometry/screen changes; the embedder rounds logical size times DPR to physical pixels. Qt mouse, wheel, key, text and focus events are translated at `PhotonWebView` and passed through LibPhotonEmbedder. The first navigation is currently fixed to `https://example.com` for integration validation. Rust browser state has not yet been connected to the runtime API.

The current shutdown order is View before Runtime: `PhotonWebView` explicitly resets the view, then the runtime. The view callbacks are destroyed with the view. Helper processes are owned by Ladybird's existing application lifecycle. Three repeated window-close runs under gdb exited normally, and no helper processes remained afterward.

Generated files and build trees belong under `build/`. Do not put tabs, config or other product state in C++ or Photon Engine.
