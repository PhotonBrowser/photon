use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(not(feature = "ui-only"))]
pub(crate) fn helper_directory() -> anyhow::Result<PathBuf> {
    resolve_helper_directory(
        std::env::var_os("PHOTON_HELPER_DIRECTORY"),
        &std::env::current_exe()?,
    )
}

fn resolve_helper_directory(
    override_directory: Option<OsString>,
    executable: &Path,
) -> anyhow::Result<PathBuf> {
    if let Some(directory) = override_directory {
        anyhow::ensure!(!directory.is_empty(), "PHOTON_HELPER_DIRECTORY is empty");
        return Ok(PathBuf::from(directory));
    }
    executable
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("cannot locate Engine helpers beside the Photon executable"))
}

pub(crate) fn startup_address(
    environment: Option<String>,
    argument: Option<String>,
) -> Option<String> {
    environment
        .into_iter()
        .chain(argument)
        .map(|address| address.trim().to_owned())
        .find(|address| !address.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{resolve_helper_directory, startup_address};
    use std::path::Path;

    #[test]
    fn packaged_helpers_follow_the_executable() {
        let directory =
            resolve_helper_directory(None, Path::new("/tmp/package with spaces/bin/Photon"))
                .unwrap();
        assert_eq!(directory, Path::new("/tmp/package with spaces/bin"));
    }

    #[test]
    fn development_helper_override_is_preserved() {
        let directory = resolve_helper_directory(
            Some("/checkout/build/engine-debug/bin".into()),
            Path::new("/checkout/target/debug/Photon"),
        )
        .unwrap();
        assert_eq!(directory, Path::new("/checkout/build/engine-debug/bin"));
        assert!(resolve_helper_directory(Some("".into()), Path::new("/bin/Photon")).is_err());
    }

    #[test]
    fn desktop_url_is_used_without_a_developer_override() {
        assert_eq!(
            startup_address(None, Some("https://example.org/a?q=one%20two&b=3".into())),
            Some("https://example.org/a?q=one%20two&b=3".into())
        );
        assert_eq!(startup_address(None, None), None);
        assert_eq!(startup_address(Some("  ".into()), None), None);
    }

    #[test]
    fn development_url_contract_is_preserved() {
        assert_eq!(
            startup_address(
                Some(" https://developer.example/ ".into()),
                Some("https://other.example/".into())
            ),
            Some("https://developer.example/".into())
        );
        assert_eq!(
            startup_address(Some(" ".into()), Some("https://example.org/".into())),
            Some("https://example.org/".into())
        );
    }
}
