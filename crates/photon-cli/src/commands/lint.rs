use std::path::Path;

use crate::support::{clang_format, invoke, path, stage, success};

pub(crate) fn lint(root: &Path, verbose: bool) -> Result<(), String> {
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

    stage("Check native formatting");
    let sources = super::format::native_sources(root)?;
    if !sources.is_empty() {
        let clang_format = clang_format(root)?;
        let mut args = vec!["--dry-run", "--Werror"];
        args.extend(sources.iter().map(String::as_str));
        invoke(&path(&clang_format), &args, root, verbose)?;
    }

    success("Lint checks passed");
    Ok(())
}
