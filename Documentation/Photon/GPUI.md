# Photon GPUI-CE fork

Photon uses its maintained fork at `vendor/gpui-ce/` (`origin`:
`PhotonBrowser/gpui-ce`; `upstream`:
`https://github.com/gpui-ce/gpui-ce`). The root repository pins a tested fork
revision. Sync upstream `main` into the persistent fork `main` branch, validate
the GPUI-CE crates and Photon, push the tested fork commit, then update the
root gitlink. Do not move the pin to an untested upstream tip.

Photon needs a generic macOS `ExternalMetalSurface` for an externally-produced
BGRA IOSurface. Its descriptor includes the logical resource and generation
plus the actual IOSurface ID, which is checked against the supplied
CVPixelBuffer and participates in the Metal texture cache key. An optional
`MetalSharedEventWait` is encoded in the same command buffer that samples the
surface. GPUI-CE's Apple renderer reports GPU completion through a callback so
the embedding application can release its own resource lease on its own
thread. The GPUI API contains no browser, Ladybird, or Photon lifecycle types.

The surface element also carries rounded corner radii through GPUI's shared
surface shader. GPUI-CE exposes submission and GPU-completion callbacks;
Photon uses them to track command buffers and return presentation leases only
after the GPU has finished sampling the surface.

Photon owns XPC presentation, frame ordering, generations, retirement, release
forwarding, Engine thread affinity, browser input, and application startup.
`./photon run` is the direct GPUI-CE application route. Runtime proof and
remaining validation gates are recorded in `GPUI-Migration-Audit.md`.
