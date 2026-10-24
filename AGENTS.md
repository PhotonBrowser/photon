# Photon — Agent Development Guide

> Instructions for coding agents working in the Photon repository. Follow these rules for implementation, refactoring, dependency maintenance, and upstream synchronization.

## Core principles

- **Rust owns the browser application.** Use Rust and GPUI-CE for shell presentation, native windows, and rendering. Browser application logic and state belong in Rust.
- **Respect architectural boundaries.** Keep normal UI work independent of engine internals. The shell must use the explicit Photon Engine embedder API, not arbitrary engine implementation details.
- **Keep Photon lightweight.** Do not add browser chrome unless explicitly requested. Never use an embedded browser to render Photon UI; do not introduce Qt Quick, Electron, or Tauri into the app shell.
- **Preserve upstream mergeability.** Keep Photon Engine regularly mergeable with Ladybird and the Photon GPUI-CE fork regularly mergeable with `gpui-ce/gpui-ce`.
- **Verify changes.** Run `photon format`, `photon check`, and relevant `photon test` commands.
- **No computer use**, except when the sole purpose is taking screenshots.

## Architecture and crate ownership

| Component | Responsibility | Boundary |
| --- | --- | --- |
| `photon-omnibox` | Address and search rules | Do not move omnibox policy into views. |
| `photon-core` | Browser state, commands, and application logic | Must remain independent of UI frameworks, native handles, and Ladybird classes. |
| `photon-ffi` | Exported Rust C ABI | Keep C ABI adaptation here rather than in core. |
| `photon-shell` | GPUI views and browser/window lifecycle integration | Shell adapter owns browser and window lifecycle; avoid arbitrary Engine internals. |
| `photon-presentation-ipc` | Presentation XPC transport | Owns transport, not browser policy. |
| `photon-presentation-broker` | Presentation broker service executable | Keep service-executable responsibilities here. |
| `Engine/` | Ladybird engine and Photon Engine embedder implementation | Engine changes belong here; shell calls the explicit embedder API. |
| `PhotonBrowser/gpui-ce` | Generic GPUI-CE changes in the pinned submodule | Keep Photon-specific native integrations in Photon crates. |

### Native and engine integration

1. Use **C++ only** for the narrow Ladybird embedder boundary and required engine glue.
2. Photon-specific native integration belongs in Photon crates. Generic GPUI-CE changes belong in the pinned `PhotonBrowser/gpui-ce` submodule.
3. UI code must **not** access arbitrary engine internals or receive page pixel buffers.
4. Before changing engine behavior, inspect existing Engine **UI/Qt and AppKit implementations** for relevant behavior that must be preserved.
5. Keep generated files out of source trees.

## UI design and styling ownership

Do not scatter hard-coded design values throughout views. Use these authoritative locations:

| Concern | Location |
| --- | --- |
| Shared semantic shell color-role resolution | `crates/photon-shell/src/platform/ui/theme.rs` |
| Colors, opacity, transparency, window tint, and corner radii | `crates/photon-shell/theme.toml` |
| Other shared measurements and typography sizes | `crates/photon-shell/src/platform/ui/metrics.rs` |
| Native window controls and native material behavior | `platform/<target>/window_settings.rs` in the relevant platform adapter |

Views must use **semantic color roles and named style tokens** instead of local color or dimension literals. Keep ordinary UI changes independent of Engine implementation details.

