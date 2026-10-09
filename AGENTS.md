# Photon development rules

1. The Photon shell uses Rust with GPUI-CE for presentation, native windows, and rendering.
2. Browser application logic and state belong in Rust. Keep `photon-core` independent of UI frameworks, native handles, and Ladybird classes; keep the exported Rust C ABI in `photon-ffi`.
3. Photon-specific native integration belongs in Photon crates: the shell adapter owns browser and window lifecycle, while `photon-presentation-ipc` owns the XPC transport. Keep generic changes in the pinned `PhotonBrowser/gpui-ce` submodule.
4. C++ is only for the narrow Ladybird embedder boundary and required engine glue.
5. Web engine changes belong in `Engine` and must use the explicit Photon Engine embedder API from shell code.
6. UI code must not reach into arbitrary engine internals or receive page pixel buffers.
7. Generated files stay out of source trees.
8. Keep Photon Engine regularly mergeable with Ladybird upstream and keep PhotonBrowser/gpui-ce regularly mergeable with `gpui-ce/gpui-ce`.
9. Do not embed a browser to render Photon UI; do not add Qt Quick, Electron, or Tauri to the app shell.
10. Keep ordinary UI work independent of engine internals.
11. Run `./photon format`, `./photon check`, and relevant `./photon test` commands.
12. Do not use computer use, unless you are ONLY screenshotting.
13. Before changing engine behavior, inspect existing Engine UI/Qt and AppKit implementations for relevant behavior to preserve.
14. Keep the shell minimal and avoid adding browser chrome unless explicitly requested.
15. When setting up a checkout, run `./photon setup`; it initializes pinned submodules and activates the persistent Photon development branches. Use `vendor/gpui-ce/main` for ordinary GPUI work. The Photon root records exact dependency commit IDs even while the local worktrees are on branches. Never use `git submodule update --remote` to casually advance a dependency: Photon pins tested commits. See [Documentation/Upstream.md](Documentation/Upstream.md).
16. Before syncing upstream Engine or GPUI-CE code, read [Documentation/Upstream.md](Documentation/Upstream.md). Keep `origin` pointed at the Photon-owned repository and `upstream` pointed at Ladybird or `gpui-ce/gpui-ce` as appropriate.
17. Use the persistent Photon dependency branches documented in [Documentation/Upstream.md](Documentation/Upstream.md) for routine edits and upstream syncs; do not create a branch for each change. Resolve conflicts by understanding both sides and preserving Photon-specific patches; do not blanket-select `ours` or `theirs`. Run dependency checks before pushing or advancing a parent pin. Use a separate review branch for unusually large or risky upstream merges.
18. Update a submodule pin only after its tested commit is available on the Photon-owned remote. Commit the gitlink change in the Photon root repository and verify `git submodule status --recursive` afterward.
19. Resolve a submodule conflict in the parent repository by choosing a tested dependency commit and staging the gitlink; a submodule entry is a commit pointer, not a text file to merge.
20. Read GPUI-CE docs for GPUI-CE related changes: https://gpui-ce.github.io/gpui-ce/
21. Never switch or update the `Engine/` checkout while CMake or Ninja is building from it. Wait for the build to finish or stop it cleanly first.
22. Keep crate boundaries aligned with independent ownership. `photon-omnibox` owns address and search rules; `photon-core` owns browser state and commands; `photon-ffi` adapts core to the exported C ABI; `photon-shell` owns GPUI views and browser/window integration; `photon-presentation-ipc` owns presentation XPC transport; `photon-presentation-broker` is its service executable.
23. Keep shell color roles in `crates/photon-shell/src/platform/ui/theme.rs` and shared measurements and type sizes in `ui/metrics.rs`. Views use semantic color roles and named style tokens instead of local color or dimension literals.
24. `./photon format` and `./photon check` cover the whole Cargo workspace. When adding a crate, update the workspace and architecture docs; do not maintain per-command package lists.
