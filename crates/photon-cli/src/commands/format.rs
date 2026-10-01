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
            "photon-gpui",
            "--package",
            "photon-native-addon",
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
                "native/gpui/PhotonEmbedderBridge.h",
                "native/gpui/PhotonEmbedderBridge.cpp",
            ],
            root,
            verbose,
        )?;
    }
    invoke("bun", &["run", "format"], &root.join("ui"), verbose)?;
    Ok(())
}
