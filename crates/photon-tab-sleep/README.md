<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-tab-sleep`

Framework-independent tab sleep modes and idle timeout policy. The crate describes when an inactive page becomes eligible for discard; callers decide when to apply that policy.

## Use

```rust
use photon_tab_sleep::{TabSleepMode, idle_timeout};

assert_eq!(
    idle_timeout(TabSleepMode::Balanced),
    Some(std::time::Duration::from_secs(60)),
);
```

`Off` disables idle discard, `Balanced` waits one minute, and `Maximum` waits five seconds. Memory-pressure cleanup policy is represented by `MemoryPressure`.
