use owo_colors::{OwoColorize, Stream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Read,
};

use crate::support::{
    hash_output, invoke, invoke_with_environment, output, path, require_tool_version, stage,
    success, with_progress,
};

const HELPER_PROCESS_TARGETS: &[&str] = &[
    "WebContent",
    "WebWorker",
    "Compositor",
    "RequestServer",
    "ImageDecoder",
    "MediaServer",
    "VideoDecoderServer",
    "AudioServer",
];

#[cfg(unix)]
static TERMINATION_SIGNAL: AtomicI32 = AtomicI32::new(0);

#[cfg(unix)]
extern "C" fn record_termination_signal(signal: libc::c_int) {
    TERMINATION_SIGNAL.store(signal, Ordering::Relaxed);
}

fn wait_for_application(mut command: Command, launch_error: &str) -> Result<ExitStatus, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        TERMINATION_SIGNAL.store(0, Ordering::Relaxed);
        // Keep the app and its helpers in a group the launcher can stop as a unit.
        command.process_group(0);
        // SAFETY: the handler only writes to a lock-free atomic, which is safe
        // to access from a signal handler.
        unsafe {
            libc::signal(
                libc::SIGINT,
                record_termination_signal as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGTERM,
                record_termination_signal as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGHUP,
                record_termination_signal as *const () as libc::sighandler_t,
            );
        }
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("{launch_error}: {error}"))?;
    wait_for_child(&mut child)
}

