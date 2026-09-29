# Photon development rules

1. The Photon shell uses React/TSX for presentation and composition, with GPUIX/GPUI owning native windows and rendering.
2. Browser application logic and state belong in Rust; keep `photon-core` independent of UI frameworks and Ladybird classes.
3. GPUIX-specific native integration belongs in the Photon shell adapter. Keep generic changes in the pinned `PhotonBrowser/gpuix` submodule.
4. C++ is only for the narrow Ladybird embedder boundary and required engine glue.
5. Web engine changes belong in `Engine` and must use the explicit Photon Engine embedder API from shell code.
6. UI code must not reach into arbitrary engine internals or receive page pixel buffers.
7. Generated files stay out of source trees.
8. Keep Photon Engine regularly mergeable with Ladybird upstream and keep PhotonBrowser/gpuix regularly mergeable with remorses/gpuix.
9. Do not embed a browser to render Photon UI; do not add Qt Quick, Electron, or Tauri to the app shell.
10. Keep ordinary UI work independent of engine internals.
11. Run `./photon format`, `./photon check`, and relevant `./photon test` commands.
12. Do not use computer use, unless you are ONLY screenshotting.
13. Before changing engine behavior, inspect existing Engine UI/Qt and AppKit implementations for relevant behavior to preserve.
14. Keep the shell minimal and avoid adding browser chrome unless explicitly requested.
15. When setting up a checkout, run `./photon setup`; it initializes pinned submodules recursively. Never use `git submodule update --remote` to casually advance a dependency: Photon pins tested commits.
16. Before syncing upstream Engine or GPUIX code, read [Documentation/Upstream.md](Documentation/Upstream.md). Keep `origin` pointed at the Photon-owned repository and `upstream` pointed at Ladybird or remorses/gpuix as appropriate.
17. Perform upstream merges inside the dependency repository on a review branch. Resolve conflicts by understanding both sides and preserving Photon-specific patches; do not blanket-select `ours` or `theirs`. Run the dependency checks before merging to its Photon default branch.
18. Update a submodule pin only after its tested commit is available on the Photon-owned remote. Commit the gitlink change in the Photon root repository and verify `git submodule status --recursive` afterward.
19. Resolve a submodule conflict in the parent repository by choosing a tested dependency commit and staging the gitlink; a submodule entry is a commit pointer, not a text file to merge.
