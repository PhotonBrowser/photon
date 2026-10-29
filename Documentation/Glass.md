# Glass

Photon draws Liquid Glass behind GPUI elements with one element, `glass`, in
[`ui/glass.rs`](../crates/photon-shell/src/platform/ui/glass.rs). On macOS 26
and later it puts native glass beneath the element's GPUI children. Everywhere
else, and when **Settings → Appearance → Glass menus** is off, the element is a
plain `div` with its fallback styles.

## Add glass to something

Use `glass(id)` where you would use `div().id(id)`. Style it, fill it and
handle its events as any `div`:

```rust
use super::glass::{glass, layer};

glass("my-panel")
    .z_index(layer::POPOVER)
    .fallback(move |panel| panel.raised(palette).border_1())
    .rounded(px(metrics::surface_radius()))
    .p(px(metrics::MENU_PADDING))
    .child(content)
```

- **Shape:** `.rounded(..)` shapes the glass too.
- **Fill:** `.bg(..)` and `.border_*(..)` draw on top of the glass, as a tint
  and an edge. Leave them off to show the glass alone.
- **Fallback:** `.fallback(|el| ..)` styles apply only where there is no glass,
  for the painted surface that stands in for it.
- **Motion:** use `.motion(entrance, transition)` rather than wrapping the
  element in an animation. The native glass does not inherit GPUI's painted
  fade and scale, so it takes the same motion itself and moves, scales and
  fades with the content; only the blur is the content's alone. To grow a
  panel out of the side it opens on, as a popover does, use
  `Entrance::scale_from_edge(scale, edge, length)` with `Edge::Left`,
  `Right`, `Top` or `Bottom`; the theme picker grows from the sidebar's side.
- **Reuse:** each element's native glass view is kept for its window, so a
  panel that closes and opens again shows the same glass instead of a new
  one that AppKit materializes with a flash. Closed, it waits out of sight at
  full size rather than hidden, so it draws at once when it returns. Give each glass a stable id.
- **Press response:** `.interactive(true)` plays AppKit's press effect, for
  glass that is itself a control, such as the address field. Leave it off for
  panels, or pressing their content moves the glass behind it.
- **On or off:** `.enabled(bool)`. Menus from `popover_surface` are off by
  default; call `.enabled(true)` on one to put it on glass.
- **Checking first:** `glass_available(cx)` says whether glass will draw, for
  content that should change with it, such as text colour.

## Native or frosted

There are two kinds of glass. Pick by what lies behind the element:

| Behind the element | Use | Why |
| --- | --- | --- |
| The window or a web page | Native glass, the default | Real Liquid Glass, macOS 26+ only |
| GPUI content, such as the new tab page or settings | `.frosted(palette)` | Native glass cannot show what GPUI draws: it clears it and shows the window instead |

Frosted glass is GPUI's own backdrop blur under a translucent surface. It works
on every platform, animates as any GPUI element, blur included, and never
flickers. Its blur is `FROSTED_GLASS_BLUR` in `metrics.rs`, and its surface
opacity is `frosted_glass` in `theme.toml`. The new tab page's customise menu
uses it.

## Panels over GPUI content

Glass inside GPUI's window refracts only what lies beneath GPUI: to show, it
clears GPUI's drawing, so it cuts through any GPUI interface behind it. A panel
that floats over GPUI content and must be native glass opens in its own window
instead, with `NativePanel` in
[`ui/native_panel.rs`](../crates/photon-shell/src/platform/ui/native_panel.rs).
The system composites that window over the browser window, so its glass
refracts everything behind it. It takes any GPUI content, moves with the
browser window, closes on Escape or a click elsewhere, and plays the content's
entrance and exit while the window fades. The theme picker opens this way on
macOS; elsewhere it is drawn in the window.

## What lies beneath the glass

Glass refracts what is beneath GPUI, not what GPUI painted beneath the element:
it clears that, so it can show through. Say what the glass rests on:

| Glass rests on | Use | Example |
| --- | --- | --- |
| The window | `.window_tint(palette)` | The address field in the toolbar |
| A GPUI surface over a page | `.backing(Some(rgba))` with that surface's colour | The address field in the sidebar shown over the page |
| The page, as a floating panel | Nothing | The theme picker |

Don't put glass on a control that sits on another GPUI surface without a
backing: it cuts a hole through that surface. This is why buttons are plain
GPUI. Menus, dropdowns and small panels are plain GPUI too; on macOS context
menus are native.

## Change the order

Glass always lies beneath GPUI's own drawing. Overlapping glass is drawn in
`z_index` order, highest on top, set when the glass first appears. Use the named
layers in `glass::layer`, and add a layer there rather than a number at the
call site:

| Layer | For |
| --- | --- |
| `layer::CHROME` | Glass resting on the window, such as the address field |
| `layer::POPOVER` | Panels over the chrome, such as the theme picker |

Pages drawn after glass, such as a link preview over the chrome, cover it
automatically.

## How it works on macOS

Glass lies directly in the window's content view, just beneath GPUI's view,
where it draws while it animates. Its order is its place among those views,
as AppKit draws them: glass of a higher `z_index` is inserted above glass of
a lower one, and glass moves beneath the pages' host only while a page drawn
after it covers it. Adding, hiding or removing a view there redraws the whole
window's glass, which flickers, so each glass view is inserted once and never
hidden or removed while its window lives: glass that should not show waits out
of sight instead. The native view and its placement live in
`AppKitGlassBackdrop` in
[`macos/glass_backdrop.rs`](../crates/photon-shell/src/platform/macos/glass_backdrop.rs).

Native AppKit controls that need no GPUI content, such as the site information
popover and context menus, use AppKit directly instead; see
[Omnibox](Omnibox.md#site-information).