fn wait_for_child(child: &mut Child) -> Result<ExitStatus, String> {
    loop {
        #[cfg(unix)]
        let signal = TERMINATION_SIGNAL.load(Ordering::Relaxed);
        #[cfg(not(unix))]
        let signal = 0;

        if signal != 0 {
            #[cfg(unix)]
            {
                // Let the app run its normal quit hooks before stopping its
                // engine helper processes. Killing the whole group here can
                // make WebContent report a crash while the browser is still
                // handling the shutdown.
                // SAFETY: signals only the child this function owns.
                unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
                loop {
                    if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                        // Stop any helper the app left behind.
                        // SAFETY: the child was started in a fresh process group.
                        unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                        return Ok(status);
                    }
                    if std::time::Instant::now() >= deadline {
                        // SAFETY: the child was started in a fresh process group.
                        unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
                        return child.wait().map_err(|error| error.to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        }

        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub(crate) fn warn_if_dependencies_are_behind(root: &Path) {
    let dependencies = [
        ("Engine", "Engine", "upstream/master", 10),
        ("GPUI-CE", "vendor/gpui-ce", "upstream/main", 1),
    ];

    let mut behind = Vec::new();
    for (name, repository, upstream, threshold) in dependencies {
        let repository = root.join(repository);
        let Some(count) = upstream_ahead_count(&repository, upstream) else {
            continue;
        };
        if is_dependency_behind(count, threshold) {
            behind.push(format!("{name}: {count}"));
        }
    }

    if !behind.is_empty() {
        let warning = format!(
            "⚠ Dependency updates available (commits behind upstream): {}. Run `photon sync --check` to review.",
            behind.join(", ")
        );
        eprintln!(
            "{}",
            warning.if_supports_color(Stream::Stderr, |text| format!("{}", text.yellow()))
        );
    }
}

fn is_dependency_behind(count: usize, threshold: usize) -> bool {
    count >= threshold
}

fn upstream_ahead_count(repository: &Path, upstream: &str) -> Option<usize> {
    let result = Command::new("git")
        .args(["rev-list", "--count", &format!("HEAD..{upstream}")])
        .current_dir(repository)
        .output()
        .ok()?;
    if !result.status.success() {
        return None;
    }
    String::from_utf8_lossy(&result.stdout)
        .trim()
        .parse::<usize>()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        process::Command,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{is_dependency_behind, warn_if_dependencies_are_behind};

    static NEXT_TEMP_REPO: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn dependency_warning_thresholds_are_inclusive() {
        assert!(!is_dependency_behind(9, 10));
        assert!(is_dependency_behind(10, 10));
        assert!(!is_dependency_behind(0, 1));
        assert!(is_dependency_behind(1, 1));
    }

    #[test]
    fn warning_displays_for_disposable_dependencies_at_thresholds() {
        let root = temporary_repository();
        for (path, branch, count) in [("Engine", "master", 10), ("vendor/gpui-ce", "main", 1)] {
            let repository = root.join(path);
            std::fs::create_dir_all(&repository).unwrap();
            git(&repository, &["init", "-q"]);
            git(&repository, &["config", "user.name", "Photon Test"]);
            git(
                &repository,
                &["config", "user.email", "photon-test@example.invalid"],
            );
            std::fs::write(repository.join("file"), "0").unwrap();
            git(&repository, &["add", "file"]);
            git(&repository, &["commit", "-qm", "base"]);
            let base = git_output(&repository, &["rev-parse", "HEAD"]);
            for revision in 1..=count {
                std::fs::write(repository.join("file"), revision.to_string()).unwrap();
                git(
                    &repository,
                    &["commit", "-qam", &format!("revision {revision}")],
                );
            }
            git(
                &repository,
                &[
                    "update-ref",
                    &format!("refs/remotes/upstream/{branch}"),
                    "HEAD",
                ],
            );
            git(&repository, &["reset", "-q", "--hard", &base]);
        }

        warn_if_dependencies_are_behind(&root);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn temporary_repository() -> std::path::PathBuf {
        let sequence = NEXT_TEMP_REPO.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("photon-cli-warning-test-{sequence}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn git(repository: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repository)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn git_output(repository: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repository)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

pub(crate) fn build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let started = Instant::now();
    engine_build(root, release, verbose)?;
    build_app(root, release, verbose)?;
    success(&format!(
        "Build completed in {:.1}s",
        started.elapsed().as_secs_f64()
    ));
    Ok(())
}

fn build_app(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    stage("Build Photon application");
    let engine = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let mut args = vec!["build", "-p", "photon-app", "--bin", "Photon"];
    if release {
        args.push("--release");
    }
    invoke_cargo(&args, root, Some(&engine), verbose)
}

fn invoke_cargo(
    args: &[&str],
    root: &Path,
    engine_dir: Option<&Path>,
    verbose: bool,
) -> Result<(), String> {
    let environment = engine_dir
        .map(|directory| vec![("PHOTON_ENGINE_BUILD_DIR", directory.display().to_string())])
        .unwrap_or_default();
    invoke_with_environment("cargo", args, root, verbose, &environment)
}

pub(crate) fn engine_build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    for (tool, minimum) in [("cmake", (3, 30)), ("ninja", (1, 10))] {
        require_tool_version(tool, minimum, root)?;
    }

    let dir = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let fingerprint = engine_fingerprint(root, release)?;
    let fingerprint_path = dir.join(".photon-build-fingerprint");
    let mut required_outputs = vec![embedder_library_output(&dir)];
    required_outputs.extend(
        HELPER_PROCESS_TARGETS
            .iter()
            .map(|name| helper_process_executable(root, &dir, release, name)),
    );
    if cfg!(target_os = "macos") {
        required_outputs.push(helper_process_executable(
            root,
            &dir,
            release,
            "ProcessReaper",
        ));
        required_outputs.extend([
            root.join("build/Resources/fonts/NotoEmoji.ttf"),
            root.join("build/Resources/fonts/SerenitySans-Regular.ttf"),
            root.join("build/Resources/ladybird/site-compatibility/cnn.com.json"),
        ]);
    }
    if std::fs::read_to_string(&fingerprint_path).ok().as_deref() == Some(fingerprint.as_str())
        && required_outputs.iter().all(|output| output.exists())
    {
        success("Photon Engine is up to date");
        return Ok(());
    }
    if !fingerprint_path.exists()
        && required_outputs.iter().all(|output| output.exists())
        && engine_targets_are_current(&dir, root)?
    {
        std::fs::write(&fingerprint_path, &fingerprint).map_err(|error| {
            format!("could not record Photon Engine build fingerprint: {error}")
        })?;
        success("Photon Engine outputs are current");
        return Ok(());
    }

    stage("Build Photon Engine");
    let mode = if release { "release" } else { "debug" };
    let vcpkg_root = root.join("build").join(format!("vcpkg-{mode}"));
    let engine_source = path(&root.join("Engine"));
    let vcpkg_arg = path(&vcpkg_root);
    with_progress("Prepare pinned Engine dependencies", !verbose, || {
        invoke(
            "python3",
            &[
                &path(&root.join("scripts/bootstrap_vcpkg.py")),
                &engine_source,
                &vcpkg_arg,
            ],
            root,
            verbose,
        )
    })?;
    let mode = if release { "Release" } else { "Debug" };
    let source_arg = path(&root.join("Engine"));
    let build_arg = path(&dir);
    let helper_dir = path(&helper_process_dir(root, &dir, release));
    let build_type = format!("-DCMAKE_BUILD_TYPE={mode}");
    let vcpkg_type = format!("-DLADYBIRD_VCPKG_TYPE={}", mode.to_ascii_lowercase());
    with_progress("Configure Photon Engine", !verbose, || {
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
        )
    })?;
    let targets = engine_build_targets();
    let mut build_args = vec!["--build", build_arg.as_str(), "--target"];
    build_args.extend(targets);
    with_progress(
        "Compile Photon Engine libraries and services",
        !verbose,
        || invoke("cmake", &build_args, root, verbose),
    )?;
    std::fs::write(&fingerprint_path, fingerprint).map_err(|error| {
        format!(
            "could not record Photon Engine build fingerprint {}: {error}",
            fingerprint_path.display()
        )
    })
}

fn helper_process_dir(root: &Path, engine_dir: &Path, release: bool) -> PathBuf {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        root.join("build")
            .join(if release { "app-release" } else { "app-debug" })
    } else {
        engine_dir.join("bin")
    }
}

fn helper_process_executable(root: &Path, engine_dir: &Path, release: bool, name: &str) -> PathBuf {
    let filename = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    helper_process_dir(root, engine_dir, release).join(filename)
}

fn embedder_library_output(engine_dir: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        engine_dir.join("lib/liblagom-photonembedder.dylib")
    } else if cfg!(target_os = "windows") {
        engine_dir.join("bin/lagom-photonembedder.dll")
    } else {
        engine_dir.join("lib64/liblagom-photonembedder.so")
    }
}

fn engine_build_targets() -> Vec<&'static str> {
    let mut targets = HELPER_PROCESS_TARGETS.to_vec();
    targets.push("LibPhotonEmbedder");
    if cfg!(target_os = "macos") {
        targets.push("ProcessReaper");
    }
    targets
}

fn engine_targets_are_current(engine_dir: &Path, root: &Path) -> Result<bool, String> {
    let engine_dir = path(engine_dir);
    let mut args = vec!["-C", &engine_dir, "-n"];
    args.extend(engine_build_targets());
    let result = Command::new("ninja")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot inspect Photon Engine build state: {error}"))?;
    if !result.status.success() {
        return Ok(false);
    }
    let output = String::from_utf8_lossy(&result.stdout);
    Ok(output.contains("ninja: no work to do."))
}

fn engine_fingerprint(root: &Path, release: bool) -> Result<String, String> {
    let engine_source = path(&root.join("Engine"));
    let revision = output("git", &["-C", &engine_source, "rev-parse", "HEAD"], root)?;
    let diff_hash = hash_output(
        "git",
        &["-C", &engine_source, "diff", "--binary", "HEAD"],
        root,
    )?;
    let status = output(
        "git",
        &[
            "-C",
            &engine_source,
            "status",
            "--porcelain",
            "--untracked-files=all",
        ],
        root,
    )?;
    let untracked = output(
        "git",
        &[
            "-C",
            &engine_source,
            "ls-files",
            "--others",
            "--exclude-standard",
        ],
        root,
    )?;

    let mut hasher = DefaultHasher::new();
    revision.hash(&mut hasher);
    diff_hash.hash(&mut hasher);
    status.hash(&mut hasher);
    release.hash(&mut hasher);
    "ENABLE_PHOTON_EMBEDDER=ON;ENABLE_LADYBIRD_UI=OFF;ENABLE_GUI_TARGETS=OFF;ENABLE_CRANELIFT_JIT=OFF"
        .hash(&mut hasher);
    for relative_path in untracked.lines() {
        let file_path = root.join("Engine").join(relative_path);
        relative_path.hash(&mut hasher);
        if file_path.is_file() {
            hash_file(&file_path, &mut hasher)?;
        }
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn hash_file(path: &Path, hasher: &mut impl Hasher) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "cannot read untracked Engine file {}: {error}",
            path.display()
        )
    })?;
    file.metadata()
        .map_err(|error| format!("cannot inspect Engine file {}: {error}", path.display()))?
        .len()
        .hash(hasher);

    let mut buffer = [0; 8 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read Engine file {}: {error}", path.display()))?;
        if read == 0 {
            return Ok(());
        }
        hasher.write(&buffer[..read]);
    }
}

fn invoke_engine_configure(
    args: &[&str],
    root: &Path,
    vcpkg_root: &Path,
    mode: &str,
    verbose: bool,
) -> Result<(), String> {
    let environment = [
        ("LADYBIRD_VCPKG_TYPE", mode.to_owned()),
        ("VCPKG_ROOT", path(vcpkg_root)),
        ("LADYBIRD_MAIN_SOURCE_DIR", path(&root.join("build"))),
    ];
    invoke_with_environment("cmake", args, root, verbose, &environment)
}

pub(crate) fn run_direct(
    root: &Path,
    release: bool,
    verbose: bool,
    url: Option<&str>,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return run_direct_macos(root, release, verbose, url, shutdown_after_seconds);

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    return run_direct_portable(root, release, verbose, url, shutdown_after_seconds);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (root, release, verbose, url, shutdown_after_seconds);
        Err("the direct Photon shell supports macOS, Linux, and Windows".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn run_direct_portable(
    root: &Path,
    release: bool,
    verbose: bool,
    url: Option<&str>,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    build(root, release, verbose)?;
    let engine = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let helper_directory = helper_process_dir(root, &engine, release);
    let executable = photon_executable(root, release);
    let mut process = Command::new(executable);
    process
        .current_dir(root)
        .env("PHOTON_HELPER_DIRECTORY", helper_directory)
        .env(runtime_library_variable(), runtime_library_path(&engine));
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    if let Some(url) = url {
        process.env("PHOTON_URL", url);
    }
    if let Some(seconds) = shutdown_after_seconds {
        process.env("PHOTON_SHUTDOWN_AFTER_SECONDS", seconds.to_string());
    }
    let status = wait_for_application(process, "cannot start direct Photon application")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Photon shell exited with {status}"))
    }
}

#[cfg(target_os = "macos")]
fn run_direct_macos(
    root: &Path,
    release: bool,
    verbose: bool,
    url: Option<&str>,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    build(root, release, verbose)?;
    let engine = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let service = super::presentation_broker::ensure(root, verbose)?;
    let helper_dir = helper_process_dir(root, &engine, release);
    let built = root
        .join("target")
        .join(if release { "release" } else { "debug" })
        .join("Photon");
    let helper_dir = helper_dir.to_string_lossy().into_owned();
    // The same environment lets macOS launch the bundle for a link; the
    // browser makes its own presentation channel then.
    let executable = super::app_bundle::ensure(
        root,
        &built,
        &[
            ("PHOTON_HELPER_DIRECTORY", helper_dir.clone()),
            ("PHOTON_PRESENTATION_XPC_SERVICE", service.clone()),
        ],
    )?;
    let mut process = Command::new(executable);
    process
        // Photon restores browser state itself. Don't let AppKit block a
        // launch with its crash-recovery window-restoration alert.
        .args(["-ApplePersistenceIgnoreState", "YES"])
        .current_dir(root)
        .env("PHOTON_HELPER_DIRECTORY", helper_dir)
        .env("PHOTON_PRESENTATION_XPC_SERVICE", service)
        .env(
            "PHOTON_PRESENTATION_CHANNEL_ID",
            format!(
                "photon-gpui-ce-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
        )
        .env("DYLD_LIBRARY_PATH", runtime_library_path(&engine));
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    if let Some(url) = url {
        process.env("PHOTON_URL", url);
    }
    if let Some(seconds) = shutdown_after_seconds {
        process.env("PHOTON_SHUTDOWN_AFTER_SECONDS", seconds.to_string());
    }
    let status = wait_for_application(process, "cannot start direct GPUI-CE application")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Photon GPUI-CE exited with {status}"))
    }
}

/// Builds and launches the existing browser shell without building or linking Photon Engine.
pub(crate) fn run_ui(
    root: &Path,
    release: bool,
    verbose: bool,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return run_ui_macos(root, release, verbose, shutdown_after_seconds);

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    return run_ui_portable(root, release, verbose, shutdown_after_seconds);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (root, release, verbose, shutdown_after_seconds);
        Err("the Photon UI shell supports macOS, Linux, and Windows".into())
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn run_ui_portable(
    root: &Path,
    release: bool,
    verbose: bool,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    stage("Build Photon UI-only shell");
    let mut args = vec![
        "build",
        "-p",
        "photon-app",
        "--bin",
        "Photon",
        "--features",
        "ui-only",
    ];
    if release {
        args.push("--release");
    }
    invoke_cargo(&args, root, None, verbose)?;

    let mut process = Command::new(photon_executable(root, release));
    process.current_dir(root);
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    if let Some(seconds) = shutdown_after_seconds {
        process.env("PHOTON_SHUTDOWN_AFTER_SECONDS", seconds.to_string());
    }
    let status = wait_for_application(process, "cannot start UI-only Photon shell")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("UI-only Photon shell exited with {status}"))
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn photon_executable(root: &Path, release: bool) -> PathBuf {
    let filename = if cfg!(target_os = "windows") {
        "Photon.exe"
    } else {
        "Photon"
    };
    root.join("target")
        .join(if release { "release" } else { "debug" })
        .join(filename)
}

#[cfg(target_os = "macos")]
fn run_ui_macos(
    root: &Path,
    release: bool,
    verbose: bool,
    shutdown_after_seconds: Option<u64>,
) -> Result<(), String> {
    stage("Build Photon UI-only shell");
    let mut args = vec![
        "build",
        "-p",
        "photon-app",
        "--bin",
        "Photon",
        "--features",
        "ui-only",
    ];
    if release {
        args.push("--release");
    }
    invoke_cargo(&args, root, None, verbose)?;

    let built = root
        .join("target")
        .join(if release { "release" } else { "debug" })
        .join("Photon");
    let executable = super::app_bundle::ensure_ui(root, &built)?;
    let mut process = Command::new(executable);
    process.current_dir(root);
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    if let Some(seconds) = shutdown_after_seconds {
        process.env("PHOTON_SHUTDOWN_AFTER_SECONDS", seconds.to_string());
    }
    let status = wait_for_application(process, "cannot start UI-only Photon shell")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("UI-only Photon shell exited with {status}"))
    }
}

pub(crate) fn runtime_library_path(engine: &Path) -> String {
    let mut paths = vec![engine.join("lib64"), engine.join("lib")];
    if cfg!(target_os = "linux") {
        for triplet in ["x64-linux-dynamic", "x64-linux-dynamic/debug"] {
            paths.push(engine.join("vcpkg_installed").join(triplet).join("lib"));
        }
    }
    if cfg!(target_os = "windows") {
        paths.push(engine.join("bin"));
        paths.push(engine.join("vcpkg_installed/x64-windows/bin"));
    }
    let variable = runtime_library_variable();
    if let Some(existing) = std::env::var_os(variable) {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn runtime_library_variable() -> &'static str {
    if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else if cfg!(target_os = "windows") {
        "PATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

pub(crate) fn clean(root: &Path, scope: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if scope != Some("engine") {
        super::presentation_broker::clean(root)?;
    }
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
    if scope != Some("engine") {
        let helper_dir = root.join("build/bin");
        if helper_dir.exists() {
            std::fs::remove_dir_all(helper_dir).map_err(|e| e.to_string())?;
        }
    }
    println!("Generated build trees removed.");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::engine_build_targets;

    #[test]
    fn audio_server_is_built_as_a_helper_process() {
        assert!(engine_build_targets().contains(&"AudioServer"));
    }
}
