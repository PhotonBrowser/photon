use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = workspace_root()?;
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    let rest: Vec<_> = args.collect();
    let verbose = rest.iter().any(|arg| arg == "--verbose");
    let release = rest.iter().any(|arg| arg == "--release");
    match command.as_str() {
        "help" | "--help" | "-h" => help(),
        "setup" => setup(&root),
        "doctor" => doctor(&root),
        "build" => build(&root, release, verbose),
        "run" => {
            build(&root, release, verbose)?;
            run_app(&root, release)
        }
        "clean" => clean(&root, rest.get(0).map(String::as_str)),
        "check" => check(&root, verbose),
        "fix-prompt" => {
            print_fix_prompt(&root, "Paste the failing command and its output here.");
            Ok(())
        }
        "format" => format(&root, verbose),
        "test" => test(&root, verbose),
        "engine" => engine(&root, rest.first().map(String::as_str), release, verbose),
        _ => Err(format!("unknown command '{command}'. Run ./photon help.")),
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or("cannot locate Photon workspace".into())
}

fn help() -> Result<(), String> {
    println!(
        "Photon development commands\n\n  setup              Check prerequisites and initialize Engine\n  doctor             Report toolchain and repository state\n  build [--release]  Build engine and Qt Quick shell\n  run [--release]    Build and launch Photon\n  clean [engine]    Remove generated build trees\n  check              Check formatting and architecture boundaries\n  fix-prompt         Print a copyable AI prompt for fixing a check failure\n  format             Format Photon-owned Rust and C++\n  test               Run Rust workspace tests\n  engine status      Show Engine repository state\n  engine build       Build Photon Engine libraries and services\n  engine sync        Merge upstream/master (requires clean engine tree)\n\nAdd --verbose to show underlying build output."
    );
    Ok(())
}

fn setup(root: &Path) -> Result<(), String> {
    let engine = root.join("Engine");
    if !engine.join(".git").exists() {
        command(
            "git",
            &["submodule", "update", "--init", "--recursive"],
            root,
            true,
        )?;
    }
    let submodule_state = output("git", &["submodule", "status", "Engine"], root)?;
    if submodule_state.starts_with(['+', '-']) {
        if !output(
            "git",
            &[
                "status",
                "--porcelain",
                "--ignore-submodules=none",
                "--",
                "Engine",
            ],
            root,
        )?
        .trim()
        .is_empty()
        {
            return Err("Engine is dirty and does not match the pinned submodule commit; preserving its work. Review Engine and update the submodule manually.".into());
        }
        command(
            "git",
            &["submodule", "update", "--init", "--recursive", "Engine"],
            root,
            true,
        )?;
    }
    for tool in ["git", "cargo", "rustc", "cmake", "ninja", "python3"] {
        require_tool(tool)?;
    }
    if compiler_version(root).is_none() {
        return Err(
            "a C++ compiler is missing; install Clang or your platform's C++ build tools".into(),
        );
    }
    for (tool, minimum) in [("cmake", (3, 25)), ("ninja", (1, 10)), ("rustc", (1, 85))] {
        let version = output(tool, &["--version"], root)?;
        if !version_at_least(&version, minimum) {
            return Err(format!(
                "{tool} {} or newer is required; found {}",
                minimum.0,
                version.trim()
            ));
        }
    }
    let qt_bin = qt_tool_directory(root)?;
    let qtpaths = resolve_tool("qtpaths6", Some(&qt_bin)).unwrap_or_else(|| "qtpaths6".into());
    let qt_version = output(&qtpaths, &["--qt-version"], root)
        .map_err(|_| "Qt 6 development tools are missing; install Qt Quick, QML, and Quick Controls development packages".to_owned())?;
    if !version_at_least(&qt_version, (6, 5)) {
        return Err(format!(
            "Qt 6.5 or newer is required; found {}",
            qt_version.trim()
        ));
    }
    for tool in ["qmllint", "qmlformat"] {
        resolve_tool(tool, Some(&qt_bin))
            .ok_or_else(|| format!("{tool} is missing; install the Qt 6 QML development tools"))?;
    }
    if !root.join("Engine/CMakeLists.txt").is_file() {
        return Err(
            "Engine submodule is missing; run git submodule update --init --recursive".into(),
        );
    }
    let upstream = output("git", &["remote", "get-url", "upstream"], &engine);
    if upstream.as_deref().unwrap_or_default().trim_end()
        != "https://github.com/LadybirdBrowser/ladybird.git"
    {
        let action = if upstream.is_ok() { "set-url" } else { "add" };
        command(
            "git",
            &[
                "remote",
                action,
                "upstream",
                "https://github.com/LadybirdBrowser/ladybird.git",
            ],
            &engine,
            true,
        )?;
    }
    for path in ["build/engine-debug", "build/app-debug", "build/bin"] {
        std::fs::create_dir_all(root.join(path)).map_err(|e| e.to_string())?;
    }
    println!(
        "Setup ready. The first engine build prepares Ladybird's pinned dependencies under build/."
    );
    Ok(())
}

