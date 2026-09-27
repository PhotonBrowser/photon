use std::path::Path;

use crate::support::{invoke, output};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    invoke("cargo", &["fmt", "--all"], root, verbose)?;
    if output("clang-format", &["--version"], root).is_ok() {
        invoke(
            "clang-format",
            &[
                "-i",
                "native/qt/main.cpp",
                "native/qt/PhotonWebView.h",
                "native/qt/PhotonWebView.cpp",
            ],
            root,
            verbose,
        )?;
    }
    photon_qml_tools::format(root, verbose)?;
    Ok(())
}
