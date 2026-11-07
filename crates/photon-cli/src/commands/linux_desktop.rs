use std::{
    env,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use photon_brand::{APP_ICON_PNG, LINUX_DESKTOP_ID, NAME};

const DESKTOP_ENTRY: &str = LINUX_DESKTOP_ID;
const MIME_TYPES: &str =
    "x-scheme-handler/http;x-scheme-handler/https;text/html;application/xhtml+xml;";
const PACKAGE_PREFIX: &str = "@PHOTON_PREFIX@";
const PACKAGE_REGISTRATION_SCRIPT: &str =
    include_str!("../../../../packaging/linux/photon-desktop");

enum DesktopLaunch<'a> {
    DevelopmentCli { cli: &'a Path, icon: &'a Path },
    Package { executable: &'a Path },
}

pub(crate) fn install(workspace_root: &Path) -> Result<(), String> {
    let data_home = xdg_data_home()?;
    let cargo_root = cargo_install_root(workspace_root)?;
    let executable = cargo_root.join("bin/photon");
    let executable = absolute_path(&executable, workspace_root)?;
    if !executable.is_file() {
        return Err(format!(
            "the Photon CLI was not found at {}; run `photon install` first or check Cargo's install root",
            executable.display()
        ));
    }
    let applications = data_home.join("applications");
    let icon = absolute_path(&data_home.join(icon_file()), workspace_root)?;

    std::fs::create_dir_all(&applications)
        .map_err(|error| format!("cannot create {}: {error}", applications.display()))?;
    if let Some(icon_directory) = icon.parent() {
        std::fs::create_dir_all(icon_directory)
            .map_err(|error| format!("cannot create {}: {error}", icon_directory.display()))?;
    }

    std::fs::write(&icon, APP_ICON_PNG)
        .map_err(|error| format!("cannot install Photon icon at {}: {error}", icon.display()))?;
    let desktop_entry = desktop_entry(DesktopLaunch::DevelopmentCli {
        cli: &executable,
        icon: &icon,
    })?;
    let desktop_path = applications.join(DESKTOP_ENTRY);
    std::fs::write(&desktop_path, desktop_entry).map_err(|error| {
        format!(
            "cannot install Photon desktop entry at {}: {error}",
            desktop_path.display()
        )
    })?;

    update_desktop_database(&applications);
    println!("Installed Linux desktop entry: {}", desktop_path.display());
    println!("Installed Linux app icon: {}", icon.display());
    Ok(())
}

pub(crate) fn uninstall(workspace_root: &Path) -> Result<(), String> {
    uninstall_registration(
        &xdg_data_home()?,
        &cargo_install_root(workspace_root)?.join("bin/photon"),
    )
}

fn uninstall_registration(data_home: &Path, executable: &Path) -> Result<(), String> {
    let applications = data_home.join("applications");
    let desktop_path = applications.join(DESKTOP_ENTRY);
    let icon_path = data_home.join(icon_file());

    if desktop_path.exists() {
        let contents = std::fs::read_to_string(&desktop_path)
            .map_err(|error| format!("cannot read {}: {error}", desktop_path.display()))?;
        let expected = format!("TryExec={}", desktop_key_value(executable)?);
        if !contents.lines().any(|line| line == expected) {
            println!(
                "{} belongs to another installation; leaving it in place",
                desktop_path.display()
            );
            return Ok(());
        }
    }
    remove_if_exists(&desktop_path)?;
    remove_if_exists(&icon_path)?;
    if applications.is_dir() {
        update_desktop_database(&applications);
    }
    println!("Removed Linux desktop entry and icon.");
    Ok(())
}

pub(crate) fn write_package_files(folder: &Path, executable: &Path) -> Result<(), String> {
    let entry = desktop_entry(DesktopLaunch::Package { executable })?;
    let script = PACKAGE_REGISTRATION_SCRIPT.replace("@DESKTOP_ID@", DESKTOP_ENTRY);
    let script_path = folder
        .join("bin")
        .join(format!("{}-desktop", NAME.to_lowercase()));
    for (path, contents) in [
        (
            folder.join(format!("share/applications/{DESKTOP_ENTRY}.in")),
            entry.as_bytes(),
        ),
        (folder.join("share").join(icon_file()), APP_ICON_PNG),
        (script_path.clone(), script.as_bytes()),
    ] {
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
        }
        std::fs::write(&path, contents)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("cannot make {} executable: {error}", script_path.display()))
}

fn icon_file() -> PathBuf {
    let icon_name = DESKTOP_ENTRY
        .strip_suffix(".desktop")
        .unwrap_or(DESKTOP_ENTRY);
    Path::new("icons/hicolor/1024x1024/apps").join(format!("{icon_name}.png"))
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {}: {error}", path.display())),
    }
}

