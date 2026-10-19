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
| Top row | Room for the native window controls, the sidebar toggle, and back, forward and reload or stop. Empty space moves the window. |
| Address field | The [omnibox](Omnibox.md), larger than in the toolbar: the page's address the short way while idle, the full address while editing, and suggestions in a panel that reaches over the page. |
| Favourites | A three-column grid of this space's favourite sites with their icons, up to 12. When empty, a dismissible drop hint explains how to add one. Drag a tab into the grid or choose "Add to Favourites" from its right-click menu; the tab becomes its tile rather than staying in the tab list. Click a tile to switch to its tab or open it. The tile is highlighted while its tab is in use. Right-click to close its tab or remove the favourite, which returns its tab to the list. |
| Pinned tabs | Tabs pinned from their menu ("Pin Tab"), above a divider. See [Pinned tabs](#pinned-tabs). |
| Tabs | "+ New Tab", which opens the [command bar](Omnibox.md#command-bar), then one row per tab of today's tabs: icon or loading spinner, title, and a close button on the active tab and on the row under the pointer. Click to switch, middle-click or the close button to close, drag to reorder or move to another window's tab list, right-click for the tab menu, and the up and down arrows move between focused tabs. Dropping a tab outside all windows leaves it in place. The list scrolls. |
| Footer | Settings, the spaces as dots with a button to add one, and the browser menu, which opens above it. |

Right-clicking any empty space in the sidebar or titlebar opens the browser's
native macOS menu: new tabs and windows, find, zoom and debug info for a web
page, "Sidebar as Icons", "Zen Mode" and Settings, each with its shortcut.
Controls take their own clicks, so right-clicking one does not open it.

⌘S or the sidebar button shows or hides the sidebar. The whole sidebar,
including its top row with the window controls, the sidebar button, back,
forward and reload, slides in from the left edge or back out, and the page
narrows or widens beside it in step, so it resizes smoothly rather than
jumping. Hidden, nothing sits above the page: the window controls hide too,
and only a thin margin around the page moves the window.

- Pointing at the window's left edge slides the sidebar in over the page, on
  a raised surface, with its labels and the window controls. It slides away
  once the pointer has left the edge and the sidebar for a moment, unless a
  menu opened from it is still open.
- Its sidebar button keeps it shown, with its labels: the page makes room
  beside it, and the surface fades as it does.
- ⌘L shows and expands the sidebar to focus the address field.
- The address field shows only the page's site, such as `github.com`, until
  it is clicked to edit the full address, unless Settings → Search shows the
  full address.

## Collapsed to icons

⇧⌘S, "Sidebar as Icons" in the browser menu, or dragging the sidebar's edge
narrow collapses it to a 56-pixel column of icons: favourites, pinned and
today's tabs as square tiles with "+" for a new tab, and the spaces above the
browser menu. The address field and the favourites hint are left out. The
window controls stack vertically at its top, above the sidebar button, which
expands it again. Labels and icons crossfade while the width animates. The
choice is saved in the profile, so new windows open the same way; hiding and
showing the sidebar keeps it.

## Zen mode

⌥⌘Z, "Zen Mode" in the browser menu or the command bar hides the chrome
around the page: the sidebar, any bars above and below it, and the window
controls. The page keeps its margin, and the margin above it moves the
window. Pointing at the left edge still
reveals the sidebar, and its sidebar button leaves Zen mode and keeps it. The
page's right-click menu offers "Exit Zen Mode" while it is on. Zen mode
belongs to its window and is not saved.

The window draws the sidebar as a layer over itself and keeps room for it
beside the page, each following a `Tween` from
[`motion.rs`](../crates/photon-shell/src/platform/ui/motion.rs).
[`window/side_dock/`](../crates/photon-shell/src/platform/ui/window/side_dock/)
keeps where the sidebar is, handles its edges, draws it, and places the
macOS native window controls through GPUI-CE's `set_traffic_light_axis` and
`set_traffic_lights_visible`. Linux keeps its custom controls in the shared
titlebar's reserved trailing space.

## Width

Drag the sidebar's right edge to widen it, up to 420 pixels. Its default
width, 240 pixels, is the narrowest it goes with labels. Dragging narrower
snaps to whichever is nearer, the default width or the icons, and dragging
almost to the edge hides it; dragging back out while still holding shows it
again. The width is saved in the profile once the drag rests and applies to
every window. The limits and the snaps are in `SidebarResize` in
[`photon-core/src/sidebar.rs`](../crates/photon-core/src/sidebar.rs).

## Pinned tabs

As in Arc, a tab's menu pins it above today's tabs, and unpins it. Pinned
tabs belong to their space, keep the address they were pinned at, and open
again, in the background, in the first window at launch, showing what they
were pinned as until they load; if several windows pin tabs, the one changed
last is what is saved. In the horizontal strip they show only their icons, at
the start.

Dragging a tab among the pinned tabs pins it, and among today's tabs, onto the
divider or onto "+ New Tab" unpins it. Settings → Tabs can close today's tabs
left unused for twelve hours; pinned tabs and the tab in use stay, and closed
tabs can be reopened with ⌘⇧T. The rules are in
[`photon-core/src/tabs.rs`](../crates/photon-core/src/tabs.rs), and the
window's side in
[`window/pinned.rs`](../crates/photon-shell/src/platform/ui/window/pinned.rs).

## Spaces

As in Arc, spaces are separate sets of tabs, each with its own name, window
colour, pinned tabs and favourites. Favourites and their dismissed empty hint
stay with that space. The sidebar shows the tabs and favourites of the space
shown, and new tabs join it.

- Switch with the footer's dots, a two-finger swipe sideways across the
  sidebar, ⌘⌥← and ⌘⌥→, or the command bar. The tabs slide in from the side
  the space lies on, and the window takes its colour. Choosing a tab of
  another space, as from the command bar, switches to that space.
- A space shows its most recent tab, or a new tab page when it has none.
- The footer's + creates a space, in a dialog asking for its name and colour.
  Right-click a dot to edit its space, or delete it and close its tabs; the
  last space stays.
- Settings → Appearance changes the colour of the space shown.

The space shown is shared by every window, as the window colour follows it.
The tab strip of the horizontal layout shows every space's tabs. The model is
in [`photon-core/src/spaces.rs`](../crates/photon-core/src/spaces.rs), the
window's side in
[`window/spaces.rs`](../crates/photon-shell/src/platform/ui/window/spaces.rs),
and the dialog in
[`space_editor.rs`](../crates/photon-shell/src/platform/ui/space_editor.rs).

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
- [`ui/window/chrome.rs`](../crates/photon-shell/src/platform/ui/window/chrome.rs):
  builds the chosen chrome components from the window's tabs and settings.
- [`ui/window/workspace.rs`](../crates/photon-shell/src/platform/ui/window/workspace.rs):
  composes the cached chrome, active page, and window overlays. Engine frames
  repaint only the page.
- [`ui/workspace_layout.rs`](../crates/photon-shell/src/platform/ui/workspace_layout.rs):
  resolves the saved horizontal or vertical layout to the shell's built-in
  composition and component catalog.
- [`photon-core`](../crates/photon-core/src/settings.rs): `TabLayout`, the
  pinned tabs and the ([`sidebar.rs`](../crates/photon-core/src/sidebar.rs)), saved
  with the settings.
