# Shell theme and style tokens

Photon shell views use shared style values so visual changes have one clear
home and stay consistent across light and dark appearances, every
transparency level, and every view.

## Colors

[`theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs) defines
`ThemeColors`, the semantic roles views use. Views get the active set from
`theme::palette(window, cx)`, which reads the appearance and transparency
from the [settings](Profile.md), and never choose colors locally.

| Role | Used for |
| --- | --- |
| `window_tint` | The window background over the desktop |
| `surface` | Everything that sits on the window: the browser's own pages, the omnibox field, the active tab, buttons, text fields and segmented choices |
| `hover_surface` | A row or control under the pointer |
| `selected_surface` | The selected row, an open menu's button, a pressed or hovered button on `surface` |
| `chosen`, `on_chosen` | What is chosen or on (a chosen option, a switch that is on, a checked box), and the text drawn on it. Also focus rings and text carets |
| `menu_surface`, `menu_border` | Raised menus, popovers and dialogs, and the light hairline around raised surfaces and groups |
| `text_primary`, `text_secondary`, `text_disabled` | Text |
| `suggestion_address` | Addresses in omnibox suggestions (a link blue) |
| `selection`, `field_error_border`, `modal_backdrop` | Selected text, an unopenable address, the dim behind a modal |

The base light and dark palettes come from GPUI-CE `Colors`. The shell maps
them to its roles, with named opacities in the `opacity` group, the link blue
in `link` and the error red in `error`. Palettes are cached per appearance and
transparency, so renders never rebuild them.

### Transparency

The transparency setting (off, subtle, clear) decides how much of the desktop
shows through, in three layers set in `SurfaceOpacity`:

| Transparency | Window | Controls on it (`surface`) | Menus and dialogs |
| --- | --- | --- | --- |
| Off | solid | solid | solid |
| Subtle (default) | 88% | 80% | 92% |
| Clear | 70% | 60% | 84% |

Menus and dialogs stay the most opaque so they remain readable over a page.
`surface` is the window color tinted by `SURFACE_TINT` of the text color.

## Dimensions and typography

[`metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) holds shared
logical-pixel measurements: window geometry, tabs, toolbar, menus, the omnibox
and its suggestions, the page frame, chips, controls (segments, switches,
checkboxes), settings rows and sidebar, and new tab tiles. It also holds type
sizes, icon sizes and corner radii.

Add a named token there when a view needs a reusable measurement. Layout
mechanics such as `flex_1`, `size_full` and `min_w_0` stay in the view.

## Elevation

[`layout.rs`](../crates/photon-shell/src/platform/ui/layout.rs) names the
shell's shadows. Use `element.elevated(Elevation::…)`, not shadow sizes:

| Level | Used for |
| --- | --- |
| `Low` | Switch knobs and chips |
| `Medium` | The find bar and the active tab |
| `High` | Menus, popovers, the omnibox panel and modals |

## Shared components

- [`menu.rs`](../crates/photon-shell/src/platform/ui/menu.rs): the popover
  surface and menu rows. Menus are compact like Chromium's: full-width flat
  rows with no gaps, padding only above and below, edge-to-edge separators and
  a light border. `popover_surface` takes any width; `menu_surface` is the
  standard menu. Rows include actions, checkboxes, switches, a stepper, small
  headings and blocks that line up embedded controls.
- [`controls.rs`](../crates/photon-shell/src/platform/ui/controls.rs):
  segmented `choices`, `switch` and `check_mark`, used by menus and pages, and
  `themed_text_input` for every text field's caret and selection.
- [`button.rs`](../crates/photon-shell/src/platform/ui/button.rs): text and
  icon buttons. [`modal.rs`](../crates/photon-shell/src/platform/ui/modal.rs):
  a centered modal over a dimmed window.
- Page layout and settings rows live in [`pages/`](Pages.md).

## Motion

[`motion.rs`](../crates/photon-shell/src/platform/ui/motion.rs) is the shell's
animation system. An `Entrance` describes how an element appears, combining:

- `fade_from(opacity)`: fades to fully opaque.
- `slide_from(edge, distance)`, or `slide_up`, `slide_down`, `slide_left` and
  `slide_right`: slides into place without moving its siblings.
- `blur_from(radius)`: sharpens from a blur.
- `grow_from(size, fraction)`: grows a fixed-size element, such as an icon.

Timing comes from `speed(Speed::Quick | Standard | Gentle)` (120, 200 and
320 ms) or `duration`, plus `delay` for staggering, and `curve(...)`: `EaseOut`
(the default), `Linear`, `EaseInOut`, `Overshoot`, or a spring such as
`Curve::SNAPPY` and `Curve::BOUNCY`. Distances come from `motion::distance`.

| Preset | Effect | Used for |
| --- | --- | --- |
| `Entrance::fade()` | Quick fade | Status chips, settings sections, checkmarks |
| `Entrance::rise()` | Fade while rising | Panels and dialogs |
| `Entrance::fall()` | Fade while falling | The find bar |
| `Entrance::popover()` | Quick fade with a slight drop | Menus and popovers (`MENU_MOTION`) |
| `Entrance::slide_in(edge)` | Fade while sliding in from an edge | Sidebars and sheets |
| `Entrance::focus()` | Fade while sharpening from a blur | Content replaced in place |
| `Entrance::pop(size)` | Grow with a little overshoot | Tab icons |

Apply it with `element.animate_in(id, entrance)`. It plays the first time the
element is shown under that id; a new id plays it again.

Elements also leave the way they came. `animate_out` plays an entrance
backwards, quicker and accelerating away, and `animate(id, entrance,
transition)` picks the direction. A removed element is gone at once, so keep
it drawn while it leaves with `Presence`: each render, pass the owner's
current value to `presence.sync(value, entrance, cx)` and draw what it
returns, the current value entering or the last one leaving. Menus, chips,
the find bar, dialogs and the customise menu work this way.

Every animation follows the system's reduced-motion setting.

### Animate changes, not appearances

An element's animation state lives only while it is drawn, so an animation
keyed to a value replays whenever its view reappears, for example on
switching tabs. Animate only what just changed:

- Switches and checkboxes remember their last value with the window's keyed
  state and animate only after it changes.
- Settings sections fade in only right after one is picked from the sidebar.
- Pages and other content that reappear on a tab switch do not animate in.

## Editing rules

- Use semantic `ThemeColors` roles from `theme::palette`; no color literals in
  views.
- Keep opacities named in `theme.rs`, and transparency levels in
  `SurfaceOpacity`.
- Put repeated dimensions and type sizes in `metrics.rs`, and shadows behind
  `Elevation`.
- Build menus and popovers from `menu.rs`, and controls from `controls.rs`,
  so hover, selected, chosen and focus look the same everywhere.
- Animate with `motion.rs`, and only on a change.
- Take the browser's name, page scheme and logo from `photon-brand`, never
  literals (see [Profile and branding](Profile.md)).
