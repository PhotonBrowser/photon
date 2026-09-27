# Architecture

Photon is split across two repositories. This browser repository owns application behavior, tooling, QML and the Qt Quick shell. `Engine/` pins one exact Photon Engine revision; `photon-engine` owns the Ladybird-derived engine, services, tests and Photon-specific engine interfaces.

Rust is the application layer and must not depend on Qt. QML owns presentation. C++ in `native/qt/` is limited to Qt classes and native engine glue. The initial `PhotonWebView` is a `QQuickItem` boundary without engine rendering. The next graphics step will use a scene-graph/RHI-compatible presentation API; no QWidget embedding is part of the target architecture.

`LibPhotonEmbedder` is introduced as a named engine build boundary over the existing `LibWebView` implementation. It does not yet provide the final stable runtime/view API; runtime ownership, view lifecycle, navigation events and page surface presentation must be moved behind explicit Photon types before browser logic calls engine internals.

Generated files and build trees belong under `build/`. Do not put tabs, config or other product state in C++ or Photon Engine.
