use std::path::Path;

use crate::support::{invoke, output, stage, success};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Format Photon Rust sources");
    invoke("cargo", &["fmt", "--all"], root, verbose)?;
    if output("clang-format", &["--version"], root).is_ok() {
        stage("Format native bridge sources");
        invoke(
            "clang-format",
            &[
                "-i",
                "crates/photon-ffi/include/photon_ffi.h",
                "native/embedder/PhotonEmbedderBridge.h",
                "native/embedder/PhotonEmbedderBridge.cpp",
                "crates/photon-presentation-ipc/native/PhotonPresentationXpc.m",
            ],
            root,
            verbose,
        )?;
    }
    success("Formatting complete");
    Ok(())
}
