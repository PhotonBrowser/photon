use std::path::Path;

#[cfg(target_os = "macos")]
use crate::support::{command, output, stage, success};

pub(crate) fn install(root: &Path, verbose: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if output("xcode-select", &["-p"], root).is_err() {
            return Err(
                "Xcode Command Line Tools are required; run `xcode-select --install`, then retry setup"
                    .into(),
            );
        }
        if output("brew", &["--version"], root).is_err() {
            return Err("Homebrew is required for automatic macOS package setup; install it from https://brew.sh and retry".into());
        }
        stage("Install Photon macOS build packages with Homebrew");
        command(
            "brew",
            &[
                "install",
                "autoconf",
                "autoconf-archive",
                "automake",
                "ccache",
                "cmake",
                "libtool",
                "nasm",
                "ninja",
                "pkg-config",
                "python",
                "qt",
            ],
            root,
            verbose,
        )?;
        success("macOS build packages are ready");
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (root, verbose);
        Err("automatic system package installation is currently supported on macOS with Homebrew; follow CONTRIBUTOR_SETUP.md for Linux/WSL2 packages".into())
    }
}
