# gpui-motion

Reusable animation mechanics for applications built with GPUI-CE. The crate is
independent of Photon and does not provide a scheduler: it composes on GPUI's
`Animation`, spring, and element-transition APIs.

```rust,ignore
use gpui_motion::{AnimateIn, Entrance, MotionConfig};

let config = MotionConfig::default();
let panel = div().animate_in(
    "settings-panel",
    Entrance::new().fade_from(0.0).slide_up(12.0),
    &config,
);
```

## What it provides

- `Entrance` builders for opacity, relative offsets, blur, growth, scale, timing,
  delay, and easing.
- `AnimateIn` for stable-ID entrance, exit, and offset-preserving element effects.
- `AnimateSpring` and `SpringSpec` as a small wrapper over GPUI-CE springs.
- `PressSpec` for configurable pressed-state scale and spring response.
- `MotionDirection` (also named `Transition`) and the `mix`/`lerp` helpers.
- `Tween` for interruptible scalar values, with explicit timestamps and curves
  for deterministic simulation and tests.
- `Presence<T>` for a single value that remains mounted during its exit. A new
  value replaces the old one immediately; crossfades and multiple exiting
  children are outside this API.
- `MotionConfig` and `MotionTimings` so consumers can connect their own
  preferences and accessibility policy.

Consumers own presets and product behavior. Keep durations, effect strength,
reduced-motion preferences, and visual distances in the application config.
Keep interaction state and layout decisions in the application as well.

## Example gallery

The workspace example demonstrates composed entrance effects, springs,
reversible tweens, and delayed presence without depending on Photon crates:

```sh
cargo run -p gpui-motion --example motion_gallery
```

## GPUI-CE compatibility

This workspace crate builds against GPUI-CE commit
`3e897b57ee75ba01a0a1088063e3634099c01ce0`, the revision pinned by this Photon
checkout. The path dependency in its manifest is for workspace use. Before
using the crate outside this workspace, select a GPUI-CE revision that matches
the consuming application's GPUI API and update the dependency source
accordingly. Moving this crate alone does not guarantee compatibility across
GPUI-CE revisions.
