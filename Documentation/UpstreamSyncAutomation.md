# Automated upstream sync reviews

Photon already uses `./photon sync` to integrate Ladybird and GPUI-CE and `./photon pin` to record tested dependency revisions in the root repository.

The GitHub workflow at `.github/workflows/upstream-sync-review.yml` watches the **root** `main` branch for changes to the `Engine` and `vendor/gpui-ce` gitlinks. When a push changes at least one pin, it opens one **Photon Upstream Sync #NNN — Ladybird & GPUI-CE** GitHub issue.

## Setup

1. In GitHub, allow GitHub Actions to create issues. The workflow already requests `contents: read` and `issues: write` using the automatic `GITHUB_TOKEN`. If your organisation restricts Actions token permissions, adjust its Actions policy.
2. In Linear, enable **GitHub Issues Sync** for `PhotonBrowser/photon` and the **Photon (PHTN)** team, choosing **two-way** if you want newly created issues to appear on both services. Verify with one test issue. Repository visibility applies to synced content.
3. Merge the workflow pull request into `main`. No Linear API key is required in Photon or GitHub Actions.

## Use

Continue with your normal local procedure:

```sh
./photon sync --check
./photon sync
# Resolve any merge conflicts, then run the documented checks.
./photon pin
# Push the Photon-owned dependency branches and root pin commit in the normal order.
```

When the updated **root** pin commit is pushed to `main`, GitHub Actions records both old/new gitlinks, links to commit comparisons, and adds a manual verification checklist. A pin change to just one dependency still creates one combined review, showing the other as unchanged. A sync with no pin changes creates no issue.

To document a historical root commit pair, run the workflow manually in GitHub's Actions tab using full 40-character `from_sha` and `to_sha`. A hidden marker containing the target root SHA prevents duplicate issue creation when retried or manually rerun.

## Limitations and safety

- An unpushed local sync or unresolved local merge cannot trigger GitHub Actions. The issue records **committed dependency pins**, not every fetch, merge attempt or upstream head.
- Issue status remains open for human review. The Action does **not** claim a successful build, run Velocity, close an issue or declare a sync verified solely because a push succeeded.
- Issue numbering is based on existing GitHub upstream-sync review titles and is serialized by Actions concurrency; manual renames/deletions may disrupt numbering.
- A single push combining multiple previous sync commits produces one review of the net pin changes. Separate pushes produce separate reviews. If you push multiple updates before a workflow runs, each push still gets its own queued run.
- GitHub–Linear issue mirroring depends on the separate integration being enabled and on its configured field/status mappings. The workflow itself only calls the GitHub API.
- The workflow does not perform upstream merges, modify pins or push code. It uses only the standard GitHub Actions token.
