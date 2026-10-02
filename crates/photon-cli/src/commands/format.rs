use std::path::Path;

use crate::support::{invoke, output, stage, success};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Format Photon Rust sources");
    invoke(
        "cargo",
        &[
            "fmt",
            "--package",
            "photon-cli",
            "--package",
            "photon-core",
            "--package",
            "photon-app",
            "--package",
            "photon-presentation-broker",
            "--package",
            "photon-omnibox",
        ],
        root,
        verbose,
    )?;
    if output("clang-format", &["--version"], root).is_ok() {
        stage("Format native bridge sources");
        invoke(
            "clang-format",
            &[
                "-i",
                "native/embedder/PhotonEmbedderBridge.h",
                "native/embedder/PhotonEmbedderBridge.cpp",
                "native/presentation/PhotonPresentationXpc.m",
            ],
            root,
            verbose,
        )?;
    }
    success("Formatting complete");
    Ok(())
}