fn doctor(root: &Path) -> Result<(), String> {
    println!("Photon environment");
    println!("  OS         {}/{}", env::consts::OS, env::consts::ARCH);
    for (name, args) in [
        ("Rust", vec!["rustc", "--version"]),
        ("Cargo", vec!["cargo", "--version"]),
        ("CMake", vec!["cmake", "--version"]),
        ("Ninja", vec!["ninja", "--version"]),
        ("Qt", vec!["qtpaths6", "--qt-version"]),
    ] {
        let line = run_tool(args[0], &args[1..], root).unwrap_or_else(|_| "not found".into());
        println!("  {name:10} {}", line.lines().next().unwrap_or("unknown"));
    }
    println!(
        "  Compiler   {}",
        compiler_version(root).unwrap_or_else(|| "not found".into())
    );
    println!(
        "  Engine     {}",
        output(
            "git",
            &["rev-parse", "--short", "HEAD"],
            &root.join("Engine")
        )
        .unwrap_or_else(|_| "missing".into())
        .trim()
    );
    println!(
        "  branch     {}",
        output("git", &["branch", "--show-current"], &root.join("Engine"))
            .unwrap_or_else(|_| "unavailable".into())
            .trim()
    );
    println!(
        "  submodule  {}",
        output("git", &["submodule", "status", "Engine"], root)
            .unwrap_or_else(|_| "unavailable".into())
            .trim()
    );
    for (name, args) in [
        ("origin", vec!["remote", "get-url", "origin"]),
        ("upstream", vec!["remote", "get-url", "upstream"]),
    ] {
        println!(
            "  {name:10} {}",
            output("git", &args, &root.join("Engine"))
                .unwrap_or_else(|_| "missing".into())
                .trim()
        );
    }
    println!(
        "  working tree {}",
        if output("git", &["status", "--porcelain"], &root.join("Engine"))
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            "clean"
        } else {
            "dirty"
        }
    );
    println!(
        "  build dir  {}",
        if root.join("build").exists() {
            "present"
        } else {
            "absent"
        }
    );
    for path in ["engine-debug", "app-debug", "vcpkg-debug"] {
        println!(
            "  {path:10} {}",
            if root.join("build").join(path).exists() {
                "present"
            } else {
                "not configured"
            }
        );
    }
    Ok(())
}

