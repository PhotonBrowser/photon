# Photon Rust crate boundaries

- Put a feature in a separate crate when it has its own owner, public contract,
  or useful consumer. Keep tightly coupled implementation modules together.
- `photon-core` contains browser state and rules without UI, native, or Engine
  types, including the history and requests to clear browsing data.
  `photon-omnibox` owns address and search resolution and suggestion matching.
- `photon-brand` owns the browser's name and marks: name, engine name,
  internal page scheme, profile folder and logo. Never write the browser's
  name in UI text, addresses or file paths; read it from `photon-brand`.
- `photon-storage` owns the on-disk profile: saving and loading the history,
  page icons, data usage, and the Engine's website data folder. Keep it plain
  file access over core types, with no GPUI or Engine dependency, so settings
  and internal pages can use it directly.
  `photon-performance` owns performance diagnostics, accumulation, formatting,
  and its GPUI overlay.
- `photon-ffi` is the Rust exported C ABI. Confine raw pointers and C string
  conversion to this crate.
- `photon-shell` owns window-bound GPUI views, the Engine adapter, platform
  integration, and presentation lifecycle. Its performance model and overlay
  live in `photon-performance`.
- `photon-presentation-ipc` owns the XPC protocol, client channel, native
  descriptor transport, and service entry point. `photon-presentation-broker`
  only provides the service executable.
- Update `Cargo.toml`, `Documentation/Architecture.md`, and applicable scoped
  instructions when adding a workspace package.
- `photon-cli` uses workspace-wide formatting and checking. Do not add another
  hard-coded list of package names to developer commands.
