use std::path::Path;

use crate::support::{invoke_with_environment, success, with_progress};

pub(crate) fn run(root: &Path, verbose: bool) -> Result<(), String> {
    let engine_dir = root.join("build").join("engine-release");
    let library_path = crate::commands::build::runtime_library_path(&engine_dir);
    let library_variable = crate::commands::build::runtime_library_variable();
    with_progress("Run Rust workspace tests", !verbose, || {
        invoke_with_environment(
            "cargo",
            &["test", "--workspace", "--release"],
            root,
            verbose,
            &[
                (library_variable, library_path),
                ("PHOTON_ENGINE_BUILD_DIR", engine_dir.display().to_string()),
            ],
        )
    })?;
    success("Rust workspace tests passed");
    Ok(())
}