fn build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let started = Instant::now();
    engine_build(root, release, verbose)?;
    let source = root.join("native/qt");
    let app_build = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" });
    stage("Configure Photon shell");
    let source_arg = path(&source);
    let build_arg = path(&app_build);
    let build_type = format!(
        "-DCMAKE_BUILD_TYPE={}",
        if release { "Release" } else { "Debug" }
    );
    invoke(
        "cmake",
        &[
            "-S",
            &source_arg,
            "-B",
            &build_arg,
            "-G",
            "Ninja",
            &build_type,
        ],
        root,
        verbose,
    )?;
    stage("Build Photon shell");
    invoke("cmake", &["--build", &build_arg], root, verbose)?;
    println!("Build succeeded in {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}

fn engine_build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let dir = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let mode = if release { "release" } else { "debug" };
    let vcpkg_root = root.join("build").join(format!("vcpkg-{mode}"));
    stage("Prepare pinned engine dependencies");
    let engine_source = path(&root.join("Engine"));
    let vcpkg_arg = path(&vcpkg_root);
    invoke(
        "python3",
        &[
            &path(&root.join("scripts/bootstrap_vcpkg.py")),
            &engine_source,
            &vcpkg_arg,
        ],
        root,
        verbose,
    )?;
    stage("Configure Photon Engine (the first run installs pinned dependencies)");
    let mode = if release { "Release" } else { "Debug" };
    let source_arg = path(&root.join("Engine"));
    let build_arg = path(&dir);
    let helper_path = if cfg!(any(target_os = "macos", target_os = "windows")) {
        root.join("build")
            .join(if release { "app-release" } else { "app-debug" })
    } else {
        root.join("build/bin")
    };
    let helper_dir = path(&helper_path);
    let build_type = format!("-DCMAKE_BUILD_TYPE={mode}");
    let vcpkg_type = format!("-DLADYBIRD_VCPKG_TYPE={}", mode.to_ascii_lowercase());
    invoke_engine_configure(
        &[
            "-S",
            &source_arg,
            "-B",
            &build_arg,
            "-G",
            "Ninja",
            &build_type,
            &vcpkg_type,
            &format!("-DPHOTON_HELPER_PROCESS_DIR={helper_dir}"),
            "-DENABLE_PHOTON_EMBEDDER=ON",
            "-DENABLE_LADYBIRD_UI=OFF",
            "-DENABLE_GUI_TARGETS=OFF",
            "-DENABLE_CRANELIFT_JIT=OFF",
        ],
        root,
        &vcpkg_root,
        mode,
        verbose,
    )?;
    stage("Build Photon Engine libraries and services");
    invoke(
        "cmake",
        &[
            "--build",
            &build_arg,
            "--target",
            "WebContent",
            "RequestServer",
            "ImageDecoder",
            "Compositor",
            "MediaServer",
            "WebWorker",
        ],
        root,
        verbose,
    )
}

