# Roadmap

This page lists planned product work. Current behavior and architecture are
documented in the guides linked from the [README](../README.md).

## Next

- Finish onboarding with a choice of workspace preset.
- Add split view for multiple tabs.
- Add folders for pinned tabs.
- Improve tab and toolbar interactions, including copying the current address,
  background audio controls, and spring motion.

## Later

- Add private windows with isolated, temporary profiles.
- Add a bookmark manager and import bookmarks and history from other browsers.
- Add a history page and a download manager.
- Add native file upload pickers and permission prompts through explicit Engine
  and platform APIs.
- Add full-screen pages and video, and show link destinations on hover.
- Add optional local notes and time reports tied to browsing activity.
- Add native ad and tracker blocking with per-site controls and explicit Engine
  APIs.

## Deferred

A full agent Work environment and Chrome extension compatibility are not planned
currently. Any future agent workflow should make progress visible and require
approval for actions that change state. Extension support needs a security
review and explicit Photon Engine APIs.

Engine performance investigations and measurements live in
[Performance](Performance.md) and [Compositor performance](CompositorPerformance.md).
