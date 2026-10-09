# photon-presentation-ipc

This crate owns Photon’s macOS presentation XPC protocol, Rust client API,
Objective-C transport, and the service entry point used by the broker binary.
Keep frame scheduling and lease policy in `photon-shell`; keep the service
executable wrapper in `photon-presentation-broker`.

Keep the XPC contract narrow and focused on transferring IOSurface and shared
event handles plus frame metadata. Keep generic GPU rendering behavior in
GPUI-CE and Engine behavior in `Engine/`.
