use std::path::Path;

use crate::support::{command, stage, success};

pub(crate) fn install(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Install the Photon command in Cargo's bin directory");
    command(
        "cargo",
        &[
            "install",
            "--path",
            "crates/photon-cli",
            "--bin",
            "photon",
            "--locked",
            "--force",
        ],
        root,
        verbose,
    )?;
    success("Photon is installed; run `photon` from this checkout");
    Ok(())
}
