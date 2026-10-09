use std::path::Path;

use crate::support::{invoke, output, stage, success};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Format Photon Rust sources");
    invoke("cargo", &["fmt", "--all"], root, verbose)?;
    if output("clang-format", &["--version"], root).is_ok() {
        let sources = native_sources(root)?;
        if !sources.is_empty() {
            stage("Format native sources");
            let mut args = vec!["-i"];
            args.extend(sources.iter().map(String::as_str));
            invoke("clang-format", &args, root, verbose)?;
        }
    }
    success("Formatting complete");
    Ok(())
}

/// Photon's own C, C++ and Objective-C files, including new ones not yet
/// added. Submodules and ignored build output are not Photon's to format.
fn native_sources(root: &Path) -> Result<Vec<String>, String> {
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
