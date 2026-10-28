# Shell theme and style tokens

Photon shell views use shared style values so visual changes have one clear
home and stay consistent across light and dark appearances, every
transparency level, and every view.

## Colors

[`theme.toml`](../crates/photon-shell/theme.toml) is the source of color,
opacity, transparency, window tint, and corner-radius values.
[`theme.rs`](../crates/photon-shell/src/platform/ui/theme.rs) resolves these
values into `ThemeColors`, the semantic roles views use. Views get the active set from
`theme::palette(window, cx)`, which reads the appearance and transparency
from the [settings](Profile.md), and never choose colors locally.

| Role | Used for |
| --- | --- |
| `window_tint` | The window background over the desktop |
| `page_background`, `page_surface` | Web content and Photon pages, with a surface tone distinct from the window |
| `surface` | Chrome controls such as the omnibox field, active tab, buttons, text fields and segmented choices |
| `hover_surface` | A row or control under the pointer |
| `selected_surface` | The selected row, an open menu's button, a pressed or hovered button on `surface` |
| `chosen`, `on_chosen` | What is chosen or on (a chosen option, a switch that is on, a checked box), and the text drawn on it. Also focus rings and text carets |
| `menu_surface`, `menu_border` | Raised menus, popovers and dialogs, and the light hairline around raised surfaces and groups |
| `text_primary`, `text_secondary` | Text |
| `suggestion_address` | Addresses in omnibox suggestions (a link blue) |
| `selection`, `field_error_border`, `modal_backdrop` | Selected text, an unopenable address, the dim behind a modal |

The light and dark base palettes, accent colors, error colors, and the colors
drawn on accents are in `theme.toml`. Named opacity tokens adjust derived
roles such as surfaces, borders, secondary text, and hover states. Palettes are
cached per appearance, transparency, and window color, so renders do not rebuild
them.

### Window colour

