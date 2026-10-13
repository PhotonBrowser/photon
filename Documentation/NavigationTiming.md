# Navigation timing

Set `PHOTON_NAVIGATION_PROFILE=1` to emit one timing summary for each
navigation whose first page surface reaches GPUI's Metal command buffer. The
profiler is disabled by default.

Each summary includes:

- `start_to_commit_ms`: navigation start to top-level document commit.
- `commit_to_engine_frame_ms`: commit to the first ready page frame.
- `engine_frame_to_surface_ms`: the first ready frame to its surface being
  installed in Photon.
- `surface_to_gpui_render_ms`: surface installation to the first GPUI render
  tree that includes the page surface.
- `gpui_render_to_gpu_present_complete_ms`: GPUI render to completion of the
  Metal command buffer that samples the page surface and queues the drawable
  for presentation.
- `start_to_gpu_present_complete_ms`: navigation start to that command buffer
  completion.
- `load_finished_ms`: navigation start to the Engine load-finished callback,
  when it has arrived by the time the first page surface completes.

The command-buffer completion is a GPU presentation milestone, not the
display's physical scan-out time. GPUI-CE does not currently expose a callback
for scan-out. Individual milestone lines remain available if load completion
arrives after the summary.

The start-to-commit interval distinguishes work before renderer commit from
page rendering and shell presentation after commit. The current embedder
callbacks do not expose network response receipt, `DOMContentLoaded`, or the
browser `load` event as separate milestones. `load_finished` is the Engine's
navigation completion callback and may also run when a load is cancelled.
