# Profile, settings, history and branding

## Profile data and settings

[`photon-storage`](../crates/photon-storage/src/lib.rs) owns the profile: one
folder holding everything Photon keeps between launches.

| File or folder | What it is |
| --- | --- |
| `config.toml` | The browser settings |
| `History.json` | Visited pages and past searches, versioned |
| `Session.json` | Windows and tabs restored at launch |
| `Favicons/` | The last icon each visited page showed |
| `Engine/` | The Engine's cookies, site storage and cache |

The default profile is in the platform's application data folder, under the
brand's folder name (`~/Library/Application Support/Photon/Default` on macOS).
By default, `config.toml` is in the platform's configuration folder; when
`PHOTON_PROFILE_DIR` selects an isolated profile, settings are stored there.
`PHOTON_PROFILE_DIR` chooses another folder, and `PHOTON_TEMPORARY_PROFILE`
runs a throwaway session that saves nothing.

Files are written atomically, so a crash never leaves one half-written. A file
this build cannot read is treated as empty. The crate is plain file access
over core types, with no GPUI or Engine dependency, and has tests for each
file.

Profile input is bounded before decoding. Settings files are limited to 1 MiB,
history files to 64 MiB, and session files to 32 MiB. Settings also limit nested
structure to 32 levels, strings to 8 KiB, keys to 256 bytes, spaces to 128,
workspace presets to 32, layout components to 64, and general lists to 4,096
items. Each space keeps at most 256 pinned tabs and 12 favourites; the new tab
page keeps 12 pins and 2,048 hidden sites, and legacy sidebar favourites are
limited to 256.
History follows the core limits of 10,000 pages and 1,000 searches; page URLs
and titles are limited to 4 KiB, and search queries to 2 KiB. Sessions allow up
to 64 windows, 256 tabs per window, and 2,048 tabs total; tab URLs are limited
to 8 KiB and titles to 4 KiB. Over-limit files are rejected without modifying
the source file.

## Settings

`BrowserSettings` in [`photon-core`](../crates/photon-core/src/settings.rs)
holds the theme, transparency, named workspace presets, search engine, pop-up
policy, the new tab page's layout, the sidebar width, the spaces (each with
its name, preset assignment, window colour, pinned tabs and favourites) and
the one shown, whether closing the last tab closes the window, and whether
unused tabs close. `animations` configures all shell entrances, exits and
sidebar tweens: enable or disable motion, set quick, standard and gentle
durations, scale slide, blur and icon growth effects, and tune native traffic
light fade-in and fade-out durations, plus the loading spinner's delay and
cadence. For example, add this section to `config.toml`:

```toml
[animations]
enabled = true
quick_duration_ms = 120
standard_duration_ms = 200
gentle_duration_ms = 320
effect_strength_percent = 100
traffic_light_fade_in_ms = 320
traffic_light_fade_out_ms = 320
loading_spinner_delay_ms = 150
spinner_step_interval_ms = 80
```

Fields have serde defaults, so a profile saved by an older build still loads.
When a profile opens, `photon-storage` runs pending migrations in order and
records the completed version in `.photon-profile-version`. The first migration
converts old `config.json` or `Settings.json` settings to TOML and keeps the
source as a timestamped backup. Future profile migrations are added as the next
ordered step. History and session data remain JSON files.
Profile-wide favourites from older settings migrate into the active space.
`reset_preferences` returns everything to its default but the shortcuts and
spaces, whose favourites and pinned tabs stay while their colours and
dismissed empty-favourites hints reset.

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
