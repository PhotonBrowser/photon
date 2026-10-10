# Tab layouts and the sidebar

Settings → Appearance → Layout chooses where the tabs go:

- **Horizontal** (the default): a tab strip in the titlebar, with the
  toolbar (back, forward, reload, the address field and the browser menu)
  below it.
- **Vertical**: a sidebar on the left of the window, with the page framed to
  its right.

Both sit directly on the window's translucent background, so they follow the
theme, appearance and transparency settings like the rest of the window. The
change applies at once, in every window.

## The sidebar, from top to bottom

| Part | What it does |
| --- | --- |
| Top row | Room for the native window controls, the sidebar toggle, and back, forward and reload or stop. Empty space moves the window; right-clicking it opens the browser menu. |
| Address field | The [omnibox](Omnibox.md), larger than in the toolbar: the page's address the short way while idle, the full address while editing, and suggestions in a panel that reaches over the page. |
| Favourites | A three-column grid of favourite sites with their icons. A click switches to a tab already showing the site, or opens it in a new tab. Right-click to remove; add one from a tab's menu ("Add to Favourites"). Up to 12. |
| Tabs | "+ New Tab", which opens the [command bar](Omnibox.md#command-bar), then one row per tab: icon or loading spinner, title, and a close button on the active tab and on the row under the pointer. Click to switch, middle-click or the close button to close, drag to reorder, right-click for the tab menu, and the up and down arrows move between focused tabs. The list scrolls. |
| Footer | Settings and the browser menu, which opens above it. |

⌘S or the sidebar button shows or hides the sidebar. Its top row, with the
window controls and the sidebar button, stays where it is; the rest slides in
from the left edge or back out, and the page narrows or widens beside it in
step, so the page resizes smoothly rather than jumping. Back, forward and
reload fade with the sidebar. Hidden, the top row is a bar as tall as the
horizontal tab strip above the page, which takes the whole width.

- Pointing at the window's left edge slides the sidebar in over the page, on
  a raised surface. It slides away once the pointer has left the edge and the
  sidebar for a moment, unless a menu opened from it is still open.
- Its sidebar button keeps it shown: the page makes room beside it, and the
  surface fades as it does.
- ⌘L shows the sidebar to focus the address field.

The window draws the sidebar as a layer over itself and keeps room for it
beside the page, both following one `Tween` from
[`motion.rs`](../crates/photon-shell/src/platform/ui/motion.rs).
[`window/sidebar_state.rs`](../crates/photon-shell/src/platform/ui/window/sidebar_state.rs)
keeps where the sidebar is and handles the left edge.

## Width

Drag the sidebar's right edge to resize it, between 200 and 420 pixels.
Dragging it narrower than 140 pixels snaps it shut, and dragging back out
while still holding shows it again. The width is saved in the profile once
the drag rests and applies to every window. The limits and the snap are in
`SidebarResize` in [`photon-core/src/sidebar.rs`](../crates/photon-core/src/sidebar.rs).

## Closing the last tab

Closing the last tab closes the window, unless Settings → Tabs says to keep
it open; then a fresh new tab page takes the closed tab's place, and ⌘⇧T
still reopens what was closed.

## Where it lives

- [`ui/tabs/`](../crates/photon-shell/src/platform/ui/tabs/mod.rs): what both
  layouts share (a tab's icon, title, close and audio buttons, dragging) in
  `mod.rs`, the horizontal strip in `strip.rs` and the sidebar's list in
  `list.rs`.
- [`ui/toolbar.rs`](../crates/photon-shell/src/platform/ui/toolbar.rs): the
  horizontal layout's toolbar.
- [`ui/sidebar/`](../crates/photon-shell/src/platform/ui/sidebar/mod.rs): the
  sidebar's views: `navigation.rs`, `favourites.rs`, `footer.rs`, and the
  layout in `mod.rs`.
- [`ui/window/sidebar.rs`](../crates/photon-shell/src/platform/ui/window/sidebar.rs):
  builds the chosen layout from the window's tabs and the settings, and shows
  or hides the sidebar. It is the window's cached chrome view, so Engine
  frames repaint only the page.
- [`photon-core`](../crates/photon-core/src/settings.rs): `TabLayout` and the
  favourites ([`sidebar.rs`](../crates/photon-core/src/sidebar.rs)), saved
  with the settings.
