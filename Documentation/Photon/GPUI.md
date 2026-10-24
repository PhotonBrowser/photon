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
[`platform/macos/window_settings.rs`](../../crates/photon-shell/src/platform/macos/window_settings.rs);
the generic AppKit integration stays in the GPUI-CE submodule.

Photon's AppKit extension in `photon-shell` also hosts retained `NSView`s as
GPUI layout elements. It owns frame synchronization and view attachment
lifetime; AppKit continues to draw and handle input for each native control.
The macOS 26+ button style uses AppKit's Liquid Glass bezel, with a native
toolbar-button fallback on older macOS. See
[`NativeAppKit.md`](NativeAppKit.md) for the API and native-view composition
limits.

Photon owns XPC presentation, frame ordering, generations, retirement, release
forwarding, Engine thread affinity, browser input, and application startup.
On macOS, `photon run` uses GPUI hot reloading through Dioxus by default. Pass
`--no-hot-reload` to use a direct plain-Cargo build. Set
`PHOTON_USE_RA_JIT=1` to opt direct app builds into ra-jit; it remains opt-in
because the current macOS ra-jit build exited with `SIGBUS` during launch.
Hot-reload launches from the same development app bundle as the direct path,
so the Dock icon, app identity, and browser link metadata are available there
too. If a build fails or the patched app crashes, `photon run` restarts Dioxus
with a short backoff and retries until a build launches successfully.

GPUI-CE's experimental Subsecond hot patching launches the app with Dioxus CLI
0.7.10 and patches GPUI render code in the existing process; it does not cover
every Rust code path. Changes to indirect workspace dependencies or startup
code still need a rebuild or restart. Hot reload reuses the selected existing
Engine build and does not build Engine; run `photon build` first if it is
missing or out of date. Use `photon run --ui` to run without requiring Engine.
