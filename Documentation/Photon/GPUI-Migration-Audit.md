# GPUI-CE migration status

## Current implementation

`./photon run` starts the native Rust application on GPUI-CE. The application
uses Photon presentation descriptors to pass Ladybird's IOSurface to GPUI-CE's
generic external Metal surface element. The Photon-specific work remains in the
application and presentation broker; GPUI-CE does not depend on Ladybird or
Photon lifecycle types.

The legacy GPUIX, React/TypeScript UI, Bun host, Photon GPUIX crates, and nested
Zed GPUI checkout have been removed from the active source tree. The C++ bridge
is kept under `native/embedder/` as part of the narrow Ladybird boundary.

## Presentation proof

A real Example Domain page was visibly presented through the direct application
route. Runtime diagnostics confirmed IOSurface resolution, shared-event waits,
texture-cache hits, external-surface drawing, Metal submission and completion,
release forwarding, and backing reuse. Engine compositor profiling reported no
GPU-to-CPU bitmap readback.

A timed shutdown completed with:

```text
submitted=4 completed=4 released=4 outstanding=0 gpu-in-flight=0
```

This is a bounded run's shutdown accounting, not a permanent zero-copy
benchmark. Continue to keep the verbose diagnostics available for longer runs.

## Remaining validation

- `./photon test` passes the Rust workspace. The separate broader Engine CTest
  run previously failed in `RustWorkspace`; that failure is outside this
  presentation gate and remains to be investigated independently.
- Exercise keyboard and pointer input through the native window.
- Verify resize and device-pixel-ratio changes, navigation, and generation
  transitions against a live Ladybird page.
- Repeat shutdown accounting across navigation and resize, and confirm no
  outstanding presentation leases after draining.
- Measure CPU copies, image uploads, and IOSurface imports over a sustained
  run; expected counts are zero CPU full-frame copies, zero GPUI image uploads,
  and no per-frame IOSurface imports after cache warmup.
- Replace the periodic application pump with event-driven invalidation where
  the native integration permits it.

Native titlebar styling, transparency, blur, and WebView insets are deferred
until these runtime gates are complete.
