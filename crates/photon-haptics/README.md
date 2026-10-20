<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-haptics`

Semantic haptic feedback helpers for GPUI-CE applications. Call sites describe the interaction; this crate maps it to the platform feedback style.

## Use

Call from a GPUI app context:

```rust
use photon_haptics::{Feedback, play};

play(cx, Feedback::StateChange);
```

Available feedback types are `Tap`, `StateChange`, and `Alignment`. Photon’s controls use this crate through [`photon-shell`](../photon-shell/README.md).
