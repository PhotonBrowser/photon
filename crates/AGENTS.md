# Photon Rust crate boundaries

- Put a feature in a separate crate when it has its own owner, public contract,
  or useful consumer. Keep tightly coupled implementation modules together.
- `photon-core` contains browser state and rules without UI, native, or Engine
  types. `photon-omnibox` owns address and search resolution.
- `photon-ffi` is the Rust exported C ABI. Confine raw pointers and C string
  conversion to this crate.
- `photon-shell` owns GPUI views, the Engine adapter, platform integration, and
  presentation lifecycle. The shell's views stay in the shell because they
  share window and Engine session state.
- `photon-presentation-ipc` owns the XPC protocol, client channel, native
  descriptor transport, and service entry point. `photon-presentation-broker`
  only provides the service executable.
- Update `Cargo.toml`, `Documentation/Architecture.md`, and applicable scoped
  instructions when adding a workspace package.
- `photon-cli` uses workspace-wide formatting and checking. Do not add another
  hard-coded list of package names to developer commands.