For GPUI-CE work, read the [GPUI-CE documentation](https://gpui-ce.github.io/gpui-ce/) before making changes.

## Checkout and dependency setup

1. When setting up a checkout, run **`photon setup`**. It initializes pinned submodules and activates persistent Photon development branches.
2. Read [`Documentation/Upstream.md`](Documentation/Upstream.md) **before any Engine or GPUI-CE upstream sync**. Its documented branch and remote arrangements are authoritative.
3. Use `vendor/gpui-ce/main` for ordinary GPUI work. Use the other persistent Photon dependency branches documented in `Documentation/Upstream.md` for routine edits and upstream syncs; **do not create a new branch for each change**.
4. Keep `origin` pointed at the **Photon-owned repository** and `upstream` pointed at **Ladybird** or **`gpui-ce/gpui-ce`**, as appropriate.
5. Photon root records **exact tested dependency commit IDs**, even when local submodule worktrees are on branches. Never casually advance dependencies with `git submodule update --remote`.
6. Never switch or update `Engine/` while CMake or Ninja is building from that checkout. Allow the build to finish or stop it cleanly first.

## Upstream synchronization and conflict resolution

**Goal:** Incorporate upstream improvements without silently losing working Photon behavior or breaking the integrated build.

1. **Prepare.** Read `Documentation/Upstream.md`; check build activity, working-tree status, dependency branch, remotes, and the currently pinned commits. Do not touch a live Engine checkout during a CMake/Ninja build.
2. **Review incoming changes.** Understand the intent of both upstream and Photon-specific modifications before resolving overlapping code. Check related callers, interfaces, tests, and platform-specific behavior.
3. **Resolve conflicts deliberately.** Never blanket-select `ours` or `theirs`.
   - If upstream is **substantially better** and supersedes a Photon change, prefer upstream and remove or adapt the redundant Photon implementation, after confirming required Photon behavior remains intact.
   - If two changes implement **different valid behavior** but conflict structurally, integrate both so that they work together.
   - If a Photon-specific patch remains necessary, preserve or reimplement it against the new upstream structure.
   - Do not keep dead compatibility layers or duplicated implementations merely to avoid choosing a coherent design.
4. **Validate behavior.** Resolve all conflicts, run relevant dependency checks and builds, inspect regressions, and ensure upstream improvements and required Photon behavior both work. Do not treat a clean merge alone as sufficient validation.
5. **Use an isolated review branch** for unusually large or risky upstream merges. Routine dependency edits and syncs stay on documented persistent branches.
6. **Publish dependency commits before repinning.** Update the parent gitlink only after the tested dependency commit is available on the Photon-owned remote.
7. **Commit and verify the pin.** Commit the gitlink change in Photon root, then run `git submodule status --recursive` to confirm the resulting dependency state.

### Parent-repository submodule conflicts

A submodule entry is a **commit pointer, not a text file**. When the parent repository has a submodule conflict, select or produce a tested dependency commit, stage the resolved gitlink, and validate that commit. Do not attempt line-by-line text merging of a gitlink.

## Validation and repository hygiene

For normal changes:

```sh
photon format
photon check
photon test
```

Run the **relevant** tests when the full suite is not appropriate, plus any dependency-specific checks or builds required by the affected area. `photon format` and `photon check` cover the **entire Cargo workspace**; do not maintain hand-written per-command package lists.

When adding a crate, update the Cargo workspace and architecture documentation. Keep crate boundaries aligned with independent ownership. Keep generated output outside source trees.

For dependency changes and upstream syncs, validate the dependency itself **before pushing or advancing the parent pin**, and recheck the integrated Photon state after repinning. An unresolved conflict, failed test, or unverified integration must be reported rather than treated as complete.

## Agent completion checklist

Before describing work as finished, confirm:

- [ ] Browser logic, views, FFI, native adapters, and engine code remain in their owning components.
- [ ] No UI-to-engine internals coupling or page pixel buffers have been introduced.
- [ ] UI styling uses shared semantic roles, tokens, and platform adapters.
- [ ] Engine behavior changes account for existing Qt/AppKit behavior.
- [ ] `photon format`, `photon check`, and relevant tests have been run, with results reported.
- [ ] Any upstream sync has resolved conflicts intentionally and retained required behavior.
- [ ] Any changed dependency pin refers to a tested commit present on a Photon-owned remote.
- [ ] `git submodule status --recursive` has been verified after a gitlink change.
- [ ] No generated files or unnecessary browser chrome were added.

**If a check could not be performed, state that explicitly and explain what remains unverified.**
