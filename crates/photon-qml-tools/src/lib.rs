use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Output from `qmllint`, including warnings when lint exits successfully.
pub struct LintOutput {
    pub diagnostics: String,
    pub succeeded: bool,
}

/// Formats the repository's QML files with the Qt formatter.
pub fn format(root: &Path, verbose: bool) -> Result<(), String> {
    let formatter = qt_tool("qmlformat", root)?;
    run(
        &formatter,
        &[
            "-i",
            "ui/Main.qml",
            "ui/PhotonPage.qml",
            "ui/browser/Omnibox.qml",
            "ui/components/Button.qml",
            "ui/components/Icon.qml",
            "ui/components/IconButton.qml",
            "ui/components/PressableControl.qml",
            "ui/chrome/TitleBar.qml",
            "ui/pages/WebSurface.qml",
            "ui/theme/Theme.qml",
        ],
        root,
        verbose,
    )
}

/// Lints the repository's QML files, loading generated module metadata when available.
pub fn lint(root: &Path) -> Result<LintOutput, String> {
    let qmllint = qt_tool("qmllint", root)?;
    let qtpaths = qt_tool("qtpaths6", root)?;
    let qt_version = output(&qtpaths, &["--qt-version"], root).map_err(|_| {
        "Qt 6 development tools are missing; install Qt Quick, QML, and Quick Controls development packages".to_owned()
    })?;
    if !version_at_least(&qt_version, (6, 9)) {
        return Err(format!(
            "Qt 6.9 or newer is required; found {}",
            qt_version.trim()
        ));
    }

    let mut args = vec!["--ignore-settings".to_owned()];
    let build_dir = root.join("build/app-debug");
    let generated_qmldir = build_dir.join("Photon/qmldir");
    let generated_main = build_dir.join("Photon/Main.qml");
    let generated_page = build_dir.join("Photon/PhotonPage.qml");
    let generated_omnibox = build_dir.join("Photon/browser/Omnibox.qml");
    let generated_icon_button = build_dir.join("Photon/IconButton.qml");
    let generated_button = build_dir.join("Photon/Button.qml");
    let generated_icon = build_dir.join("Photon/Icon.qml");
    let generated_pressable_control = build_dir.join("Photon/PressableControl.qml");
    let generated_title_bar = build_dir.join("Photon/TitleBar.qml");
    let generated_web_surface = build_dir.join("Photon/WebSurface.qml");
    let generated_theme = build_dir.join("Photon/theme/Theme.qml");
    let generated_resource = build_dir.join(".qt/rcc/photon_raw_qml_0.qrc");
    if generated_qmldir.is_file() {
        args.extend([
            "-I".into(),
            path(&build_dir),
            "-I".into(),
            path(&build_dir.join("Photon")),
            "-i".into(),
            path(&generated_qmldir),
        ]);
    }
    if generated_resource.is_file() {
        args.extend(["--resource".into(), path(&generated_resource)]);
    }
    // CMake copies QML module files into the build tree. Use those copies so
    // qmllint sees every module type and the current singleton properties.
    if generated_main.is_file()
        && generated_page.is_file()
        && generated_omnibox.is_file()
        && generated_icon_button.is_file()
        && generated_button.is_file()
        && generated_icon.is_file()
        && generated_pressable_control.is_file()
        && generated_title_bar.is_file()
        && generated_web_surface.is_file()
        && generated_theme.is_file()
    {
        args.extend([
            path(&generated_main),
            path(&generated_button),
            path(&generated_icon),
            path(&generated_pressable_control),
            path(&generated_icon_button),
            path(&generated_page),
            path(&generated_omnibox),
            path(&generated_title_bar),
            path(&generated_web_surface),
            path(&generated_theme),
        ]);
    } else {
        args.extend([
            "ui/Main.qml".into(),
            "ui/PhotonPage.qml".into(),
            "ui/browser/Omnibox.qml".into(),
            "ui/components/Button.qml".into(),
            "ui/components/Icon.qml".into(),
            "ui/components/IconButton.qml".into(),
            "ui/components/PressableControl.qml".into(),
            "ui/chrome/TitleBar.qml".into(),
            "ui/pages/WebSurface.qml".into(),
            "ui/theme/Theme.qml".into(),
        ]);
    }
    let string_args: Vec<_> = args.iter().map(String::as_str).collect();
    let result = Command::new(qmllint)
        .args(&string_args)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("cannot run qmllint: {error}"))?;
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(LintOutput {
        diagnostics,
        succeeded: result.status.success(),
    })
}

/// Returns a Qt executable path when Qt's host tools directory can be found.
pub fn tool_path(tool: &str, root: &Path) -> Option<String> {
    qt_tool_directory(root)
        .ok()
        .and_then(|directory| find_tool(tool, &directory))
}

fn qt_tool(tool: &str, root: &Path) -> Result<String, String> {
    tool_path(tool, root)
        .ok_or_else(|| format!("{tool} is missing; install the Qt 6 QML development tools"))
}

fn qt_tool_directory(root: &Path) -> Result<PathBuf, String> {
    let qtpaths =
        find_tool("qtpaths6", Path::new("/nonexistent")).unwrap_or_else(|| "qtpaths6".into());
    if let Ok(prefix) = output(&qtpaths, &["--query", "QT_HOST_BINS"], root) {
        let directory = PathBuf::from(prefix.trim());
        if directory.is_dir() {
            return Ok(directory);
        }
    }
    for directory in [
        "/usr/lib/qt6/bin",
        "/usr/lib64/qt6/bin",
        "/usr/lib/x86_64-linux-gnu/qt6/bin",
    ] {
        let directory = PathBuf::from(directory);
        if directory.join("qtpaths6").is_file() {
            return Ok(directory);
        }
    }
    Err(
        "Qt 6 tools are missing; install Qt Quick, QML, and Quick Controls development packages"
            .into(),
    )
}

fn find_tool(tool: &str, qt_bin: &Path) -> Option<String> {
    let executable = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    };
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            let candidate = directory.join(&executable);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    let candidate = qt_bin.join(executable);
    candidate
        .is_file()
        .then(|| candidate.to_string_lossy().into_owned())
}

fn run(program: &str, args: &[&str], root: &Path, verbose: bool) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(root);
    if verbose {
        return command
            .status()
            .map_err(|error| error.to_string())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("{program} exited with {status}"))
                }
            });
    }
    let result = command
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ))
    }
}

fn output(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let result = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| error.to_string())?;
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).trim().to_owned())
    }
}

fn path(value: &Path) -> String {
    value.to_string_lossy().into_owned()
}

fn version_at_least(version: &str, minimum: (u32, u32)) -> bool {
    let parts: Vec<u32> = version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .take(2)
        .collect();
    (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
    ) >= minimum
}
