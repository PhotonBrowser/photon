# Linux native embedder adapter

Linux-specific native bridge sources belong in this directory. The generic
LibPhotonEmbedder ABI remains in `../PhotonEmbedderBridge.h`; the macOS bridge
implementation and AppKit helpers live beside it and in `../macos/`.

The Rust Linux adapter handles GNOME reduced motion and default-browser
selection through `gsettings` and `xdg-settings`. The Engine clipboard bridge is
connected to GPUI's Wayland and X11 clipboard adapters. Remaining work includes
passing reduced-motion changes into Engine pages, observing window
visibility/display changes, and importing native page surfaces. Add native
sources and target link inputs in `crates/photon-shell/build.rs` under the Linux
target branch when needed. The macOS IOSurface, Mach port, Metal, and XPC path
does not belong here.
