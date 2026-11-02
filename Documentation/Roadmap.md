# Roadmap

This page lists planned product work. Current behavior and architecture are
documented in the guides linked from the [README](../README.md).

## Next

- Finish onboarding with a choice of workspace preset.
- Add split view for multiple tabs.
- Add folders for pinned tabs.
- Limit the file upload panel to the types a page's file input accepts.
- Clear a site's IndexedDB and its storage on non-default ports from its info
  menu.

## Later

- Add a bookmark manager and import bookmarks and history from other browsers.
- Add optional local notes and time reports tied to browsing activity.
- Add native ad and tracker blocking with per-site controls and explicit Engine
  APIs.
- Define separate stable and canary release channels once Photon can sign and
  publish installable builds with a reliable update path.

## Deferred

Chrome extension compatibility is not planned currently. Extension support needs
a security review and explicit Photon Engine APIs.

Engine performance investigations and measurements live in
[Performance](Performance.md) and [Compositor performance](CompositorPerformance.md).