fn desktop_entry(launch: DesktopLaunch<'_>) -> Result<String, String> {
    let (executable, arguments, icon) = match launch {
        DesktopLaunch::DevelopmentCli { cli, icon } => {
            (cli.to_path_buf(), " run", icon.to_path_buf())
        }
        DesktopLaunch::Package { executable } => {
            let prefix = Path::new(PACKAGE_PREFIX);
            (
                prefix.join(executable),
                "",
                prefix.join("share").join(icon_file()),
            )
        }
    };
    let exec = desktop_exec_argument(&executable)?;
    Ok(format!(
        "[Desktop Entry]\n\
         Version=1.5\n\
         Type=Application\n\
         Name={NAME}\n\
         GenericName=Web Browser\n\
         Comment=Browse the web with Photon\n\
         Exec={exec}{arguments} %u\n\
         TryExec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType={MIME_TYPES}\n\
         StartupNotify=true\n",
        desktop_key_value(&executable)?,
        desktop_key_value(&icon)?
    ))
}

fn desktop_exec_argument(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or_else(|| {
        format!(
            "the Photon executable path is not valid UTF-8: {}",
            path.display()
        )
    })?;
    let mut escaped = String::with_capacity(path.len() + 2);
    escaped.push('"');
    for character in path.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '`' => escaped.push_str("\\`"),
            '$' => escaped.push_str("\\$"),
            '%' => escaped.push_str("%%"),
            character if character.is_control() => {
                return Err("the Photon executable path contains a control character".into());
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    Ok(escaped)
}

fn desktop_key_value(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or_else(|| {
        format!(
            "the Photon executable path is not valid UTF-8: {}",
            path.display()
        )
    })?;
    let mut escaped = String::with_capacity(path.len());
    for character in path.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            character if character.is_control() => {
                return Err("the Photon executable path contains a control character".into());
            }
            character => escaped.push(character),
        }
    }
    Ok(escaped)
}

fn xdg_data_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(format!(
                "XDG_DATA_HOME must be an absolute path; got {}",
                path.display()
            ));
        }
        return Ok(path);
    }

    let home = env::var_os("HOME").ok_or_else(|| {
        "HOME is not set, so Photon cannot choose an XDG data directory".to_owned()
    })?;
    let path = PathBuf::from(home).join(".local/share");
    if !path.is_absolute() {
        return Err(format!(
            "HOME must be an absolute path to choose an XDG data directory; got {}",
            path.display()
        ));
    }
    Ok(path)
}

fn cargo_install_root(workspace_root: &Path) -> Result<PathBuf, String> {
    if let Some(root) = env::var_os("CARGO_INSTALL_ROOT") {
        return absolute_path(&PathBuf::from(root), workspace_root);
    }

    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .ok_or_else(|| "neither CARGO_INSTALL_ROOT, CARGO_HOME nor HOME is set".to_owned())?;
    let cargo_home = absolute_path(&cargo_home, workspace_root)?;
    let install_package = workspace_root.join("crates/photon-cli");

    if let Some(configured_root) = configured_install_root(&install_package, &cargo_home)? {
        let (path, base) = configured_root;
        return absolute_path(&path, &base);
    }
    Ok(cargo_home)
}

fn configured_install_root(
    workspace_root: &Path,
    cargo_home: &Path,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    for directory in workspace_root.ancestors() {
        if let Some(root) = install_root_in_directory(directory)? {
            return Ok(Some(root));
        }
    }
    cargo_home_install_root(cargo_home)
}

fn install_root_in_directory(directory: &Path) -> Result<Option<(PathBuf, PathBuf)>, String> {
    for path in [
        directory.join(".cargo/config.toml"),
        directory.join(".cargo/config"),
    ] {
        if let Some(root) = install_root_in_config(&path, directory)? {
            return Ok(Some(root));
        }
    }
    Ok(None)
}

fn install_root_in_config(
    path: &Path,
    relative_to: &Path,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot read Cargo config {}: {error}",
                path.display()
            ));
        }
    };
    let config = contents
        .parse::<toml::Value>()
        .map_err(|error| format!("cannot parse Cargo config {}: {error}", path.display()))?;
    if let Some(value) = config
        .get("install")
        .and_then(|install| install.get("root"))
    {
        let root = value.as_str().ok_or_else(|| {
            format!(
                "Cargo install.root in {} must be a path string",
                path.display()
            )
        })?;
        return Ok(Some((PathBuf::from(root), relative_to.to_path_buf())));
    }
    Ok(None)
}

fn cargo_home_install_root(cargo_home: &Path) -> Result<Option<(PathBuf, PathBuf)>, String> {
    let relative_to = cargo_home.parent().unwrap_or(cargo_home);
    for filename in ["config.toml", "config"] {
        let path = cargo_home.join(filename);
        if let Some(root) = install_root_in_config(&path, relative_to)? {
            return Ok(Some(root));
        }
    }
    Ok(None)
}

