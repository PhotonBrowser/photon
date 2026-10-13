use std::path::Path;

use crate::support::{clang_format, invoke, output, path, stage, success};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Format Photon Rust sources");
    invoke("cargo", &["fmt", "--all"], root, verbose)?;
    let sources = native_sources(root)?;
    if !sources.is_empty() {
        let clang_format = clang_format(root)?;
        stage("Format native sources");
        let mut args = vec!["-i"];
        args.extend(sources.iter().map(String::as_str));
        invoke(&path(&clang_format), &args, root, verbose)?;
    }
    success("Formatting complete");
    Ok(())
}

/// Photon's own C, C++ and Objective-C files, including new ones not yet
/// added. Submodules and ignored build output are not Photon's to format.
pub(super) fn native_sources(root: &Path) -> Result<Vec<String>, String> {
    let listing = output(
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.c",
            "*.cc",
            "*.cpp",
            "*.h",
            "*.hpp",
            "*.m",
            "*.mm",
        ],
        root,
    )?;
    Ok(listing
        .lines()
        .filter(|path| root.join(path).is_file())
        .map(str::to_owned)
        .collect())
}
