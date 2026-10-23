# Upstream maintenance

Photon pins tested commits of Photon-owned repositories. `photon.toml` configures each submodule and its remotes; `.gitmodules` stores the clone URLs. `photon setup` initializes missing dependencies, configures remotes, and activates the persistent Photon branches. Repeating setup keeps existing worktrees and avoids fetching branches that are already present. Do not use `git submodule update --remote` to select a new pin.

A Git submodule entry in the Photon repository records a **commit ID**, never a branch name. This makes a fresh checkout reproducible. Your local `Engine/`, `vendor/gpui-ce/`, and `crates/gpui-motion/` worktrees can stay on `master` or `main` for everyday work. Moving a branch does not change the commit recorded by Photon until you advance the root gitlink with `photon deps pin`.

| Dependency | Photon `origin` | `upstream` | Persistent Photon branch |
| --- | --- | --- | --- |
| `Engine/` | `PhotonBrowser/photon-engine` | `LadybirdBrowser/ladybird` (`master`) | `master` |
| `vendor/gpui-ce/` | `PhotonBrowser/gpui-ce` | `gpui-ce/gpui-ce` (`main`) | `main` |
| `crates/gpui-motion/` | `PhotonBrowser/gpui-motion` | `1bye/gpui-motion` (`main`) | `main` |

Setup configures the Photon remotes to use HTTPS so fresh clones and submodule updates work without SSH keys. Before pushing through HTTPS, run `gh auth login` and `gh auth setup-git`; contributors who prefer SSH can configure SSH URLs in `photon.toml`.

GPUI-CE is a standalone submodule. Keep generic externally-produced Metal surface support in `PhotonBrowser/gpui-ce`; keep Photon frame ordering, broker policy, Engine ownership and release translation in this repository. Do not add Photon or Ladybird types to GPUI-CE.

The `gpui-motion` fork is based on upstream `1bye/gpui-motion` commit
[`fd9baf910ba0e88b06967bfb322d6be4b2d39723`](https://github.com/1bye/gpui-motion/commit/fd9baf910ba0e88b06967bfb322d6be4b2d39723).
Syncs merge upstream `main` into Photon's persistent `main` branch; keep the
GPUI-CE native adapters and Photon integration as separate commits after the
upstream base. The fork retains both upstream license notices.

## Routine development

Setup activates the persistent Photon branches. `photon deps status` shows each dependency's current branch, pin alignment, worktree cleanliness, and cached upstream counts. `photon deps edit [engine|gpui|motion]` switches one or all clean checkouts to the persistent Photon branches. `photon deps restore [engine|gpui|motion]` returns one or all checkouts to the commits pinned by this root repository. Switching requires a clean source worktree.

## Sync upstream

Before syncing, save work in the root and dependency being changed. Never switch the Engine checkout while CMake or Ninja is building from it; wait for the build or stop it cleanly first.

Run `photon deps sync --check` to fetch upstream tips and preview all three merges. Run `photon deps sync` to merge them locally, or specify `engine`, `gpui`, or `motion` to sync one dependency. The command selects the persistent Photon branch when the worktree is clean. A sync does not require GitHub authentication unless you pass `--review` to dispatch the review issue. If a merge conflicts, the command stops and prints every unresolved path, the full combined conflict diff, Git's output, and instructions for completing the merge. Resolve and commit that merge before starting another sync.

The legacy `photon sync` command remains available and dispatches the review workflow after a successful all-dependency sync. `photon sync --no-review` skips that dispatch. See [Automated upstream sync reviews](UpstreamSyncAutomation.md).

The preview compares committed branch tips; uncommitted edits are excluded and may still prevent a real sync.

For GPUI-CE, `photon deps sync gpui` fetches and merges upstream `main` into the persistent Photon `main` branch. Resolve conflicts by understanding both sides and preserving Photon-independent generic renderer behavior. Run focused dependency checks and Photon checks before pushing the branch:

```bash
cargo check -p gpui_ce_apple
cargo test -p gpui_ce_apple
photon format
photon check
photon test
```

For Engine, `photon deps sync engine` merges Ladybird `master` into Photon `Engine/master`. Inspect the merge and run Photon validation before pushing.

For gpui-motion, run the native reorder/accessibility suite and motion tests against the vendored GPUI-CE package before pushing `main`:

```bash
cargo test --manifest-path crates/gpui-motion/Cargo.toml \
  --features gpui,native-tests --test advanced_native --test motion \
  --config 'patch.crates-io.gpui-ce.path="vendor/gpui-ce/crates/gpui"'
```

Run `photon deps pin` to advance and commit root gitlinks to the current Engine, GPUI-CE, and gpui-motion branch tips. It checks that each changed tip is on the Photon-owned origin branch, runs the relevant dependency build or tests and Photon checks, commits only the changed gitlinks, and prints recursive submodule status. Push dependency commits to their Photon-owned remotes before using this command. It does not push.

`photon deps restore` moves clean local worktrees back to the commits already recorded by Photon. It does not advance or commit root gitlinks.

For manual pinning, only advance a root gitlink after its tested dependency commit exists on the Photon-owned remote. Then check out that exact commit in the submodule and stage the gitlink in the Photon root:

```bash
git -C vendor/gpui-ce fetch origin --prune
git -C vendor/gpui-ce checkout --detach <tested-gpui-ce-commit>
git add vendor/gpui-ce
git diff --cached --submodule=log
git submodule status --recursive
```

Apply the same sequence to `Engine/` when its pin changes. A submodule conflict is resolved by choosing a tested dependency commit and staging the gitlink; it is not a source file to merge.

Do not blanket-select `ours` or `theirs` for source conflicts. Inspect the base and both sides, resolve each file deliberately, and run the relevant checks before completing a dependency merge.
