# Shell theme and style tokens

Photon shell views use shared style values so visual changes have one clear
home and stay consistent across light and dark appearances.

## Colors

[`theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs) defines
`ThemeColors`, the semantic roles used by views. Examples include primary and
secondary text, accent, focused field, tab hover, menu surface, and diagnostics
surface. Views ask `ThemeColors::for_appearance` for the active set instead of
choosing colors locally.

The base light and dark palettes come from GPUI-CE `Colors`. The shell maps
those base values to its semantic roles and adjusts opacity in the local
`opacity` token group. The resulting role sets are cached once per appearance,
so repeated view renders do not rebuild the palette. To tune shell colors,
change the role mapping or a named opacity token in `theme.rs`.

`ThemePreference` follows the system appearance until the user selects Light
or Dark from the browser menu. Selection remains shared by windows and tabs
created from that browser session.

## Dimensions and typography

[`metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) contains
shared logical-pixel measurements for window geometry, tabs, toolbar, menus,
the omnibox, page surface, crash notice, and diagnostics overlay. It also holds
shared type sizes, icon sizes, corner radii, and the debug overlay typeface.

Add a named token there when a view needs a reusable visual measurement. Keep
layout mechanics such as `flex_1`, `size_full`, zero minimum widths, and
GPUI-CE's named border or shadow styles in the view that owns that layout.

## Motion

[`motion.rs`](../crates/photon-shell/src/platform/ui/motion.rs) is the shell's
animation system. An `Entrance` describes how an element appears, combining any
of these effects:

- `fade_from(opacity)`: fades to fully opaque.
- `slide_from(edge, distance)`, or `slide_up`, `slide_down`, `slide_left` and
  `slide_right`: slides into place without moving its siblings.
- `blur_from(radius)`: sharpens from a blur.
- `grow_from(size, fraction)`: grows a fixed-size element, such as an icon.

Timing comes from `speed(Speed::Quick | Standard | Gentle)` (120, 200 and
320 ms) or `duration`, plus `delay` for staggering, and `curve(...)`: `EaseOut`
(the default), `Linear`, `EaseInOut`, `Overshoot`, or a spring such as
`Curve::SNAPPY` and `Curve::BOUNCY`. Distances come from `motion::distance`.

Start from a preset and adjust it:

| Preset | Effect | Used for |
| --- | --- | --- |
| `Entrance::fade()` | Quick fade | Status chips |
| `Entrance::rise()` | Fade while rising | Panels and dialogs |
| `Entrance::fall()` | Fade while falling | The find bar |
| `Entrance::popover()` | Quick fade with a slight drop | Menus, popovers, dropdowns (built into `menu_surface`) |
| `Entrance::slide_in(edge)` | Fade while sliding in from an edge | Sidebars and sheets |
| `Entrance::focus()` | Fade while sharpening from a blur | Content replaced in place |
| `Entrance::pop(size)` | Grow with a little overshoot | Tab icons |

Apply it with `element.animate_in(id, entrance)`. The entrance plays the first
time the element is shown under that id; a new id plays it again. Every
entrance follows the system's reduced-motion setting by appearing at once.

## Editing rules

- Use semantic `ThemeColors` roles; do not put RGB literals in components.
- Keep opacity adjustments named by role in `theme.rs`.
- Put repeated dimensions and type sizes in `metrics.rs`.
- Animate elements in with `motion.rs` entrances rather than hand-written
  animations, so motion stays consistent.
- Give a new token a clear semantic name and reuse it where the same role
  appears.
- Keep genuinely component-specific layout local when it has no shared role.
