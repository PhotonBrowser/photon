# Native GPU presentation

**Status: working on macOS.** Pages are shown directly by Core Animation from Engine's IOSurfaces, as in Ladybird's Qt view, with no CPU page-pixel copies and no GPUI drawing of page pixels. On 2026-10-26 an animated test page showed a new frame on 59.5–59.9 of every 60 display refreshes, measured from screen captures, and switching between tabs always showed the selected page.

On macOS, the Photon Engine compositor paints Skia output into BGRA IOSurface-backed frame stores. Photon receives each frame's backing identity, generation, frame ID and producer `MTLSharedEvent` value over its native presentation broker. The Engine and GPUI-CE Metal devices are checked for matching registry IDs.

Photon looks up each backing's IOSurface once and holds only the surface itself. A CoreVideo buffer or a Mach send right would mark the surface in use for as long as it lived, and the compositor reads that mark (`IOSurfaceIsInUse`) to tell when the window server has finished with a surface it displayed; the broker likewise keeps the surface rather than its port.

Each web view owns a `CALayer` placed directly beneath GPUI-CE's window layer. GPUI-CE paints a rounded cutout where the page is (`Window::paint_cutout`), so Core Animation and the window server composite the page, and anything GPUI-CE draws over the page afterwards, such as tooltips, menus, the omnibox panel and dialogs, stays above it. Core Animation cannot wait for the producer's GPU work, so a frame is installed only once its producer `MTLSharedEvent` value has signaled; a Metal shared-event listener on the main queue wakes the view then, without a waiting thread. A hidden tab hides its layer.

When a frame is shown, the frames it replaced are released at once, like Ladybird's view releasing its previous front buffer. A frame that was never shown is reported finished and can be reused immediately. A frame that Core Animation displayed is not reported finished while the window server may still read it: the compositor does not render into it and adds a backing store rather than wait. While the window is fully occluded Photon marks the active view hidden, and Engine stops rendering frames. At shutdown, the application disables new frame publication, releases pending and displayed leases, and checks `received == completed == released` with `outstanding == 0`.

The XPC protocol, Rust client channel, and native Objective-C transport live in
`crates/photon-presentation-ipc`. The small
`crates/photon-presentation-broker` executable starts the service; the CLI
command stages and registers that executable. It transports IOSurface Mach
ports and shared-event handles between the Engine and Photon processes.
`photon run` rebuilds the broker and restarts its service when the executable
changes. GPUI-CE contains only the generic cutout primitive.

`photon run --verbose --shutdown-after-seconds 5` provides a fixed-window trace ending in the shutdown lease accounting. `EXTERNAL_IMAGE_LEASE_TRACE=1` adds the Engine side, including each release's `iosurface_in_use` state.
