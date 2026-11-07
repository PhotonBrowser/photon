# Linux distribution work for issue #220

## Goal

Install and launch the full Photon browser without a source checkout or build
tools. Opening a web URL must reach the installed browser. Record runtime
libraries, profile locations, updates, and removal behavior.

## Verification gates

1. Prepare the pinned Engine and GPUI dependencies with `./photon setup`.
2. Build the full browser and run `scripts/linux_render_smoke.py` in an isolated
   graphical session. A shell-only launch does not pass this gate.
3. Stage an installation outside the checkout. Launch from another working
   directory and verify that Engine helpers, resources, and libraries resolve.
4. Exercise the installed desktop entry and URL arguments. Document profile
   retention, updates, and uninstall behavior.
5. Run formatting, workspace checks, relevant tests, and independent review.

## Sequence

- Capture the Linux baseline and resolve build blockers first.
- Choose the first package target after inspecting runtime relocation needs.
- Implement packaging in small commits with a check for each unit.
- Keep packaging experimental until the installed full browser passes the
  rendering check. Do not claim completion of issue #220 before that gate.

## Initial environment

The host is TUXEDO OS based on Ubuntu 24.04, with Qt 6.10.2 and CMake 4.2.3.
A graphical session is available. The initial `./photon setup` invocation fails
before running the CLI because Cargo's patched GPUI dependency is not yet
initialized. Initialize the pinned submodules before retrying setup.

## Progress checkpoint

At the end of each build or implementation unit, record the result and next
blocker in `linux-distribution-220.tsv`. Commit verified changes separately.
The first checkpoint is full setup and a recorded full-build result.

## Contribution dependency pin

This contribution branch manually pins Engine
`a2c37c1d4e63762571a9bbc8a9a173515964a4a7`, advertised by the Photon-owned
`refs/pull/5/head` at https://github.com/PhotonBrowser/photon-engine/pull/5.
Its full Linux debug build and three native CPU-default canvas launches passed
with rendering overrides removed and clean persistent session markers. The
1,204 LibWeb Rust unit tests passed at its CSS-fix parent `191d9b5000e`; the
new commit only changes the private C++ Photon Linux painting policy. The
macOS build and LibWeb text tests have not run. Vulkan CPU readback returned
transparent pixels on the tested AMD host and remains experimental opt-in.
Final archive acceptance at this new pin must be recorded separately.

This uses the manual-pinning rule in `Documentation/Upstream.md`; it is an
exception to `photon deps pin`, which requires ancestry on `origin/master`.
The PR remains unmerged and is based on the earlier root pin `f3ffa371f3a`;
the current Engine master contains a newer Ladybird merge. The PR head can
move, and a squash or rebase merge would not preserve this exact commit in
master. Before this root change can land, merge the Engine fixes and replace
this contribution pin with an available, tested master commit if necessary.
Do not relax the CLI ancestry check or change the official submodule URL.

A fresh root clone followed by normal `git submodule update --init --depth 1
Engine` must resolve the recorded SHA from the official URL before pushing
this contribution pin. Its result belongs in the decision log.
