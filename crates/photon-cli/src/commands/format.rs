use std::path::Path;

use crate::support::{invoke, output};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
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
    Ok(())
}
