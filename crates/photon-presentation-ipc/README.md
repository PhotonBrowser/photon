<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-presentation-ipc`

Photon’s macOS XPC transport for sharing IOSurface and Metal shared-event handles between the browser shell and presentation broker. It owns the narrow protocol and client channel; frame scheduling and lease policy stay in the shell.

## Use

The macOS-only API exposes `Channel`, `BackingDescriptor`, and `run_service`:

```rust,ignore
let channel = photon_presentation_ipc::Channel::connect(service_name)?;
channel.register_backing(channel_id, descriptor, mach_port)?;
```

This is an internal Photon workspace crate and depends on Apple frameworks. See the [native GPU presentation guide](../../Documentation/NativeGpuPresentation.md) and the [`photon-presentation-broker`](../photon-presentation-broker/README.md) guide.
