# Roadmap

What the Photon shell does today and what comes next. Engine performance work is tracked separately in [Performance](Performance.md) and [Compositor frame rate](CompositorPerformance.md).

## Done

### Window and page

- [x] Native macOS window with opaque, blurred or Liquid Glass background (`PHOTON_WINDOW_BACKGROUND`)
- [x] Engine pages drawn from shared IOSurfaces through Metal, paced to the window's display and refresh rate
- [x] Engine stops rendering while the window is occluded or the tab is hidden
- [x] Keyboard, pointer, scroll and cursor forwarding to the page; page focus follows window focus
- [x] Crash recovery: a crashed page process restarts and offers to reload the page
- [x] JavaScript `alert`, `confirm` and `prompt` as a window modal; dialogs are dismissed and held off while the browser navigates away
- [x] Pages follow the shell theme through `prefers-color-scheme`
- [x] Multiple windows

### Tabs

- [x] Tab strip with new, close and select; arrow keys move between tabs
- [x] Loading spinner after a load has run for 150 ms
- [x] Page favicons from the Engine, a globe for pages without one, and the Photon logo for new tabs
- [x] Icons scale in once when a new icon appears
- [x] Reopen closed tabs (last 25 per window)

### Toolbar and omnibox

- [x] Back, forward, reload and stop buttons
- [x] Omnibox: address-or-search resolution, search engine URLs, unopenable addresses flagged in place
- [x] Browser menu: new tab, new window, debug overlay, and a system, light or dark theme
- [x] Titlebar right-click menu
- [x] Performance diagnostics overlay

### Keyboard shortcuts

- [x] ⌘T, ⌘N, ⌘W, ⌘⇧T
- [x] ⌃Tab / ⌃⇧Tab, ⌘⇧] / ⌘⇧[, ⌘1–⌘8, ⌘9
- [x] ⌘R, ⌘., ⌘[, ⌘], ⌘L

### Accessibility and motion

- [x] Accessibility roles and labels on tabs, buttons, menus, the omnibox and alerts
- [x] System reduced-motion preference passed to the Engine and to shell animations

## Next

In order of priority.

- [ ] Find in page (⌘F)
- [ ] Tab strip interactions: middle-click to close, right-click menu (close others, duplicate), drag to reorder
- [ ] Omnibox suggestions: "Search for" and "Go to" rows while typing
- [ ] Page right-click menus
- [ ] Page zoom (⌘+, ⌘−, ⌘0)
- [ ] Profile slow Skia flushes on the first paint of heavy pages ([section 9](CompositorPerformance.md#9-slow-first-paint-on-heavy-pages--open))
- [ ] Draw the page area without re-rendering the whole window on every Engine frame

## Later

- [ ] History store, then history suggestions in the omnibox
- [ ] Bookmarks
- [ ] Downloads
- [ ] File upload pickers and permission prompts
- [ ] Hover link status
- [ ] Session restore
- [ ] Settings page, including the default search engine
- [ ] Private windows
- [ ] Full-screen pages and video
