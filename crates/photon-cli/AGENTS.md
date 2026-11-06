# photon-cli

Keep one module per command and reuse `support.rs` for process execution,
diagnostics, progress, and terminal output. Formatting and Rust checks operate
on the complete Cargo workspace; do not add package-name lists that omit new
crates.

`commands/install.rs` installs or refreshes only the Cargo CLI binaries. Keep
Linux XDG desktop entry and URL-handler management in
`commands/linux_desktop.rs`, exposed as `photon desktop install` and
`photon desktop uninstall` for manual maintenance. Full `photon setup` invokes
desktop installation automatically after the CLI is installed;
`photon setup --ui-only` must leave existing desktop files untouched and must
not install them.

`commands/linux_package.rs` owns `photon package`, which archives completed
build outputs and never builds. `packaging/linux/stage.cmake` owns copying and
relocating ELF files; it reads the helper list from the Engine and the paths
from the Engine build's `CMakeCache.txt`. Keep `HELPER_PROCESS_TARGETS` in
`commands/build.rs` equal to the Engine's helper list; a test enforces it.
`commands/linux_desktop.rs` renders every desktop entry and writes the
packaged registration script from `packaging/linux/photon-desktop`.
