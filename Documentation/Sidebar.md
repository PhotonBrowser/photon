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
| Tabs | "+ New Tab", then one row per tab: icon or loading spinner, title, and a close button on the active tab and on the row under the pointer. Click to switch, middle-click or the close button to close, drag to reorder, right-click for the tab menu, and the up and down arrows move between focused tabs. The list scrolls. |
| Footer | Settings and the browser menu, which opens above it. |

⌘S shows or hides the sidebar. Hidden, its top row stays where it was, in
a bar as tall as the horizontal tab strip, and the page takes the whole width
below it; ⌘L shows the sidebar again to focus the address field. The
sidebar slides in when shown.

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
