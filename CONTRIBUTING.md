# Contributing to Photon

Thanks for your interest in Photon! Photon is pre-release software, and contributions of all sizes are welcome for review. Please keep changes focused and be patient with review times.

## Get started

1. Read the [contributor setup guide](CONTRIBUTOR_SETUP.md) for platform prerequisites and first checkout.
2. Follow [Building Photon](Documentation/Building.md) to build, run, and test the project.
3. Read the [architecture guide](Documentation/Architecture.md) to find the right component.
4. Check the [roadmap](Documentation/Roadmap.md) and [GitHub issues](https://github.com/PhotonBrowser/photon/issues) before starting work.

For a first contribution, look for issues labelled `good first issue` or `help wanted`. If there are none, documentation, tests, and small bug fixes are good places to start. Comment on an issue to discuss an approach before investing in a large change.

## AI-assisted work

AI tools may assist with research, coding, or review, but you are responsible for every change you submit. Read the full diff, verify the behavior, and remove or rewrite anything you cannot explain and support. Do not submit AI-written PR descriptions: explain the change and its validation in your own words.

Before requesting maintainer review, resolve merge conflicts and respond to every CodeRabbit and other automated review finding. Verify each suggestion, make the appropriate fix, and explain any finding you leave unresolved. Automated reviews do not replace your own review of the diff or project checks.

The PR template repeats these acknowledgments, and CI checks that both are selected. Maintainers must require the `Photon checks` status in branch protection for this check to block merging.

## Submit a pull request

1. Fork the repository (or create a branch if you have write access) and make a focused change.
2. Add or update tests and documentation where relevant.
3. Run the checks that apply to your change:

   ```bash
   photon format
   photon check
   photon test
   ```

   `photon build` builds the Engine and can take a while. Run it for Engine or native integration changes when practical. Explain skipped checks and platform limitations in your PR.
4. Open a pull request against `main`. Complete the PR template honestly, link the related issue, and include screenshots or recordings for visible UI changes. Its checklist asks you to confirm that you understand the full diff and will address automated review findings before requesting maintainer review.
5. Respond to review feedback. Maintainers may request changes or decline proposals that do not fit the project's direction.

**Never disclose security vulnerabilities in public issues or PRs.** See the [security policy](SECURITY.md). Please follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Where changes belong

| Area | Location |
| --- | --- |
| Browser UI, tabs, and GPUI views | `crates/photon-shell/src/platform/ui/` |
| Platform-specific integration | `crates/photon-shell/src/platform/<target>/` |
| Shared browser rules and state | `crates/photon-core/` |
| Profile and persistent data | `crates/photon-storage/` |
| Rust-to-Engine integration | `crates/photon-shell/src/platform/engine/` and `native/embedder/` |
| Ladybird-based engine and embedder API | [Photon Engine](https://github.com/PhotonBrowser/photon-engine) |
| Generic GPUI-CE framework changes | [Photon GPUI-CE](https://github.com/PhotonBrowser/gpui-ce) |
| Generic motion library changes | `crates/gpui-motion/` |

Keep shared UI code platform-neutral, platform-specific behavior in its adapter, and engine implementation behind the embedder boundary. Avoid unrelated refactors in focused PRs.

Photon keeps Engine and GPUI-CE as pinned Git submodules. The gpui-motion crate is vendored directly in this repository and can be changed alongside Photon code. Follow [Upstream maintenance](Documentation/Upstream.md) for Engine and GPUI-CE development branches, upstream syncs, validation, and pin updates. Do not use `git submodule update --remote` to change dependency pins.

## GitHub issues and Linear

**GitHub is the public entry point for external contributors.** Use GitHub issues to report bugs, propose features, and discuss changes; link the GitHub issue in your PR.

Maintainers also use Linear for internal planning and prioritization. Contributors do not need a Linear account. Maintainers can link or synchronize relevant work between the two trackers; do not assume every internal Linear issue is available publicly.

## Before asking for help

Check [Contributor setup](CONTRIBUTOR_SETUP.md), [Building Photon](Documentation/Building.md), and existing GitHub issues. When asking about a build failure, include your OS, tool versions, relevant logs, and commit SHA, with secrets removed.