Each [space](Sidebar.md#spaces) tints the window, as in Arc and Zen: the
system grey, or blue, purple, pink, red, orange, yellow, green or teal,
optionally as a gradient into the neighbouring hue from the top left, chosen
in Settings → Appearance or the space's dialog with the shared
`color_picker`. Hue, tint saturation, and lightness values are in the
`[window]` section of `theme.toml`; the same values tone swatches and light or
dark window tints. The system tint uses the `window` color in each appearance's
palette, while `page` sets the separate web content and Photon page color. This
keeps solid themes distinct without requiring a gradient. Draw the window's
background with `palette.window_background()`, which is the tint or the
gradient; `window_color_swatch` gives the colour each swatch shows.

### Transparency

The transparency setting (off, subtle, clear) decides how much of the desktop
shows through, and how much floating surfaces blur what is behind them, set
in `[transparency]`:

| Transparency | Window | Page | Menus and dialogs | Page/control blur | Raised-surface blur |
| --- | --- | --- | --- | --- | --- |
| Off | solid | 36% | solid | 8 px | none |
| Subtle (default) | 88% | 30% | 92% | 16 px | 20 px |
| Clear | 46% | 24% | 84% | 24 px | 48 px |

`surface`, `hover_surface` and `selected_surface` are tints of the text color
over the window, not layers of their own, so everything on the window is
exactly as see-through as the window. Menus and dialogs float over pages, so
they are more opaque and frost the page behind them to stay readable. The
theme-controlled page/control blur also frosts the backdrop under translucent
Photon pages, controls and shortcut icon wells while leaving their contents
sharp.

## Dimensions and typography

[`metrics.rs`](../crates/photon-shell/src/platform/ui/metrics.rs) holds shared
logical-pixel measurements: window geometry, the tab strip, toolbar, sidebar
and its tabs, menus, the omnibox (with a larger size in the sidebar) and its
suggestions, the page frame, chips, controls, settings rows and new tab tiles.
It also holds type and icon sizes. Corner radii live in the `[radii]` section
of `theme.toml`.

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

Floating surfaces take their background with `element.raised(palette)`
rather than setting it themselves. Raised surfaces are fully opaque across
all window transparency settings and do not blur the content behind them.
Menus and popovers, the omnibox panel, the find bar, chips, dialogs, the
revealed sidebar and a dragged tab all use the same surface role.

## Shared components

- [`menu.rs`](../crates/photon-shell/src/platform/ui/menu.rs): the popover
  surface and menu rows. Menus are compact like Chromium's: full-width flat
  rows with no gaps, padding only above and below, edge-to-edge separators and
  a light border. `popover_surface` takes any width; `menu_surface` is the
  standard menu. Rows include actions, checkboxes, switches, a stepper, small
  headings and blocks that line up embedded controls.
- [`controls.rs`](../crates/photon-shell/src/platform/ui/controls.rs):
  segmented `choices`, colour `swatches`,
  `switch` and `check_mark`, used by menus and pages, `text_field` for a
  dialog's text field with its text centred, and
  `themed_text_input` for every text field's caret and selection.
- [`dropdown.rs`](../crates/photon-shell/src/platform/ui/dropdown.rs): a
  button showing the chosen option that opens a menu of the options, which
  animates in and back out like any menu.
- [`button.rs`](../crates/photon-shell/src/platform/ui/button.rs): text and
  icon buttons, and the toolbar and menu buttons both tab layouts share. [`modal.rs`](../crates/photon-shell/src/platform/ui/modal.rs):
  a centered modal over a dimmed window.
- Page layout and settings rows live in [`pages/`](Pages.md).

## Icons

[`icons.rs`](../crates/photon-shell/src/platform/ui/icons.rs) gives each icon
a function taking a color and a size, such as `back_icon(color, size)`, so
views never handle SVG data. Icons are [Lucide](https://lucide.dev) (ISC
license), vendored unchanged from `lucide-static` 0.460.0 in
[`assets/icons/lucide/`](../crates/photon-shell/assets/icons/lucide) with
its license. Only the loading spinner, whose spokes fade behind the leading
one, and the brand logo are drawn by Photon.

To add an icon, download it from the same `lucide-static` version into that
folder, then add a function that loads it with `lucide!("name")`.

## Motion

[`gpui-motion`](../crates/gpui-motion/README.md) provides reusable animation
mechanics. The shell's
[`motion/`](../crates/photon-shell/src/platform/ui/motion/) keeps Photon
visual presets and distances. An `Entrance` describes an element effect,
composed from:

- `fade_from(opacity)`: fades to fully opaque.
- `slide_from(edge, distance)`, or `slide_up`, `slide_down`, `slide_left` and
  `slide_right`: slides into place without moving its siblings.
- `blur_from(radius)`: sharpens from a blur.
- `grow_from(size, fraction)`: grows a fixed-size element, such as an icon.

Timing comes from `speed(Speed::Quick | Standard | Gentle)` or `duration`, plus
`delay` for staggering, and `curve(...)`: `EaseOut` (the default), `Linear`,
`EaseInOut`, `Overshoot`, or a spring such as `Curve::SNAPPY` and
`Curve::BOUNCY`. The default timing tiers are 120, 200 and 320 ms. Distances
come from the shell's `motion::distance` values.

Press feedback is opt-in at the control call site: `.pressable()` adds a
spring scale of 0.97, and `.icon_pressable()` uses 0.92. Both use the fork's
native gesture and scale adapter, so the child keeps its click, keyboard,
focus, accessibility, and reduced-motion behavior. Slider stretch and switch
thumb motion also use the fork's retained `MotionElement` state.

| Preset | Effect | Used for |
| --- | --- | --- |
| Shell `fade()` | Quick fade | Status chips, settings sections, checkmarks |
| Shell `rise()` | Fade while rising | Panels and dialogs |
| Shell `fall()` | Fade while falling | The find bar |
| Shell `popover()` | Quick fade with a slight drop | Menus and popovers (`MENU_MOTION`) |
| Shell `slide_in(edge)` | Fade while sliding in from an edge | Sidebars and sheets |
| Shell `pop(size)` | Grow with a little overshoot | Tab icons |

Apply it with `element.animate_in(id, entrance, &motion_config)`. It plays the
first time the element is shown under that id; a new id plays it again.

Elements also leave the way they came. `animate_out` plays an entrance
backwards, quicker and accelerating away. `animate(id, entrance, transition,
&motion_config)` picks the direction. A removed element is gone at once, so
keep it drawn while it leaves with `Presence`: each render, pass the owner's
current value to `presence.sync(value, entrance, &motion_config, cx)` and draw
what it returns, the current value entering or the last one leaving. Menus,
chips, the find bar, dialogs and the customise menu work this way.

Layout that animates as a whole, such as the sidebar opening while the page
narrows beside it, uses a `Tween`: a value from 0 to 1 that eases in and out
toward its target and can turn back midway without jumping. Its owner reads
it as it draws and asks for another frame while it runs. When several parts of
the window follow the same motion, sample it with `value_at` at one instant per
frame, as the sidebar does, rather than `value`, which reads the clock on every
call and lets the parts drift apart.

Photon's motion leans on springs, as Arc's does. A spring can be turned back
midway and keeps its momentum, where a fixed-length curve restarts from rest.
Menus and popovers (`popover`) and the find bar spring in on `Curve::SNAPPY`,
with a slight scale, drop and blur. Toasts, peeks and spaces arriving use the
softer `Curve::SMOOTH`. Springs leave on an ease-out of the entrance's exit
duration, so `Presence` can unmount them on time. The sidebar opens, closes,
collapses and tucks away for Zen mode on `SIDEBAR_LAYOUT_SPRING`, which is
critically damped so the page beside it never overshoots. The sidebar revealed
over the page uses the quicker, slightly livelier `SIDEBAR_REVEAL_SPRING`. A
spring ignores the quick, standard and gentle durations, but still jumps
straight to rest when motion is off or reduced.

Surfaces fade between their resting, hovered, pressed and selected colours
rather than switching at once: call `.surface_fade()` (from `SurfaceFade` in
`motion/press.rs`) before `.hover_style(...)`. Buttons, icon buttons, controls,
tab rows, the tab strip and the sidebar footer use it. Colour fades stay on
under reduced motion, since nothing moves.

The shell adapts its persisted animation preferences and the current
system reduced-motion preference into `gpui_motion::MotionConfig`. When motion
is unavailable, effects use their final values and `Presence` releases exiting
items immediately. `Tween` can also sample against the current config so a
running layout value settles after a preference change.

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
  views. Edit theme values in `theme.toml`, and keep their appearance logic in
  `theme.rs`.
- Keep opacity and corner-radius values in `theme.toml`.
- Put repeated dimensions and type sizes in `metrics.rs`, shadows behind
  `Elevation`, and floating backgrounds behind `raised`.
- Build menus and popovers from `menu.rs`, and controls from `controls.rs`,
  so hover, selected, chosen and focus look the same everywhere.
- Animate with `motion/`, and only on a change.
- Take the browser's name, page scheme and logo from `photon-brand`, never
  literals (see [Profile and branding](Profile.md)).
