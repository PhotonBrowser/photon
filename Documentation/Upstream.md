# Upstream maintenance tutorial

Photon pins exact commits of two Photon-maintained downstream repositories. GPUIX also pins the Photon Zed fork as a nested submodule. `photon.toml` configures which root submodules `./photon setup` initializes and the Git remotes it ensures in each checkout. Git still owns the pinned commits and clone URLs in `.gitmodules`; setup never advances a submodule to a branch tip.

| Dependency | Photon remote (`origin`) | Upstream remote/reference | Photon branch |
| --- | --- | --- | --- |
| `Engine/` | `PhotonBrowser/photon-engine` | `LadybirdBrowser/ladybird` (`master`) | `master` |
| `vendor/gpuix/` | `PhotonBrowser/gpuix` | `remorses/gpuix` (`main`) | `main` |
| `vendor/gpuix/zed/` (nested) | `PhotonBrowser/zed` | `remorses/zed` (`gpuix`, [`ea042f2`](https://github.com/remorses/zed/tree/ea042f2f045157ada1d0ad71009520931dcace83)) | `photon/live-image` |

The root repository records the tested Engine and GPUIX commits as submodule gitlinks. GPUIX records its tested Zed commit in its own nested gitlink. `photon.toml` limits Zed's `upstream` fetch to the `gpuix` branch, currently at `ea042f2`; setup does not fetch or track `remorses/zed` `main`. The Photon Zed branch is based on that upstream commit. Merge upstream changes into each Photon-owned repository first, then update the gitlink that owns that dependency. This keeps Photon-specific changes in their respective downstreams and lets each layer pin tested revisions.

## Make routine GPUI changes

Use the existing long-lived Photon branches for ordinary GPUI work. Do not make a new branch for each edit:

- `vendor/gpuix/zed/`: `photon/live-image` tracks `PhotonBrowser/zed` and contains Photon GPUI changes.
- `vendor/gpuix/`: `main` tracks `PhotonBrowser/gpuix` and contains GPUIX changes plus the tested Zed gitlink.
- The Photon root pins a tested GPUIX commit; it should stay detached at that pin during normal builds.

After the first `./photon setup`, enter edit mode with one command:

```bash
./photon gpui edit
```

Edit GPUI in `vendor/gpuix/zed/crates/gpui`, then commit it on the existing `photon/live-image` branch. The command refuses to switch if either repository has uncommitted source changes. After the GPUI commit, update GPUIX's nested pin on its existing `main` branch:

```bash
git -C vendor/gpuix add zed
git -C vendor/gpuix commit -m "Update Photon GPUI pin"
```

If the change also modifies GPUIX, include those files in the same GPUIX commit. Run the checks required by the change, then push the Zed and GPUIX commits to their Photon-owned remotes. Only after the tested GPUIX commit is available from `PhotonBrowser/gpuix` should the root repository update and commit `vendor/gpuix` as described below. Run `./photon gpui pin` to return to the versions currently pinned by the root checkout; this preserves the commits on both long-lived branches.

The nested gitlinks are deliberate: Zed owns GPUI source, GPUIX owns its tested Zed revision, and Photon owns its tested GPUIX revision. Keep the work on these two persistent branches and move each pin forward only after its dependency commit is ready. Avoid running `./photon setup` while developing on these branches because recursive submodule initialization restores the revisions recorded by the parent checkout.

## Before syncing

Start at the Photon repository root. Commit or save work in the root and both submodules before switching branches. Do not try to sync over local edits.

Before switching or checking out an Engine revision, make sure no Photon or Engine build is compiling from `Engine/`. CMake and Ninja read source files throughout the build; changing the submodule worktree while they run can make files disappear temporarily and corrupt the generated Ninja graph. Let the build finish or stop it cleanly before changing the checkout.

```bash
./photon setup
git status --short
git -C Engine status --short
git -C vendor/gpuix status --short
git submodule status --recursive
```

The three worktrees should be clean. `./photon setup` initializes the submodules listed in `photon.toml` and configures the repositories and remotes listed there. Edit that file when adding a dependency or changing a fork/upstream mapping. Confirm the remotes before fetching:

```bash
git -C Engine remote -v
git -C vendor/gpuix remote -v
```

Engine `origin` must be Photon Engine and `upstream` must be Ladybird. GPUIX `origin` must be Photon GPUIX and `upstream` must be remorses/gpuix. If the upstream's default branch changes, verify it with the repository maintainers before changing the commands below.

## Sync Ladybird into Photon Engine

The Engine is a downstream of Ladybird. Merge Ladybird's `master` into a review branch based on the current Photon Engine `master`:

```bash
cd Engine
git fetch origin --prune
git fetch upstream --prune
git switch master
git pull --ff-only origin master
git switch -c sync/ladybird-YYYY-MM-DD
git merge --no-ff upstream/master
```

Replace `YYYY-MM-DD` with the sync date. This merge keeps Photon Engine's commits and combines them with Ladybird's new commits. Do not reset Photon Engine to `upstream/master` or replace the downstream history.

If Git reports conflicts, use the conflict workflow below. After the merge is resolved, build and check it from the Photon root so the regular embedder configuration and pinned build are used:

```bash
cd ..
git -C Engine submodule update --init --recursive
./photon check
./photon test
./photon build
```

The merge command creates a merge commit when Git can merge cleanly; after conflicts, `git merge --continue` creates it. Once the checks pass, push the review branch to `origin` and merge it into `PhotonBrowser/photon-engine` `master` according to that repository's review policy:

```bash
git -C Engine push -u origin sync/ladybird-YYYY-MM-DD
```

The Photon root must only pin a merge commit that is present on the Photon Engine remote.

## Sync upstream GPUIX into the Photon fork

GPUIX's generic external custom-element API is maintained in `PhotonBrowser/gpuix`. Photon-specific elements and engine code stay in this repository. Merge upstream `main` into a review branch based on the Photon fork's `main`:

```bash
cd vendor/gpuix
git fetch origin --prune
git fetch upstream --prune
git switch main
git pull --ff-only origin main
git switch -c sync/gpuix-YYYY-MM-DD
git merge --no-ff upstream/main
```

Resolve conflicts using the workflow below. Preserve the generic custom-element registration and render-context extension needed by Photon, while incorporating compatible upstream changes. Do not add Ladybird or Photon browser code to GPUIX.

Before merging into `PhotonBrowser/gpuix` `main`, validate the generic extension and package builds:

```bash
git submodule update --init --recursive
cargo fmt --manifest-path packages/native/Cargo.toml -- --check
cargo test --manifest-path packages/native/Cargo.toml --lib extension_api_tests
cd packages/react && bun run build
cd ../../..
```

Also run the full GPUIX checks required by the change. Push the review branch to `origin` and merge it into the Photon fork's `main` according to its review policy:

```bash
git -C vendor/gpuix push -u origin sync/gpuix-YYYY-MM-DD
```

The Photon root should pin only a tested commit available from `PhotonBrowser/gpuix`.

## Resolve source conflicts safely

For a normal file conflict, inspect each conflict and both sides before editing. Git records the merge base, current Photon downstream (`ours`), and incoming upstream (`theirs`) as index stages 1, 2, and 3. Standard conflict markers show `ours` and `theirs`; diff3-style markers also show the base.

```bash
git status
git diff --name-only --diff-filter=U
git diff -- path/to/conflicted-file
git show :1:path/to/conflicted-file
git show :2:path/to/conflicted-file
git show :3:path/to/conflicted-file
```

Edit the file to preserve Photon behavior and take compatible upstream improvements. Do not run a repository-wide `git checkout --ours .` or `git checkout --theirs .`; either can silently discard an entire side of the downstream merge. Stage only resolved paths, review the staged result, and continue:

```bash
git add path/to/resolved-file
git diff --cached
git merge --continue
```

After `git merge --continue` creates the merge commit, run the dependency checks before pushing. If a check fails, fix it in a follow-up commit on the review branch. If the conflict resolution is going in the wrong direction, return the worktree to its pre-merge state with `git merge --abort`, then retry with a smaller set of changes. Never force-push to repair a conflict.

When resolving Engine conflicts, keep the explicit `LibPhotonEmbedder` contract used by Photon and preserve Photon Engine changes that have not yet been accepted upstream. When resolving GPUIX conflicts, keep the API generic and preserve external factory registration, renderer-owned instance lifetime, and native image rendering. Update or add tests for behavior changed by the merge.

## Update Photon’s pinned submodules

After the upstream merge has landed on the Photon-owned default branch, work from the Photon root and check out the exact merged commit from each `origin`. Use only the dependencies that changed:

```bash
git -C Engine fetch origin --prune
git -C Engine checkout --detach <merged-engine-commit>
git -C Engine submodule update --init --recursive
git add Engine
```

For GPUIX:

```bash
git -C vendor/gpuix fetch origin --prune
git -C vendor/gpuix checkout --detach <merged-gpuix-commit>
git -C vendor/gpuix submodule update --init --recursive
git add vendor/gpuix
```

Confirm the staged gitlink is the intended remote commit and review its included commits:

```bash
git diff --cached --submodule=log
git submodule status --recursive
```

Build and test Photon against the new pin, then commit and push the root change:

```bash
./photon check
./photon test
./photon build
git commit -m "Update Photon Engine and GPUIX pins"
git push origin main
```

If only one dependency changed, stage only that path and use a matching commit message. After the root commit is pushed, run `./photon setup` and verify `git submodule status --recursive` shows the committed revisions with no `+` or `-` markers.

Do not run `git submodule update --remote` to choose a dependency revision. It advances to whichever branch the submodule tracks, which may be unreviewed and may not be present on the Photon-owned remote.

## Resolve a conflict in the root gitlinks

When a root-repository merge reports a conflict on `Engine` or `vendor/gpuix`, the conflict is between commit pointers; it is not a source-file conflict. Decide which tested dependency commit should be pinned, check it out in the submodule, then stage the gitlink:

```bash
git -C Engine fetch origin --prune
git -C Engine checkout --detach <tested-engine-commit>
git add Engine
```

Or, for GPUIX:

```bash
git -C vendor/gpuix fetch origin --prune
git -C vendor/gpuix checkout --detach <tested-gpuix-commit>
git add vendor/gpuix
```

Use `git diff --cached --submodule=log` to review the chosen commit. Ensure it is reachable from the Photon-owned remote and validate the root build before completing the root merge. Resolve any `.gitmodules` text conflict separately, retaining the Photon-owned repository URLs.
