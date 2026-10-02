# Upstream maintenance

Photon pins tested commits of two Photon-owned repositories. `photon.toml` configures each submodule and its remotes; `.gitmodules` stores the clone URLs. `./photon setup` initializes missing dependencies, configures remotes, and activates the persistent Photon branches. Repeating setup keeps existing worktrees and avoids fetching branches that are already present. Do not use `git submodule update --remote` to select a new pin.

A Git submodule entry in the Photon repository records a **commit ID**, never a branch name. This makes a fresh checkout reproducible. Your local `Engine/` and `vendor/gpui-ce/` worktrees can stay on `master` and `main` for everyday work. Moving either branch does not change the commit recorded by Photon until you advance the root gitlink with `./photon pin`.

| Dependency | Photon `origin` | `upstream` | Persistent Photon branch |
| --- | --- | --- | --- |
| `Engine/` | `PhotonBrowser/photon-engine` | `LadybirdBrowser/ladybird` (`master`) | `master` |
| `vendor/gpui-ce/` | `PhotonBrowser/gpui-ce` | `gpui-ce/gpui-ce` (`main`) | `main` |

GPUI-CE is a standalone submodule. Keep generic externally-produced Metal surface support in `PhotonBrowser/gpui-ce`; keep Photon frame ordering, broker policy, Engine ownership and release translation in this repository. Do not add Photon or Ladybird types to GPUI-CE.

## Routine development

Setup activates the persistent Photon branches. `./photon engine sync` and `./photon gpui sync` also switch to the appropriate branch when the dependency worktree is clean, so an explicit edit command is unnecessary for normal use. `./photon engine edit` and `./photon gpui edit` remain available for manual switching; their `pin` counterparts return to the commits in the root checkout. Switching requires a clean source worktree.

## Sync upstream

Before syncing, save work in the root and dependency being changed. Never switch the Engine checkout while CMake or Ninja is building from it; wait for the build or stop it cleanly first.

Run `./photon sync` to fetch and merge Ladybird `master` into `Engine/master`, then GPUI-CE `main` into `vendor/gpui-ce/main`. Use `./photon engine sync` or `./photon gpui sync` to update one dependency. Each command selects its persistent Photon branch when the worktree is clean. If a merge conflicts, the command stops and prints every unresolved path, the full combined conflict diff, Git's output, and instructions an agent can use to finish the merge. Resolve and commit that merge before starting another sync.

Use `./photon sync --check` to fetch both upstream tips and see each Photon branch's ahead/behind counts and predicted merge conflicts without switching branches or merging. `./photon engine sync --check` and `./photon gpui sync --check` check one dependency. The preview compares committed branch tips; uncommitted edits are excluded and may still prevent a real sync.

For GPUI-CE, `./photon gpui sync` fetches and merges upstream `main` into the persistent Photon `main` branch. Resolve conflicts by understanding both sides and preserving Photon-independent generic renderer behavior. Run focused dependency checks and Photon checks before pushing the branch:

```bash
cargo check -p gpui_ce_apple
cargo test -p gpui_ce_apple
./photon format
./photon check
./photon test
```

For Engine, `./photon engine sync` merges Ladybird `master` into Photon `Engine/master`. Inspect the merge and run Photon validation before pushing.

Run `./photon pin` to advance and commit root gitlinks to the current Engine and GPUI-CE branch tips. It checks that each changed tip is on the Photon-owned origin branch, runs the relevant dependency build or tests and Photon checks, commits only the changed gitlinks, and prints recursive submodule status. Push dependency commits to their Photon-owned remotes before using this command. It does not push.

The per-dependency `engine pin` and `gpui pin` commands have a different purpose: they move a local worktree back to the commit already recorded by Photon. They do not advance or commit the root gitlink.

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
