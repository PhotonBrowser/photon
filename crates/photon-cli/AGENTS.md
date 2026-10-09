# photon-cli

Keep one module per command and reuse `support.rs` for process execution,
diagnostics, progress, and terminal output. Formatting and Rust checks operate
on the complete Cargo workspace; do not add package-name lists that omit new
crates.
