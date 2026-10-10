# Sidebar

Photon's tabs live in a vertical sidebar on the left of the window, with the
page framed to its right. The sidebar sits directly on the window's
translucent background, so it follows the theme, appearance and
transparency settings like the rest of the window.

## From top to bottom

| Part | What it does |
| --- | --- |
| Top row | Room for the native window controls, the sidebar toggle, and back, forward and reload or stop. Empty space moves the window; right-clicking it opens the browser menu. |
| Address field | The [omnibox](Omnibox.md): the page's address the short way while idle, the full address while editing, and suggestions in a panel that reaches over the page. |
| Favourites | A three-column grid of favourite sites with their icons. A click switches to a tab already showing the site, or opens it in a new tab. Right-click to remove; add one from a tab's menu ("Add to Favourites"). Up to 12. |
| Space | The current space's icon and name, over its tabs. |
| Tabs | "+ New Tab", then one row per tab: icon or loading spinner, title, and a close button on the active tab and on the row under the pointer. Click to switch, middle-click or the close button to close, drag to reorder, right-click for the tab menu, and the up and down arrows move between focused tabs. The list scrolls. |
| Footer | Settings, the current space, and the browser menu, which opens above it. |

⌘S shows or hides the sidebar. Hidden, a slim bar keeps room for the window
controls and the toggle, and the page takes the whole width; ⌘L shows the
sidebar again to focus the address field. The sidebar slides in when shown.

## Where it lives

- [`ui/sidebar/`](../crates/photon-shell/src/platform/ui/sidebar/mod.rs): the
  views: `navigation.rs`, `favourites.rs`, `tabs.rs`, `footer.rs`, and the
  layout in `mod.rs`.
- [`ui/window/sidebar.rs`](../crates/photon-shell/src/platform/ui/window/sidebar.rs):
  builds the sidebar from the window's tabs and the settings, and shows or
  hides it. The sidebar is the window's cached chrome view, so Engine frames
  repaint only the page.
- [`photon-core/src/sidebar.rs`](../crates/photon-core/src/sidebar.rs): the
  favourites and spaces, saved with the settings.

## Spaces

There is one space, "Personal", for now. `SidebarSettings` keeps a list of
spaces and the active one, so switching spaces and giving each its own tabs
can be added without changing what is saved.
