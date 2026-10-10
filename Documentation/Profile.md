# Profile, settings, history and branding

## The profile

[`photon-storage`](../crates/photon-storage/src/lib.rs) owns the profile: one
folder holding everything Photon keeps between launches.

| In the profile | What it is |
| --- | --- |
| `Settings.json` | The browser settings |
| `History.json` | Visited pages and past searches, versioned |
| `Favicons/` | The last icon each visited page showed |
| `Engine/` | The Engine's cookies, site storage and cache |

The default profile is in the platform's application data folder, under the
brand's folder name (`~/Library/Application Support/Photon/Default` on macOS).
`PHOTON_PROFILE_DIR` chooses another folder, and `PHOTON_TEMPORARY_PROFILE`
runs a throwaway session that saves nothing.

Files are written atomically, so a crash never leaves one half-written. A file
this build cannot read is treated as empty. The crate is plain file access
over core types, with no GPUI or Engine dependency, and has tests for each
file.

## Settings

`BrowserSettings` in [`photon-core`](../crates/photon-core/src/settings.rs)
holds the theme, transparency, search engine, pop-up policy, the new tab
page's layout, the sidebar's favourites and spaces, and whether closing the
last tab closes the window. Fields have serde defaults, so a profile saved by an older
build still loads.

In the shell, [`ui/settings.rs`](../crates/photon-shell/src/platform/ui/settings.rs)
keeps one app-wide `Settings` global:

- `Settings::get(cx)` reads; `Settings::update(cx, change)` changes, saves at
  once and notifies observers.
- Windows, pages and site windows observe the global, so a change made
  anywhere applies everywhere at once. The app applies the window appearance
  and the Engine's pop-up policy when settings change.

## History

`History` in [`photon-core`](../crates/photon-core/src/history.rs) records
visited web pages (titles, visit counts, last visit) and queries searched for,
trims its oldest entries, lists the most visited pages, gives omnibox
suggestions, and clears what a `ClearBrowsingData` request asks for.

In the shell, [`ui/history.rs`](../crates/photon-shell/src/platform/ui/history.rs)
keeps one app-wide `BrowsingHistory`:

- Tabs record a visit when their address changes, then keep its title and
  icon current, and only report what changed.
- Changes save to the profile a moment later, off the main thread, and once
  more on quit. Icons save as they arrive and load when first drawn.
- `most_visited`, `measure_data_usage` and `clear` serve the new tab and
  settings pages.

## Clearing browsing data

`ClearBrowsingData` says what to delete and from when: history, searches,
cache and site data. `BrowsingHistory::clear` deletes Photon's own data and
asks the Engine to delete the cache and site data through the embedder's
`Runtime::clear_browsing_data`. Settings → Privacy builds the request.

## Branding

[`photon-brand`](../crates/photon-brand/src/lib.rs) holds the browser's name,
the engine's name, the internal page scheme, the profile folder name and the
logo. Everything people see that names the browser reads from it: window
titles, notices, settings text, `photon://` addresses, the profile folder and
the logo on tabs and the new tab page. To rebrand, change it there.

Changing the page scheme or folder name also moves the page addresses and
where profiles are kept. Developer logs and panic messages still say Photon.
