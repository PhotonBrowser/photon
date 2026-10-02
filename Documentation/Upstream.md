# Upstream maintenance

Photon pins tested commits of two Photon-owned repositories. `photon.toml` configures each submodule and its remotes; `.gitmodules` stores the clone URLs. Setup initializes missing dependencies and preserves existing worktrees. Do not use `git submodule update --remote` to select a new pin.

| Dependency | Photon `origin` | `upstream` | Persistent Photon branch |
| --- | --- | --- | --- |
| `Engine/` | `PhotonBrowser/photon-engine` | `LadybirdBrowser/ladybird` (`master`) | `master` |
| `vendor/gpui-ce/` | `PhotonBrowser/gpui-ce` | `gpui-ce/gpui-ce` (`main`) | `main` |

GPUI-CE is a standalone submodule. Keep generic externally-produced Metal surface support in `PhotonBrowser/gpui-ce`; keep Photon frame ordering, broker policy, Engine ownership and release translation in this repository. Do not add Photon or Ladybird types to GPUI-CE.

## Routine development

Use `./photon engine edit` and `./photon gpui edit` to switch the initialized submodules to their persistent Photon branches. `./photon engine pin` and `./photon gpui pin` return them to the commits in the current root checkout. These commands require clean source worktrees before switching.

## Sync upstream

Before syncing, save work in the root and dependency being changed. Never switch the Engine checkout while CMake or Ninja is building from it; wait for the build or stop it cleanly first.

For GPUI-CE, `./photon gpui sync` fetches and merges upstream `main` into the persistent Photon `main` branch. Resolve conflicts by understanding both sides and preserving Photon-independent generic renderer behavior. Run focused dependency checks and Photon checks before pushing the branch:

```bash
cargo check -p gpui_ce_apple
cargo test -p gpui_ce_apple
./photon format
./photon check
./photon test
```

For Engine, `./photon engine sync` merges Ladybird `master` into Photon `Engine/master`. Inspect the merge and run Photon validation before pushing.

Only advance a root gitlink after its tested dependency commit exists on the Photon-owned remote. Then check out that exact commit in the submodule and stage the gitlink in the Photon root:

```bash
git -C vendor/gpui-ce fetch origin --prune
git -C vendor/gpui-ce checkout --detach <tested-gpui-ce-commit>
git add vendor/gpui-ce
git diff --cached --submodule=log
git submodule status --recursive
```

Apply the same sequence to `Engine/` when its pin changes. A submodule conflict is resolved by choosing a tested dependency commit and staging the gitlink; it is not a source file to merge.

Do not blanket-select `ours` or `theirs` for source conflicts. Inspect the base and both sides, resolve each file deliberately, and run the relevant checks before completing a dependency merge.
