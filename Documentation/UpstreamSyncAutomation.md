# Automated upstream sync reviews

Photon can use `photon deps sync --review` to integrate Ladybird, GPUI-CE, and gpui-motion and dispatch a GitHub Actions review workflow. Local merges do not require GitHub authentication by default. `photon deps pin` later records tested dependency revisions in the root repository.

The workflow at `.github/workflows/upstream-sync-review.yml` listens for the `photon_upstream_sync` repository dispatch sent by `photon deps sync --review` (or legacy `photon sync`). GitHub only runs a `repository_dispatch` workflow when its workflow file exists on the repository's default branch, so merge this workflow there before dispatching a review. It opens one combined **Photon Upstream Sync #NNN** GitHub issue with before and after tips for Engine, GPUI-CE, and gpui-motion.

## Setup

1. In GitHub, allow GitHub Actions to create issues. The workflow already requests `contents: read` and `issues: write` using the automatic `GITHUB_TOKEN`. If your organisation restricts Actions token permissions, adjust its Actions policy.
2. In Linear, enable **GitHub Issues Sync** for `PhotonBrowser/photon` and the **Photon (PHTN)** team, choosing **two-way** if you want newly created issues to appear on both services. Verify with one test issue. Repository visibility applies to synced content.
3. Authenticate the GitHub CLI on machines that run `photon deps sync --review` (or legacy `photon sync`) with `gh auth login`; the authenticated account needs permission to dispatch repository events to `PhotonBrowser/photon`. The command checks authentication before it starts merging. No Linear API key is required in Photon or GitHub Actions.

## Use

Continue with your normal local procedure:

```sh
photon deps sync --check
photon deps sync --review
# Resolve any merge conflicts, then run the documented checks.
photon deps pin
# Push the Photon-owned dependency branches and root pin commit in the normal order.
```

Only a successful all-dependency sync with review dispatch enabled starts this workflow. `photon deps sync --check`, per-dependency syncs, and merging a pull request that changes submodule pins do not dispatch it. A sync that advances no dependency branch creates no issue. The workflow records the persistent branch tips at sync time; run `photon deps pin` after review and checks to update the root gitlinks.

A hidden marker built from the root SHA and all three after-sync branch tips prevents duplicate issue creation if GitHub retries a dispatch.

## Limitations and safety

- The review event is dispatched after all three top-level sync steps succeed. If GitHub rejects the dispatch, the dependency merges remain complete and the command prints a retry request.
- The issue records persistent Photon branch tips at sync time, which may be ahead of the tested revisions eventually recorded by `photon deps pin`.
- Issue status remains open for human review. The Action does **not** claim a successful build, run Velocity, close an issue or declare a sync verified solely because a push succeeded.
- Issue numbering is based on existing GitHub upstream-sync review titles and is serialized by Actions concurrency; manual renames/deletions may disrupt numbering.
- Pull request merges and root pin changes do not trigger this workflow. Re-running a previously dispatched event reuses the existing issue for the same root and dependency tips.
- GitHub–Linear issue mirroring depends on the separate integration being enabled and on its configured field/status mappings. The workflow itself only calls the GitHub API.
- The workflow does not perform upstream merges, modify pins or push code. The local Photon CLI dispatches the event using the authenticated GitHub CLI; the workflow uses the standard GitHub Actions token to create the issue.
