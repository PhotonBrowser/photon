# Linux package staging decision

Package completed outputs in Photon root without changing Engine or GPUI pins.
The first artifact is an experimental relocatable app-directory archive for
Ubuntu 24.04 x86_64. Other Linux feature work is outside this change.

## Ownership

- Rust shell owns startup. On Linux, helpers default to the executable's
  directory and the first positional argument supplies the startup URL when
  `PHOTON_URL` does not provide one. Development overrides remain supported.
- `photon-cli` owns the package command and desktop metadata. `photon install`
  continues to install only the developer CLI.
- A root-owned CMake script reads the Engine's helper list, consumes completed
  matching-profile outputs, resolves libraries and stages copied files. It
  rewrites only the copies' library search paths.
- The layout is `bin/`, `lib/`, and `share/Lagom/`. The existing Engine resource
  lookup and Linux sandbox already accommodate that layout.
- The existing render smoke supports a packaged executable. Final acceptance
  also requires masking the checkout and testing a runtime-only environment.

## Alternatives

The root-staging candidate avoided an Engine repin but proposed extra ELF tools
and a wrapper. The Engine-install candidate derived install knowledge directly
but required a dependency commit and made packaging start a build.

The independent judge selected root staging. Its useful additions were CMake
script-mode dependency scanning and search-path rewriting, reading the helper
list directly from pinned Engine source, and native Rust startup. This avoids
extra package tools and an Engine publication dependency. Packaging never
starts a build or modifies an active build tree.

## Gates

Fixture tests establish staging mechanics only. The full browser must build,
load and render a page after relocation without the source checkout, accept
installed links, and retain its profile across update and removal before issue
#220 is complete. Runtime dependencies and redistribution notices must be
verified before publishing a downloadable browser.
