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
    arguments: impl IntoIterator<Item = OsString>,
) -> Option<String> {
    if let Some(address) = environment
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
    {
        return Some(address);
    }
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--automation-socket" {
            arguments.next();
            continue;
        }
        if argument == "--" {
            return arguments.next()?.into_string().ok();
        }
        let Some(argument) = argument.to_str() else {
            continue;
        };
        if !argument.starts_with('-') && !argument.trim().is_empty() {
            return Some(argument.trim().to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{resolve_helper_directory, startup_address};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::Path};

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
        let url = "https://example.org/a?q=one%20two&b=3";
        assert_eq!(startup_address(None, [url.into()]), Some(url.into()));
        assert_eq!(startup_address(None, []), None);
        assert_eq!(startup_address(Some("  ".into()), []), None);
    }

    #[test]
    fn development_url_contract_is_preserved() {
        assert_eq!(
            startup_address(
                Some(" https://developer.example/ ".into()),
                ["https://other.example/".into()]
            ),
            Some("https://developer.example/".into())
        );
        assert_eq!(
            startup_address(Some(" ".into()), ["https://example.org/".into()]),
            Some("https://example.org/".into())
        );
    }

    #[test]
    fn automation_flags_and_the_socket_path_are_not_addresses() {
        assert_eq!(
            startup_address(None, ["--automation-socket".into(), "/tmp/socket".into()]),
            None
        );
        assert_eq!(
            startup_address(
                None,
                [
                    "--automation-socket".into(),
                    "/tmp/socket".into(),
                    "https://example.org/".into()
                ]
            ),
            Some("https://example.org/".into())
        );
        assert_eq!(
            startup_address(None, ["--automation-socket=/tmp/socket".into()]),
            None
        );
    }

    #[test]
    fn non_utf8_arguments_do_not_panic() {
        assert_eq!(
            startup_address(None, [OsString::from_vec(vec![0xff])]),
            None
        );
        let executable = Path::new("/tmp")
            .join(OsString::from_vec(vec![0xff]))
            .join("Photon");
        assert_eq!(
            resolve_helper_directory(None, &executable).unwrap(),
            executable.parent().unwrap()
        );
    }
}
