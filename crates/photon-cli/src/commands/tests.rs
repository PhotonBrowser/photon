use std::path::Path;

use crate::support::{invoke, invoke_with_environment, with_progress};

pub(crate) fn run(root: &Path, verbose: bool) -> Result<(), String> {
    let engine_dir = root.join("build").join("engine-debug");
    let library_path = crate::commands::build::runtime_library_path(&engine_dir);
    with_progress("Run Rust workspace tests", !verbose, || {
        invoke_with_environment(
            "cargo",
            &["test", "--workspace"],
            root,
            verbose,
            &[("LD_LIBRARY_PATH", library_path)],
        )
    })?;
    with_progress("Test GPUIX external custom elements", !verbose, || {
        invoke(
            "cargo",
            &[
                "test",
                "--manifest-path",
                "vendor/gpuix/packages/native/Cargo.toml",
                "--lib",
                "extension_api_tests",
            ],
            root,
            verbose,
        )
    })
}
