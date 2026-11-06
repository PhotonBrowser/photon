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
