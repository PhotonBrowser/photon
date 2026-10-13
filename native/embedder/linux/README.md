# Linux native embedder adapter

Linux-specific native bridge sources belong in this directory. The generic
LibPhotonEmbedder ABI remains in `../PhotonEmbedderBridge.h`; the macOS bridge
implementation and AppKit helpers live beside it and in `../macos/`.

The Linux adapter still needs to choose and implement system clipboard,
reduced-motion, default-browser, window/display observation, and native page
surface integration. Add its sources and target link inputs in
`crates/photon-shell/build.rs` under the Linux target branch when those
interfaces are defined. The macOS IOSurface, Mach port, Metal, and XPC path
does not belong here.
