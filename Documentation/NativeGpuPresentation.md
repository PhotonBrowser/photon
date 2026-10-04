# Native GPU presentation

**Status: working on macOS.** Example.com has been displayed through the GPUI-CE Metal and IOSurface path, with GPU command buffers completing and no CPU page-pixel copies. An intermittent blank launch was also observed during development; that startup issue is not considered resolved by this successful render.

On macOS, the Photon Engine compositor paints Skia output into BGRA IOSurface-backed frame stores. Photon receives each frame's backing identity, generation, frame ID and producer `MTLSharedEvent` value over its native presentation broker. The Engine and GPUI-CE Metal devices are checked for matching registry IDs.

Photon resolves and caches the IOSurface once per backing. GPUI-CE caches the imported Metal texture by resource ID, generation and actual IOSurface ID. It encodes the producer event wait in the same command buffer that samples the texture, draws directly to the native layer, and reports both submission and completion to Photon. There are no CPU page-pixel copies or GPUI image uploads in this path.

Photon keeps the displayed Engine backing leased until a replacement frame's GPU completion makes the old backing reusable. At shutdown, the application disables new frame publication, waits for all tracked surface-sampling command buffers, releases pending and displayed leases, and asserts `submitted == completed == released` with `outstanding == 0`.

The Photon-owned XPC service is implemented in `native/presentation/PhotonPresentationXpc.m` and launched by `crates/photon-cli/src/commands/presentation_broker.rs`. It transports IOSurface Mach ports and shared-event handles between the Engine and Photon processes. GPUI-CE itself contains only generic external Metal surface rendering APIs.

`./photon run --verbose --shutdown-after-seconds 5` provides a fixed-window trace. The latest verified run reported `submitted=4 completed=4 released=4 outstanding=0 gpu-in-flight=0`. The compositor profile reported zero GPU-to-CPU bitmap readback time, and repeated frames reused backing IDs 13–15. AppKit titlebar and window material styling remain separate work after interaction and resize validation.
