use std::path::Path;

use crate::support::{CLANG_FORMAT_VERSION, clang_format, invoke, path, stage, success};

pub(crate) fn lint(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Check Rust formatting");
    invoke("cargo", &["fmt", "--all", "--", "--check"], root, verbose)?;

    stage("Lint Rust workspace");
    invoke(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        root,
        verbose,
    )?;

    let sources = super::format::native_sources(root)?;
    check_native_sources(
        root,
        "Photon native sources",
        &sources,
        CLANG_FORMAT_VERSION,
        verbose,
    )?;

    let engine_sources = super::format::engine_embedder_sources(root)?;
    check_native_sources(
        root,
        "Photon Engine embedder sources",
        &engine_sources,
        CLANG_FORMAT_VERSION,
        verbose,
    )?;

    success("Lint checks passed");
    Ok(())
}

fn check_native_sources(
    root: &Path,
    label: &str,
    sources: &[String],
    version: &str,
    verbose: bool,
) -> Result<(), String> {
    if sources.is_empty() {
        return Ok(());
    }
    let clang_format = clang_format(root, version)?;
    stage(&format!("Check {label} with clang-format {version}"));
    let mut args = vec!["--style=file", "--dry-run", "--Werror"];
    args.extend(sources.iter().map(String::as_str));
    invoke(&path(&clang_format), &args, root, verbose)
}
