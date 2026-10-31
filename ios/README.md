# Photon for iOS

The iOS target is a native SwiftUI app host with a Rust/WebKit backend. Its
home screen currently has the pastel dotted surface and Liquid Glass bottom
actions shown in the design reference; browser chrome and page content remain
to be wired in. The app does not use Ladybird, the desktop Engine embedder,
GPUI, or a web page to render Photon’s interface.

## Design

```text
HomeScreen
        │
        ▼
    AppServices ── WebKitBrowserAdapter ── BrowserPage interface
                         │                    │
                         ▼                    ▼
                 PhotonCoreBrowser       WebKitPage
                         │                    │
                  photon-ffi              WKWebView
                         │
                   photon-core
```

- `photon-core` owns address/search resolution, page-state rules, and command
  availability.
- `photon-ffi` exposes those rules and state operations through the repository’s
  C ABI. `PhotonCoreBrowser` is the Swift ownership and string-conversion
  boundary for one page handle.
- `WebKitBrowserAdapter` is platform glue: it sends commands to the Rust
  browser handle, drives one native page surface, and forwards page events.
  It contains no browser policy.
- `BrowserPage` is the page-service interface. `WebKitPage` owns `WKWebView`,
  navigation delegates, and translation of WebKit state into browser events.
  Its `platformView` is the surface a future SwiftUI view can host.
- `AppServices` is the composition root for the Rust and WebKit adapters.

The Rust handle is created per page. Rust resolves address-bar input before
WebKit loads the resulting URL, and WebKit reports navigation state back
through the same handle. WebKit’s default persistent website data store is used.

The current FFI does not expose a Rust tab/session manager or
`photon-storage` profile APIs. The iOS scaffold therefore does not implement
tab policy, session restoration, or a second Swift persistence format. Extend
the Rust API before adding those features to the iOS app.

## CLI workflow

On macOS with Xcode and Rust installed, the repository CLI can list targets,
build the app, and launch it in a simulator or on a connected iPhone:

```sh
photon ios devices
photon ios devices --kind model
photon ios devices --kind physical --paired
photon ios devices --connected
photon ios build --model "iPhone 18 Pro"
photon ios run --iphone "iPhone 18 Pro"
photon ios run --device "My iPhone" --team-id ABCDE12345
```

`--model` (also `--iphone`) selects an iPhone simulator model. If that model
does not have a simulator instance yet, the CLI creates one using the latest
compatible iOS runtime installed in Xcode. `--device` selects an existing
simulator or a physical iPhone by its Xcode device name or UDID. Physical
launches require a paired, connected phone and a valid Xcode signing setup.
Provide the Apple Developer Team ID with `--team-id` or configure signing in
Xcode. Add `--allow-provisioning-updates` when Xcode should update profiles
through the Apple Developer account. If the phone is not registered to that
team yet, also pass `--allow-device-registration`.

Without a target, `photon ios build` builds for a generic simulator. `launch`
and `run` use the only booted simulator, or the only installed iPhone simulator
if none is booted; when the target is ambiguous, pass `--model` or `--device`.
Build products go under the ignored `build/ios/DerivedData/` directory. The
Xcode build phase compiles `photon-ffi` for the selected iOS SDK and Rust target.
The app currently launches to the home surface; its bottom actions are
presentation components ready to receive app-service closures.

## Source layout

```text
ios/
├── README.md
├── scripts/
│   └── build-rust-ffi.sh       # Builds the iOS Rust static library
├── PhotonBrowser.xcodeproj/
└── PhotonBrowser/
    ├── App/                    # App entry point and service composition
    ├── Core/
    │   ├── Browser/            # Rust page-state snapshot types
    │   └── Services/           # Platform page interface
    ├── DesignSystem/           # Shared semantic colors and glass surfaces
    ├── Features/
    │   └── Home/               # Dotted home surface and bottom actions
    └── Platform/
    │   ├── PhotonCore/         # Swift wrapper and C bridging header
    │   ├── WebKit/             # WKWebView adapter and delegate mapping
    │   └── Persistence/        # Reserved for a Rust-backed persistence adapter
    └── Tests/                  # Reserved for future test targets
```

Keep browser policy and durable browser state in Rust. Keep WebKit types inside
`Platform/WebKit`, C ABI adaptation inside `Platform/PhotonCore`, shared visual
tokens in `DesignSystem`, and app composition in `App`.
