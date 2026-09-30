use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::{OwoColorize, Stream};

pub(crate) fn path(value: &Path) -> String {
    value.to_string_lossy().into_owned()
}

pub(crate) fn stage(text: &str) {
    println!(
        "{} {text}",
        "›".if_supports_color(Stream::Stdout, |symbol| format!("{}", symbol.bright_cyan()))
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
        return Err(String::from_utf8_lossy(&result.stderr).trim().to_owned());
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
    let stderr = String::from_utf8_lossy(&result.stderr);
    Err(format!(
        "{program} failed: {}",
        stderr
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    ))
}
