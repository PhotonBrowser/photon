use std::path::Path;

use crate::support::{invoke_with_environment, success, with_progress};

pub(crate) fn run(root: &Path, verbose: bool) -> Result<(), String> {
    let engine_dir = root.join("build").join("engine-debug");
    let library_path = crate::commands::build::runtime_library_path(&engine_dir);
    let library_variable = if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    };
    with_progress("Run Rust workspace tests", !verbose, || {
        invoke_with_environment(
            "cargo",
            &["test", "--workspace"],
            root,
            verbose,
            &[(library_variable, library_path)],
        )
    })?;
    success("Rust workspace tests passed");
    Ok(())
}
