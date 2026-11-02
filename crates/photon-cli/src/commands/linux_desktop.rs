use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use photon_brand::{APP_ICON_PNG, LINUX_DESKTOP_ID, NAME};

const DESKTOP_ENTRY: &str = LINUX_DESKTOP_ID;
const MIME_TYPES: &str =
    "x-scheme-handler/http;x-scheme-handler/https;text/html;application/xhtml+xml;";

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
    let icon_name = DESKTOP_ENTRY
        .strip_suffix(".desktop")
        .unwrap_or(DESKTOP_ENTRY);
    let icon = data_home
        .join("icons/hicolor/1024x1024/apps")
        .join(format!("{icon_name}.png"));
    let icon = absolute_path(&icon, workspace_root)?;

    std::fs::create_dir_all(&applications)
        .map_err(|error| format!("cannot create {}: {error}", applications.display()))?;
    if let Some(icon_directory) = icon.parent() {
        std::fs::create_dir_all(icon_directory)
            .map_err(|error| format!("cannot create {}: {error}", icon_directory.display()))?;
    }

    std::fs::write(&icon, APP_ICON_PNG)
        .map_err(|error| format!("cannot install Photon icon at {}: {error}", icon.display()))?;
    let desktop_entry = desktop_entry(&executable, &icon)?;
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

pub(crate) fn uninstall() -> Result<(), String> {
    let data_home = xdg_data_home()?;
    let applications = data_home.join("applications");
    let icon_name = DESKTOP_ENTRY
        .strip_suffix(".desktop")
        .unwrap_or(DESKTOP_ENTRY);
    let desktop_path = applications.join(DESKTOP_ENTRY);
    let icon_path = data_home
        .join("icons/hicolor/1024x1024/apps")
        .join(format!("{icon_name}.png"));

    remove_if_exists(&desktop_path)?;
    remove_if_exists(&icon_path)?;
    if applications.is_dir() {
        update_desktop_database(&applications);
    }
    println!("Removed Linux desktop entry and icon.");
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {}: {error}", path.display())),
    }
}

fn desktop_entry(executable: &Path, icon: &Path) -> Result<String, String> {
    let exec = desktop_exec_argument(executable)?;
    Ok(format!(
        "[Desktop Entry]\n\
         Version=1.5\n\
         Type=Application\n\
         Name={NAME}\n\
         GenericName=Web Browser\n\
         Comment=Browse the web with Photon\n\
         Exec={exec} run %u\n\
         TryExec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType={MIME_TYPES}\
         StartupNotify=true\n",
        desktop_key_value(executable)?,
        desktop_key_value(icon)?
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