fn absolute_path(path: &Path, relative_to: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let base = if relative_to.is_absolute() {
            relative_to.to_path_buf()
        } else {
            env::current_dir()
                .map_err(|error| error.to_string())?
                .join(relative_to)
        };
        Ok(base.join(path))
    }
}

fn update_desktop_database(applications: &Path) {
    match Command::new("update-desktop-database")
        .arg(applications)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!(
            "Warning: update-desktop-database exited with {status}; desktop launchers may take time to discover Photon."
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => eprintln!(
            "Warning: update-desktop-database is unavailable; install desktop-file-utils to refresh Linux application metadata."
        ),
        Err(error) => eprintln!("Warning: cannot update Linux application metadata: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_keeps_mime_types_and_startup_notification_separate() {
        let entry = desktop_entry(DesktopLaunch::DevelopmentCli {
            cli: Path::new("/opt/photon/bin/photon"),
            icon: Path::new("/opt/photon/icon.png"),
        })
        .unwrap();
        let values = entry
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(values.get("MimeType"), Some(&MIME_TYPES));
        assert_eq!(values.get("StartupNotify"), Some(&"true"));
    }

    #[test]
    fn packaged_registration_belongs_to_one_extracted_folder() {
        let root = temporary_directory("registration");
        let data = root.join("data");
        let older = root.join("Photon 0.1.0/photon one");
        let newer = root.join("Photon 0.2.0 & more");
        let entry = data.join("applications").join(DESKTOP_ENTRY);

        for folder in [&older, &newer] {
            write_package_files(folder, Path::new("bin/Photon")).unwrap();
            assert_registration(folder, "install", &data);
            let folder = folder.canonicalize().unwrap();
            let contents = std::fs::read_to_string(&entry).unwrap();
            assert!(contents.contains(&format!("\nExec=\"{}/bin/Photon\" %u\n", folder.display())));
            assert!(contents.contains(&format!("\nTryExec={}/bin/Photon\n", folder.display())));
            assert!(contents.contains(&format!(
                "\nIcon={}/share/{}\n",
                folder.display(),
                icon_file().display()
            )));
        }

        assert_registration(&older, "uninstall", &data);
        assert!(
            std::fs::read_to_string(&entry)
                .unwrap()
                .contains("Photon 0.2.0 & more"),
            "an older folder must not remove a newer registration"
        );
        assert_registration(&newer, "uninstall", &data);
        assert!(!entry.exists());
        assert_registration(&newer, "uninstall", &data);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_registration_rejects_characters_it_cannot_quote() {
        let root = temporary_directory("registration-rejected");
        let data = root.join("data");
        for name in [
            "photon $HOME",
            "photon \"quoted\"",
            "photon 100%",
            "photon \\ path",
        ] {
            let folder = root.join(name);
            write_package_files(&folder, Path::new("bin/Photon")).unwrap();
            let output = registration(&folder, "install", &data);
            assert!(!output.status.success(), "{name} must be rejected");
            assert!(String::from_utf8_lossy(&output.stderr).contains("cannot register"));
        }
        assert!(!data.join("applications").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn developer_uninstall_preserves_packaged_registration() {
        let data = temporary_directory("uninstall-ownership");
        let entry = data.join("applications").join(DESKTOP_ENTRY);
        let icon = data.join(icon_file());
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::create_dir_all(icon.parent().unwrap()).unwrap();
        let packaged = desktop_entry(DesktopLaunch::Package {
            executable: Path::new("bin/Photon"),
        })
        .unwrap();
        std::fs::write(&entry, &packaged).unwrap();
        std::fs::write(&icon, b"icon").unwrap();
        let executable = Path::new("/cargo/bin/photon");
        uninstall_registration(&data, executable).unwrap();
        assert_eq!(std::fs::read_to_string(&entry).unwrap(), packaged);
        assert!(icon.exists());
        std::fs::write(
            &entry,
            desktop_entry(DesktopLaunch::DevelopmentCli {
                cli: executable,
                icon: &icon,
            })
            .unwrap(),
        )
        .unwrap();
        uninstall_registration(&data, executable).unwrap();
        assert!(!entry.exists());
        assert!(!icon.exists());
        std::fs::remove_dir_all(data).unwrap();
    }

    fn assert_registration(folder: &Path, action: &str, data: &Path) {
        let output = registration(folder, action, data);
        assert!(
            output.status.success(),
            "{action} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn registration(folder: &Path, action: &str, data: &Path) -> std::process::Output {
        let script = folder
            .join("bin")
            .join(format!("{}-desktop", NAME.to_lowercase()));
        Command::new("sh")
            .arg(script)
            .arg(action)
            .env("XDG_DATA_HOME", data)
            .output()
            .unwrap()
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!("photon-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
