# photon-motion

Reusable motion primitives for GPUI applications. This crate has no browser or
native-platform dependencies, so applications can supply their own animation
preferences through `MotionConfig`.

```rust,ignore
use photon_motion::{AnimateIn, Entrance, MotionSettings};

let settings = MotionSettings::default();
let panel = div().animate_in("settings-panel", Entrance::rise(), &settings);
```

The crate also provides `Presence` for values that need to remain visible while
leaving, `Tween` for layout values that move together, shared control
transitions, and `animate_spring` for element-specific spring styling. Set
`MotionSettings::reduced_motion` from the app's accessibility preference;
GPUI's own element animations also honor the platform reduced-motion setting.
