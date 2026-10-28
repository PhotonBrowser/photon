<p align="center"><img src="crates/photon-brand/assets/brand-kit/logo/logo.svg" alt="Photon" width="96" /></p>

<h1 align="center">Photon</h1>

<p align="center"><strong>A browser built on <a href="https://ladybird.org">Ladybird</a>.</strong></p>

<p align="center">
  <a href="#run-from-source">Run from source</a> ·
  <a href="Documentation/Architecture.md">Architecture</a> ·
  <a href="Documentation/Roadmap.md">Roadmap</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<img width="2592" height="1644" alt="image" src="https://github.com/user-attachments/assets/59500315-d983-4af4-b105-4d102498ccc7" />


## Run from source

Install the platform prerequisites in [contributor setup](CONTRIBUTOR_SETUP.md) first.

```sh
git clone --recurse-submodules https://github.com/PhotonBrowser/photon.git
cd photon
```

On macOS, run `./photon setup --install-system-deps`; on Linux, install the
packages listed in [contributor setup](CONTRIBUTOR_SETUP.md), then run
`./photon setup`. Follow the platform-specific build and run guidance there
before launching Photon.

Setup installs or refreshes the `photon` command and `ph` alias in Cargo's bin directory. Make
sure `~/.cargo/bin` is on your `PATH` to use `photon` without the `./` prefix.

To work on the GPUI shell on Linux without setting up Ladybird Engine, clone
without submodules and use the UI-only path:

```sh
git clone https://github.com/PhotonBrowser/photon.git
cd photon
./photon setup --ui-only
photon run --ui
```

This opens the New Tab page and Photon-owned settings pages; web pages require
the full Engine setup.

## Develop Photon

Start with [contributor setup](CONTRIBUTOR_SETUP.md), then use the [build guide](Documentation/Building.md) and [architecture guide](Documentation/Architecture.md). See [contribution guidance](CONTRIBUTING.md) for pull requests and the [roadmap](Documentation/Roadmap.md) for planned work.

```mermaid
flowchart LR
    User[You] --> App[photon-app]
    App --> Shell[photon-shell<br/>windows, tabs, and UI]
    Shell --> Core[photon-core<br/>browser rules and state]
    Shell --> Storage[photon-storage<br/>profile files]
    Shell --> Embedder[Photon Engine API]
    Embedder --> Engine[Ladybird Engine<br/>web pages]
    Shell --> Platform[Platform adapters<br/>macOS · Linux · Windows]
    Shell --> PerfUI[photon-performance-ui<br/>GPUI charts and overlays]
    PerfUI --> Perf[photon-performance<br/>sampling and diagnostics]
    FFI[photon-ffi<br/>C API] --> Core
    CLI[photon-cli<br/>setup and developer commands]
```

The shell connects the browser interface to framework-independent browser
rules, profile storage, and the Ladybird Engine. See the
[architecture guide](Documentation/Architecture.md) for component boundaries
and common change locations.

<details>
<summary>Rust crate guides</summary>

- [photon-app](crates/photon-app/README.md) · [photon-brand](crates/photon-brand/README.md) · [photon-cli](crates/photon-cli/README.md) · [photon-core](crates/photon-core/README.md)
- [photon-ffi](crates/photon-ffi/README.md) · [photon-performance](crates/photon-performance/README.md) · [photon-performance-ui](crates/photon-performance-ui/README.md)
- [photon-presentation-broker](crates/photon-presentation-broker/README.md) · [photon-presentation-ipc](crates/photon-presentation-ipc/README.md) · [photon-shell](crates/photon-shell/README.md)
- [photon-storage](crates/photon-storage/README.md) · [gpui-motion](crates/gpui-motion/README.md)

</details>

## Star History

<a href="https://www.star-history.com/?repos=photonbrowser%2Fphoton&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=photonbrowser/photon&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=photonbrowser/photon&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=photonbrowser/photon&type=date&legend=top-left" />
 </picture>
</a>
