<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-performance`

Performance diagnostics for Photon views: thread-safe frame and engine statistics, bounded presentation timing history, process sampling, formatting, charts, and a GPUI overlay.

## Use

Collect a lightweight view snapshot through `PerformanceMonitor`:

```rust
use photon_performance::PerformanceMonitor;

let monitor = PerformanceMonitor::default();
monitor.set_enabled(true);
monitor.mark_input();
monitor.record_frame();
let snapshot = monitor.snapshot();
```

The shell supplies Engine statistics and semantic colors. Keep Engine IPC and native embedder details in [`photon-shell`](../photon-shell/README.md).
