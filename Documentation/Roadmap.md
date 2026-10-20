# Roadmap

What the Photon shell does today and what comes next. Engine performance work is tracked separately in [Performance](Performance.md) and [Compositor frame rate](CompositorPerformance.md).

## Done

### Window and page

- [x] Native macOS window with opaque, blurred or Liquid Glass background (`PHOTON_WINDOW_BACKGROUND`)
- [x] Native macOS traffic lights use their disabled idle appearance while windowed and remain interactive in fullscreen
- [x] In native and borderless fullscreen the shell uses the space the hidden window controls leave
- [x] Engine pages drawn from shared IOSurfaces through Metal, paced to the window's display and refresh rate
- [x] Engine frames update the page without rebuilding cached browser chrome or overlays
- [x] Engine stops rendering while the window is occluded or the tab is hidden
- [x] Heavy pages no longer freeze scrolling while they first paint: the GPU programs they need are compiled at startup ([section 9](CompositorPerformance.md#9-slow-first-paint-on-heavy-pages--fixed))
- [x] Keyboard, pointer, scroll and cursor forwarding to the page; page focus follows window focus
- [x] Swipe back and forward with two fingers, as in Safari: once a sideways scroll reaches the page's edge the page slides away with an arrow, and lets go to go through the tab's history or settle back; pages can opt out with CSS `overscroll-behavior`. Mouse back and forward buttons go through history too
- [x] Crash recovery: a crashed page reloads in a fresh process, a page that keeps crashing offers a Reload, and Engine service restarts are announced, all in a chip at the bottom right
- [x] JavaScript `alert`, `confirm` and `prompt` as a window modal; dialogs are dismissed and held off while the browser navigates away
- [x] Pages follow the shell theme through `prefers-color-scheme`
- [x] Multiple windows
- [x] Shell icons from Lucide, vendored at a pinned version

### Tabs and sidebar

- [x] Horizontal tabs in the titlebar (the default) or a vertical sidebar, chosen in Settings → Appearance
- [x] Vertical tab sidebar: navigation, a larger address field, favourite sites, tabs and a footer, shown or hidden with ⌘S
- [x] The address bar shows only the page's site, as in Arc, and the full address once clicked; Settings → Search can show the full address instead
- [x] The sidebar slides in and out while the page resizes beside it in step; hidden, pointing at the window's left edge reveals it over the page until the pointer leaves
- [x] Drag the sidebar's edge to widen it (remembered in the profile); narrower than its default hides it
- [x] Pinned tabs above a divider, as in Arc: pinned from the tab menu or by dragging, saved and reopened at launch, icons only in the horizontal strip
- [x] Optionally close today's tabs left unused for 12 hours
- [x] Spaces, as in Arc: separate sets of tabs, each with its own name, window colour and pinned tabs, switched by the footer's dots, swiping or the keyboard, and created, edited or deleted from the footer
- [x] Favourite sites: up to 12 in a grid, added from a tab's menu; as in Arc, a favourite's tab lives in its tile, not the tab list
- [x] Tabs with new, close and select; up and down arrows move between tabs
- [x] Optionally keep the window open on a new tab page when its last tab closes
- [x] Loading spinner after a load has run for 150 ms
- [x] Page favicons from the Engine, a globe for pages without one, the brand logo for new tabs, and a symbol for each of the browser's own pages
- [x] Icons scale in once when a new icon appears
- [x] Reopen closed tabs (last 25 per window)
- [x] Restore open windows and tabs from the profile at launch, with a Settings toggle
- [x] Peek, as in Arc: Shift-click a link to preview it over the page, then close it (Escape or a click outside) or keep it as a tab without reloading
- [x] Middle-click to close, drag to reorder, and a right-click menu: reload, duplicate, close, close others, close to the right

### Toolbar and omnibox

- [x] Back, forward, reload and stop buttons
- [x] Omnibox: address-or-search resolution, search engine URLs, unopenable addresses flagged in place
- [x] Command bar on ⌘T, as in Arc and Zen: open an address or search in a new tab, switch to an open tab, or run a command, on frosted glass over the page
- [x] Omnibox opens into a suggestion panel while typing: visited pages with their icons, past searches, the typed address and a search, with the best visited address completed inline
- [x] History of visited pages and past searches, saved in the profile with page icons; rows can be removed from the omnibox
- [x] Persistent profile for cookies, site storage and the cache (`PHOTON_TEMPORARY_PROFILE` for a throwaway one)
- [x] Browser menu in the toolbar or sidebar footer: new tab, new window, settings, find in page, zoom and the debug overlay
- [x] Titlebar right-click menu
- [x] Performance diagnostics overlay
- [x] Page zoom from the menu or ⌘+, ⌘−, ⌘0, shown in a chip with Reset; each site's zoom is remembered for the session
- [x] Page right-click menus from the Engine for pages, links, selections, images and media, with Copy, Cut and Paste through the macOS pasteboard
- [x] Find in page: a floating bar in the page's corner with match count and highlights (⌘F, ⌘G, ⌘⇧G)

### Internal pages and settings

- [x] New tab page: logo and 4–12 site shortcuts in up to two rows (pinned, then most visited, one per site), with tiles removed on hover, shortcuts added by name and address, and a customise panel
- [x] Settings page with a sidebar of sections: appearance (theme, transparency, tab layout), search, new tab page, tabs, sites (pop-ups), and privacy (clearing browsing data by time range, with disk usage)
- [x] Settings saved in the profile and followed by every window at once
- [x] Reset to defaults, after asking, keeping favourites, shortcuts and pinned tabs
- [x] Window colour, as in Arc and Zen: the system grey or one of eight colours, optionally as a gradient, chosen with swatches in Appearance
- [x] Transparency setting (off, subtle, clear) applied to every surface: window, tabs, omnibox, buttons, menus and pages; floating surfaces frost what is behind them, more as transparency rises
- [x] The browser's own pages live at `photon://` addresses, shown in the omnibox (the new tab page leaves it empty) and listed in one registry

### Keyboard shortcuts

- [x] ⌘T (new tab), ⌘N, ⌘W, ⌘⇧T
- [x] ⌃Tab / ⌃⇧Tab, ⌘⇧] / ⌘⇧[, ⌘1–⌘8, ⌘9
- [x] ⌘R, ⌘., ⌘[, ⌘], ⌘L
- [x] ⌘F, ⌘G, ⌘⇧G
- [x] ⌘+, ⌘−, ⌘0
- [x] ⌘S shows or hides the sidebar
- [x] ⌘⌥← and ⌘⌥→ switch spaces

### Accessibility and motion

- [x] Accessibility roles and labels on tabs, buttons, menus, the omnibox and alerts
- [x] System reduced-motion preference passed to the Engine and to shell animations
- [x] Menus, chips, the find bar and JavaScript dialogs animate in and back out; switches, checkboxes and settings sections animate only when they change

### Branding

- [x] The browser's name, page scheme, profile folder and logo come from one place, `photon-brand`

## Next

In order of priority.

### Customizable browser workspace

See the [customization architecture and delivery plan](Customization.md).

- [x] Extract the horizontal and vertical compositions into reusable shell layout modules while preserving existing behavior and profile settings
- [x] Add named presets for horizontal or vertical tabs and left or right sidebar placement
- [x] Choose a profile default, assign a preset per space, and keep each space's colour independent
- [x] View, create, edit, and delete presets; choose or create one while creating or editing a space
- [ ] Reuse workspace presets in first-run onboarding

### Arc and Zen feel

- [ ] Split view: two or more tabs side by side
- [ ] Tab folders among pinned tabs
- [ ] Polish: ⌘⇧C to copy the address with a toast, a mini player for background audio, spring motion on tab rows

### Engine

## Later

- [ ] Add native ad and tracker blocking with per-site controls and element hiding; request filtering must use an explicit Photon Engine API, with bounded rule compilation and updates
- [ ] Add private windows backed by isolated, ephemeral Engine profiles; private browsing data must not be written into the regular profile
- [ ] Add a native bookmark manager with folders, search, and links kept separate from pinned tabs and favourites
- [ ] History page
- [ ] Import bookmarks and browsing history from supported browsers, with category selection, preview, validation, and duplicate handling
- [ ] Add a download manager with native destination selection, progress, cancellation, and safe filename handling
- [ ] Add file upload pickers and permission prompts through explicit Engine and native APIs
- [ ] Hover link status
- [ ] Full-screen pages and video
- [ ] Add user-owned Markdown notes linked to the current page, with external file edits reflected in Photon
- [ ] Add opt-in, local-only time reports and focus sessions; count active browsing from state transitions and exclude private windows

### Deferred product directions

A full agent Work environment and Chrome extension compatibility are not current roadmap commitments. Any future agent workflow should keep progress and results visible and require approval for state-changing actions. Extension support needs a dedicated security review and explicit Photon Engine APIs; Zephium's Tauri and system-WebView integration is not directly portable.
