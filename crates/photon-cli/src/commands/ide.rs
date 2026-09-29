use std::path::Path;

use crate::support::output;

pub(crate) fn setup(root: &Path, _verbose: bool) -> Result<(), String> {
    output(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
        root,
    )
    .map_err(|error| format!("Cargo workspace metadata is unavailable: {error}"))?;
    println!("IDE setup ready:");
    println!("  Rust    {}", root.join("Cargo.toml").display());
    println!("  TSX     {}", root.join("ui/tsconfig.json").display());
    Ok(())
}

pub(crate) fn doctor(root: &Path) {
    let rust = output(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
        root,
    )
    .is_ok();
    println!("IDE");
    println!(
        "  Rust workspace {}",
        if rust { "ready" } else { "unavailable" }
    );
    println!(
        "  TSX config      {}",
        if root.join("ui/tsconfig.json").is_file() {
            "ready"
        } else {
            "missing"
        }
    );
}