fn invoke_engine_configure(
    args: &[&str],
    root: &Path,
    vcpkg_root: &Path,
    mode: &str,
    verbose: bool,
) -> Result<(), String> {
    let mut command = Command::new("cmake");
    command
        .args(args)
        .current_dir(root)
        .env("LADYBIRD_VCPKG_TYPE", mode)
        .env("VCPKG_ROOT", vcpkg_root)
        .env("LADYBIRD_MAIN_SOURCE_DIR", root.join("build"));
    if verbose {
        return command
            .status()
            .map_err(|e| e.to_string())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("cmake exited with {status}"))
                }
            });
    }
    let result = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if result.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    Err(format!(
        "CMake configure failed: {}",
        stderr
            .lines()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn run_app(root: &Path, release: bool) -> Result<(), String> {
    let exe = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" })
        .join("photon");
    command(&path(&exe), &[], root, true)
}

fn clean(root: &Path, scope: Option<&str>) -> Result<(), String> {
    let dirs: &[&str] = if scope == Some("engine") {
        &["engine-debug", "engine-release"]
    } else {
        &[
            "debug",
            "release",
            "app-debug",
            "app-release",
            "engine-debug",
            "engine-release",
        ]
    };
    for dir in dirs {
        let path = root.join("build").join(dir);
        if path.exists() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
        }
    }
    let helper_dir = root.join("build/bin");
    if helper_dir.exists() {
        std::fs::remove_dir_all(helper_dir).map_err(|e| e.to_string())?;
    }
    println!("Generated build trees removed.");
    Ok(())
}

fn check(root: &Path, verbose: bool) -> Result<(), String> {
    let mut diagnostics = String::new();
    match check_inner(root, verbose, &mut diagnostics) {
        Ok(()) => Ok(()),
        Err(error) => {
            let detail = if diagnostics.is_empty() {
                error.clone()
            } else {
                format!("{error}\n{diagnostics}")
            };
            print_fix_prompt(root, &detail);
            Err(error)
        }
    }
}

fn check_inner(root: &Path, verbose: bool, diagnostics: &mut String) -> Result<(), String> {
    invoke("cargo", &["fmt", "--all", "--", "--check"], root, verbose)?;
    invoke("cargo", &["check", "--workspace"], root, verbose)?;
    let lint_output = lint_qml(root, verbose)?;
    diagnostics.push_str(&lint_output);
    if lint_output
        .lines()
        .any(|line| line.starts_with("Warning:") || line.starts_with("Error:"))
    {
        return Err("QML lint reported warnings or errors".into());
    }
    let forbidden = output(
        "rg",
        &["-n", "QWidget|QMainWindow|Qt::Widgets", "native/qt", "ui"],
        root,
    )
    .unwrap_or_default();
    if !forbidden.trim().is_empty() {
        return Err(format!("Qt Widgets dependency found:\n{forbidden}"));
    }
    let submodule = output("git", &["submodule", "status", "Engine"], root)?;
    if submodule.starts_with('-') || submodule.starts_with('+') {
        return Err(format!("Engine submodule mismatch: {}", submodule.trim()));
    }
    println!("Architecture checks passed.");
    Ok(())
}

fn print_fix_prompt(root: &Path, diagnostics: &str) {
    let instructions = std::fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default();
    println!("\n--- Copy the prompt below into your AI agent ---\n");
    println!(
        "Fix the failing Photon check in this repository. Inspect the relevant files, make the smallest correct change, and rerun `./photon check`. Explain what you changed and report the check result. Preserve the project architecture and avoid unrelated edits.\n"
    );
    println!("Project instructions:\n{instructions}");
    println!("Check failure / diagnostics:\n{diagnostics}");
    println!("--- End prompt ---");
}

fn format(root: &Path, verbose: bool) -> Result<(), String> {
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
    let qmlformat = resolve_tool("qmlformat", qt_tool_directory(root).ok().as_deref())
        .ok_or_else(|| "qmlformat is missing; install the Qt 6 QML development tools".to_owned())?;
    invoke(
        &qmlformat,
        &["-i", "ui/Main.qml", "ui/theme/Theme.qml"],
        root,
        verbose,
    )?;
    Ok(())
}

fn lint_qml(root: &Path, _verbose: bool) -> Result<String, String> {
    let qt_bin = qt_tool_directory(root)?;
    let qmllint = resolve_tool("qmllint", Some(&qt_bin))
        .ok_or_else(|| "qmllint is missing; install the Qt 6 QML development tools".to_owned())?;
    let qtpaths = resolve_tool("qtpaths6", Some(&qt_bin)).unwrap_or_else(|| "qtpaths6".into());
    let qt_version = output(&qtpaths, &["--qt-version"], root)
        .map_err(|_| "Qt 6 development tools are missing; install Qt Quick, QML, and Quick Controls development packages".to_owned())?;
    if !version_at_least(&qt_version, (6, 5)) {
        return Err(format!(
            "Qt 6.5 or newer is required; found {}",
            qt_version.trim()
        ));
    }
    println!("==> Lint QML");
    let mut args = vec!["--ignore-settings".to_owned()];
    let build_dir = root.join("build/app-debug");
    let generated_qmldir = build_dir.join("Photon/qmldir");
    let generated_resource = build_dir.join(".qt/rcc/photon_raw_qml_0.qrc");
    if generated_qmldir.is_file() {
        args.push("-I".into());
        args.push(path(&build_dir));
        args.push("-I".into());
        args.push(path(&build_dir.join("Photon")));
        args.push("-i".into());
        args.push(path(&generated_qmldir));
    }
    if generated_resource.is_file() {
        args.push("--resource".into());
        args.push(path(&generated_resource));
    }
    args.extend(["ui/Main.qml".into(), "ui/theme/Theme.qml".into()]);
    let string_args: Vec<_> = args.iter().map(String::as_str).collect();
    capture_and_print(&qmllint, &string_args, root)
}

fn test(root: &Path, verbose: bool) -> Result<(), String> {
    invoke("cargo", &["test", "--workspace"], root, verbose)
}

fn engine(root: &Path, sub: Option<&str>, release: bool, verbose: bool) -> Result<(), String> {
    let engine = root.join("Engine");
    match sub.unwrap_or("status") {
        "status" => {
            for args in [
                &["status", "--short", "--branch"][..],
                &["remote", "-v"],
                &["rev-parse", "--short", "HEAD"],
            ] {
                print!("{}", output("git", args, &engine)?);
            }
            if let (Ok(a), Ok(b)) = (
                output(
                    "git",
                    &["rev-list", "--count", "HEAD..upstream/master"],
                    &engine,
                ),
                output(
                    "git",
                    &["rev-list", "--count", "upstream/master..HEAD"],
                    &engine,
                ),
            ) {
                println!("upstream behind {}, ahead {}", a.trim(), b.trim());
            } else {
                println!("upstream counts unavailable; fetch upstream to refresh tracking refs");
            }
            Ok(())
        }
        "build" => engine_build(root, release, verbose),
        "sync" => {
            if !output("git", &["status", "--porcelain"], &engine)?
                .trim()
                .is_empty()
            {
                return Err("Engine working tree must be clean before sync".into());
            }
            command("git", &["fetch", "upstream"], &engine, true)?;
            let current = output("git", &["rev-parse", "--short", "HEAD"], &engine)?;
            let target = output("git", &["rev-parse", "--short", "upstream/master"], &engine)?;
            println!("Merging Engine {current:.7} with upstream/master {target:.7}");
            command("git", &["merge", "upstream/master"], &engine, true)?;
            println!("Sync complete. Update and commit the Engine submodule pointer in photon.");
            Ok(())
        }
        _ => Err("usage: ./photon engine [status|build|sync]".into()),
    }
}

fn path(value: &Path) -> String {
    value.to_string_lossy().into_owned()
}
fn stage(text: &str) {
    println!("==> {text}");
}
fn require_tool(tool: &str) -> Result<(), String> {
    output(tool, &["--version"], Path::new("."))
        .map(|_| ())
        .map_err(|_| format!("{tool} is missing; install it using your OS package manager"))
}
fn qt_tool_directory(root: &Path) -> Result<PathBuf, String> {
    let qtpaths = resolve_tool("qtpaths6", None).unwrap_or_else(|| "qtpaths6".into());
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
fn resolve_tool(tool: &str, qt_bin: Option<&Path>) -> Option<String> {
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
    let candidate = qt_bin?.join(executable);
    candidate
        .is_file()
        .then(|| candidate.to_string_lossy().into_owned())
}
fn run_tool(tool: &str, args: &[&str], root: &Path) -> Result<String, String> {
    let qt_bin = qt_tool_directory(root).ok();
    let resolved = resolve_tool(tool, qt_bin.as_deref()).unwrap_or_else(|| tool.to_owned());
    output(&resolved, args, root)
}
fn compiler_version(root: &Path) -> Option<String> {
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
fn output(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
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
fn command(program: &str, args: &[&str], cwd: &Path, verbose: bool) -> Result<(), String> {
    invoke(program, args, cwd, verbose)
}
fn capture_and_print(program: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let result = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    use std::io::Write;
    std::io::stdout()
        .write_all(&result.stdout)
        .map_err(|e| e.to_string())?;
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    if result.status.success() {
        Ok(diagnostics)
    } else {
        Err(format!("{program} failed:\n{diagnostics}"))
    }
}
fn invoke(program: &str, args: &[&str], cwd: &Path, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
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
