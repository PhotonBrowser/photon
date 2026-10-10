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

The macOS window API also exposes
`Window::set_traffic_light_hover_behavior`. Photon enables it for browser and
popup windows. While windowed, the native buttons stay enabled at their
AppKit positions. GPUI-CE fades their layers out while idle and draws muted
circles underneath them; a padded tracking area fades in the real buttons
before the pointer reaches them, so AppKit supplies the hover glyphs and
native actions. The overlay does not intercept hit tests or change the
buttons' enabled/highlighted state.

In fullscreen, GPUI-CE restores the native controls and leaves titlebar
reveal, placement, and hover handling to AppKit. Photon-configured placement
is restored after exiting fullscreen. The shell removes titlebar insets
reserved for window controls in native and borderless fullscreen, so its
content can use that space when the controls are hidden. It configures the
behavior in
[`platform/window_settings.rs`](../../crates/photon-shell/src/platform/window_settings.rs);
the generic AppKit integration stays in the GPUI-CE submodule.

Photon owns XPC presentation, frame ordering, generations, retirement, release
forwarding, Engine thread affinity, browser input, and application startup.
`./photon run` is the direct GPUI-CE application route. Runtime proof and
remaining validation gates are recorded in `GPUI-Migration-Audit.md`.
