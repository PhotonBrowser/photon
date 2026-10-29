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
  element in an animation. The slide moves the glass with the content and the
  glass takes the fade, but only the content scales and blurs: resizing
  native glass every frame stutters. For a glass container, prefer a fade, as
  the theme picker uses, with its content sharpening inside.
- **Reuse:** each element's native glass view is kept for its window, so a
  panel that closes and opens again shows the same glass instead of a new
  one that AppKit materializes with a flash. Give each glass a stable id.
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
`z_index` order, highest on top, whatever order GPUI drew it in. Use the named
layers in `glass::layer`, and add a layer there rather than a number at the
call site:

| Layer | For |
| --- | --- |
| `layer::CHROME` | Glass resting on the window, such as the address field |
| `layer::POPOVER` | Panels over the chrome, such as the theme picker |

Pages drawn after glass, such as a link preview over the chrome, cover it
automatically.

## How it works on macOS

Each window has two Photon-owned views beneath GPUI's: one above the pages and
one beneath them (`glass_host_view` in
[`macos/page_layer.rs`](../crates/photon-shell/src/platform/macos/page_layer.rs)).
Every glass view lives in one of them. Glass never goes in the window's own
content view: adding, hiding or removing a view there redraws the whole
window's glass, which flickers. The native view and its placement live in
`AppKitGlassBackdrop` in
[`macos/appkit.rs`](../crates/photon-shell/src/platform/macos/appkit.rs).

Native AppKit controls that need no GPUI content, such as the site information
popover and context menus, use AppKit directly instead; see
[Omnibox](Omnibox.md#site-information).
