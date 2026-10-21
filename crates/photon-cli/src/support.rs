use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::{OwoColorize, Stream};

pub(crate) const CLANG_FORMAT_VERSION: &str = "22.1.8";

pub(crate) fn path(value: &Path) -> String {
    value.to_string_lossy().into_owned()
}

pub(crate) fn stage(text: &str) {
    println!(
        "{} {text}",
        "›".if_supports_color(Stream::Stdout, |symbol| format!("{}", symbol.bright_cyan()))
    );
}

pub(crate) fn success(text: &str) {
    println!(
        "{} {text}",
        "✓".if_supports_color(Stream::Stdout, |symbol| format!("{}", symbol.green()))
    );
}

pub(crate) fn with_progress<T>(
    message: &str,
    enabled: bool,
    task: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if !enabled {
        return task();
    }

    let progress = ProgressBar::new_spinner();
    let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner());
    progress.set_style(style);
    progress.set_message(message.to_owned());
    progress.enable_steady_tick(Duration::from_millis(100));

    match task() {
        Ok(value) => {
            progress.finish_with_message(format!("✓ {message}"));
            Ok(value)
        }
        Err(error) => {
            progress.finish_with_message(format!("✗ {message}"));
            Err(error)
        }
    }
}

pub(crate) fn require_tool(tool: &str) -> Result<(), String> {
    output(tool, &["--version"], Path::new("."))
        .map(|_| ())
        .map_err(|_| format!("{tool} is missing; install it using your OS package manager"))
}

pub(crate) fn clang_format(root: &Path, version: &str) -> Result<PathBuf, String> {
    let executable_name = if cfg!(windows) {
        "clang-format.exe"
    } else {
        "clang-format"
    };
    let major = version
        .split('.')
        .next()
        .and_then(|major| major.parse::<u32>().ok())
        .ok_or_else(|| format!("invalid clang-format version: {version}"))?;

    let versioned_executable = if cfg!(windows) {
        format!("clang-format-{major}.exe")
    } else {
        format!("clang-format-{major}")
    };
    let candidates = [versioned_executable, executable_name.to_owned()];
    for candidate in candidates {
        if let Ok(executable_path) = find_on_path(&candidate)
            && clang_format_major(&executable_path, root) == Some(major)
        {
            return Ok(executable_path);
        }
    }

    if cfg!(target_os = "macos") {
        let formula = format!("llvm@{major}");
        if let Ok(prefix) = output("brew", &["--prefix", &formula], root) {
            let executable_path = PathBuf::from(prefix.trim())
                .join("bin")
                .join(executable_name);
            if clang_format_major(&executable_path, root) == Some(major) {
                return Ok(executable_path);
            }
        }
    }

    let install_root = root.join("build/tools/clang-format").join(version);
    let relative_bin = if cfg!(windows) { "Scripts" } else { "bin" };
    let executables = [
        install_root.join(relative_bin).join(executable_name),
        install_root
            .join("local")
            .join(relative_bin)
            .join(executable_name),
    ];
    for executable in &executables {
        if executable.is_file() && clang_format_major(executable, root) == Some(major) {
            return Ok(executable.clone());
        }
    }

    stage(&format!("Install clang-format {version} for this checkout"));
    let python = if cfg!(windows) { "python" } else { "python3" };
    invoke(
        python,
        &[
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--no-warn-script-location",
            "--prefix",
            &path(&install_root),
            &format!("clang-format=={version}"),
        ],
        root,
        false,
    )
    .map_err(|error| {
        format!(
            "could not install clang-format {version}; install Python 3 with pip or install clang-format {major} using your OS package manager: {error}"
        )
    })?;

    for executable in &executables {
        if executable.is_file() && clang_format_major(executable, root) == Some(major) {
            return Ok(executable.clone());
        }
    }

    Err(format!(
        "clang-format {version} installation completed but a compatible executable was not found under {}",
        install_root.display()
    ))
}

fn clang_format_major(executable: &Path, root: &Path) -> Option<u32> {
    let version = output(&path(executable), &["--version"], root).ok()?;
    let number = version.split_whitespace().find(|part| {
        part.chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    })?;
    number.split('.').next()?.parse().ok()
}

fn find_on_path(executable: &str) -> Result<PathBuf, String> {
    let path_var = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    for directory in std::env::split_paths(&path_var) {
        let candidate = directory.join(executable);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!("{executable} not found on PATH"))
}

pub(crate) fn require_tool_version(
    tool: &str,
    minimum: (u32, u32),
    cwd: &Path,
) -> Result<(), String> {
    let version = output(tool, &["--version"], cwd)
        .map_err(|_| format!("{tool} is missing; install it using your OS package manager"))?;
    if !version_at_least(&version, minimum) {
        return Err(format!(
            "{tool} {} or newer is required; found {}",
            minimum.0,
            version.trim()
        ));
    }
    Ok(())
}

pub(crate) fn run_tool(tool: &str, args: &[&str], root: &Path) -> Result<String, String> {
    output(tool, args, root)
}

pub(crate) fn compiler_version(root: &Path) -> Option<String> {
    for (compiler, args) in [
        ("c++", &["--version"][..]),
        ("clang++", &["--version"]),
        ("g++", &["--version"]),
        ("cl", &["/Bv"]),
    ] {
        if let Ok(version) = output(compiler, args, root) {
            return Some(version.lines().next().unwrap_or("unknown compiler").into());
        }
    }
    None
}

pub(crate) fn version_at_least(version: &str, minimum: (u32, u32)) -> bool {
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

pub(crate) fn output(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let result = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if !result.status.success() {
        let diagnostics = diagnostics(&result.stdout, &result.stderr, 12);
        return Err(format!(
            "{program} failed ({}): {diagnostics}",
            result.status
        ));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

pub(crate) fn command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    verbose: bool,
) -> Result<(), String> {
    invoke(program, args, cwd, verbose)
}

pub(crate) fn invoke(
    program: &str,
    args: &[&str],
    cwd: &Path,
    verbose: bool,
) -> Result<(), String> {
    invoke_with_environment(program, args, cwd, verbose, &[])
}

pub(crate) fn invoke_with_environment(
    program: &str,
    args: &[&str],
    cwd: &Path,
    verbose: bool,
    environment: &[(&str, String)],
) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    for (name, value) in environment {
        cmd.env(name, value);
    }
    if verbose {
        return cmd.status().map_err(|e| e.to_string()).and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("{program} exited with {s}"))
            }
        });
    }
    let result = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if result.status.success() {
        return Ok(());
    }
    Err(format!(
        "{program} failed ({}): {}",
        result.status,
        diagnostics(&result.stdout, &result.stderr, 12)
    ))
}

pub(crate) fn diagnostics(stdout: &[u8], stderr: &[u8], limit: usize) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let source = if stderr.trim().is_empty() {
        &stdout
    } else {
        &stderr
    };
    let lines: Vec<_> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return "no diagnostics were produced".into();
    }
    let start = lines.len().saturating_sub(limit);
    let mut tail = lines[start..].join("\n");
    if let Some(first_error) = lines[..start]
        .iter()
        .position(|line| line.starts_with("error") || line.starts_with("Error"))
    {
        let context_end = (first_error + 5).min(start);
        let context = lines[first_error..context_end].join("\n");
        tail = format!("{context}\n…\n{tail}");
    }
    if start > 0 {
        tail = format!("… {start} earlier lines omitted\n{tail}");
    }
    tail
}
