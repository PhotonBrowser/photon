# iOS development guide

These instructions apply only to files under `ios/`. The iOS target is a
native SwiftUI host for WebKit page content, with a small home surface and
Liquid Glass bottom actions.

## Ownership

- Keep browser rules and durable browser state in `photon-core` and expose
  them through the explicit `photon-ffi` C API.
- Keep Swift limited to native app lifecycle, service composition, and
  platform-page coordination. Do not duplicate Rust browser policy in Swift.
- Keep C ABI pointer and string adaptation in
  `PhotonBrowser/Platform/PhotonCore`.
- Keep `WKWebView` and WebKit delegate types in
  `PhotonBrowser/Platform/WebKit`.
- Keep `Core` independent of SwiftUI and WebKit. Use `BrowserPage` as the
  platform boundary and keep its WebKit implementation in `Platform/WebKit`.
- Do not add Ladybird, the desktop Engine embedder, GPUI, or C++ engine glue to
  the iOS app. Do not use WebKit to render Photon-owned interface.
- Do not create an iOS-specific persistence format. Extend the Rust storage
  API before adding profile persistence.
- Keep UI styling in named tokens under `DesignSystem`; avoid scattered color
  and sizing literals.
- Keep the home screen limited to the explicitly requested surface and bottom
  actions until more product UI is requested.

See [`README.md`](README.md) for the architecture and build workflow.
