# The browser's own pages

Photon draws some pages itself, natively in GPUI, rather than loading them in
the Engine: the new tab page and settings. Each lives at an address with the
brand's page scheme, such as `photon://settings`, and is a view in
[`ui/pages/`](../crates/photon-shell/src/platform/ui/pages/mod.rs).

## How a tab shows a page

A tab shows either a web view or a page
([`window/content.rs`](../crates/photon-shell/src/platform/ui/window/content.rs)).
Navigating the active tab to a `photon://` address opens that page in it, and
any other address opens a web view, replacing whichever the tab showed. Pages
sit in the same frame as web pages: the same inset and rounded corners, on the
theme's `surface`.

The omnibox shows a page's address, or stays empty on the new tab page so you
can type at once. Pages are not recorded in the history.

## The registry

[`pages/registry.rs`](../crates/photon-shell/src/platform/ui/pages/registry.rs)
lists every page. Each page module declares a `PageDefinition`:

| Field | Meaning |
| --- | --- |
| `name` | The name in its address, as `settings` in `photon://settings` |
| `title` | The tab's title |
| `icon` | `PageIcon::Logo` (the brand logo) or `PageIcon::Symbol(fn)` |
| `shows_address` | Whether the omnibox shows the address |
| `single_tab` | Whether opening it again switches to the tab already showing it |
| `build` | Builds the page's view from a `PageContext` |

A page asks its window for anything beyond itself through its `PageContext`:
`open(url)` loads an address in the page's tab, and `open_page(page)` opens
another page. The window needs no code for a new page.

### Adding a page

1. Add a module under `pages/` with the page's view and a
   `pub(super) const PAGE: PageDefinition`.
2. Add it to `PAGES` in `registry.rs`, with a named constant if other code
   opens it, as with `NEW_TAB` and `SETTINGS`.
3. Build it from the shared parts: [`pages/layout.rs`](../crates/photon-shell/src/platform/ui/pages/layout.rs)
   (scrolling column, headings, labels, groups),
   [`pages/controls.rs`](../crates/photon-shell/src/platform/ui/pages/controls.rs)
   (switch and checkbox rows), and the shell-wide
   [controls and menus](Theme.md#shared-components).
4. Read and change settings with `Settings::get` and `Settings::update`, and
   observe the `Settings` global to redraw when they change elsewhere.

## New tab page

[`pages/new_tab/`](../crates/photon-shell/src/platform/ui/pages/new_tab/mod.rs)
shows the brand logo and shortcuts to sites:

- `tiles.rs`: the shortcut grid. Hovering a tile offers to remove it; an "Add
  shortcut" tile opens `shortcut_form.rs`, which pins a site by name and
  address.
- `customise.rs`: the Customise button in the corner and the menu it opens
  just above it: show or hide the logo and shortcuts, the number of
  shortcuts, restore removed shortcuts, and all settings.
- `options.rs`: what those options do, shared with the settings page, which
  lays them out in its own style.

The rules live in [`photon-core/src/new_tab.rs`](../crates/photon-core/src/new_tab.rs):
pinned sites first, then the most visited sites, one per site, up to the
chosen 4, 6, 8, 10 or 12, in at most two rows. Removing a tile hides its whole
site until removed sites are restored.

## Settings

[`pages/settings/`](../crates/photon-shell/src/platform/ui/pages/settings/mod.rs)
has a sidebar of sections, one shown at a time, each in its own module:

| Section | Settings |
| --- | --- |
| Appearance | Theme (system, light, dark), transparency (off, subtle, clear) and layout: tabs horizontal or vertical |
| Search | The search engine for the omnibox |
| New tab page | The new tab page's options |
| Tabs | Whether closing the last tab closes the window, or leaves it open on a new tab page |
| Sites | Pop-up windows: ask, allow or block |
| Privacy | Clear browsing data: a time range, which kinds (history, searches, cache, cookies and site data) with their sizes on disk, and Clear data |

Changes save at once and apply in every window; see
[Profile and branding](Profile.md).
