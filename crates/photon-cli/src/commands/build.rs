use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};

use crate::support::{invoke, path, stage, with_progress};

pub(crate) fn build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    stage("Run checks before building");
    crate::commands::check::check(root, verbose)?;

    let started = Instant::now();
    engine_build(root, release, verbose)?;
    stage("Build Rust browser state");
    let mut rust_args = vec![
        "build".to_owned(),
        "--manifest-path".to_owned(),
        path(&root.join("Cargo.toml")),
        "-p".to_owned(),
        "photon-core".to_owned(),
    ];
    if release {
        rust_args.push("--release".to_owned());
    }
    let rust_refs: Vec<&str> = rust_args.iter().map(String::as_str).collect();
    invoke("cargo", &rust_refs, root, verbose)?;
    let app_build = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" });
    stage("Build Photon shell");
    let build_arg = path(&app_build);
    let build_type = format!(
        "-DCMAKE_BUILD_TYPE={}",
        if release { "Release" } else { "Debug" }
    );
    let engine_build = path(&root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    }));
    let helper_directory = if cfg!(any(target_os = "macos", target_os = "windows")) {
        path(&app_build)
    } else {
        path(
            &root
                .join("build")
                .join(if release {
                    "engine-release"
                } else {
                    "engine-debug"
                })
                .join("bin"),
        )
    };
    with_progress("Configure Qt Quick shell", !verbose, || {
        invoke(
            "cmake",
            &[
                "-S",
                &path(&root.join("native/qt")),
                "-B",
                &build_arg,
                "-G",
                "Ninja",
                &build_type,
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
                &format!("-DPHOTON_ENGINE_BUILD_DIR={engine_build}"),
                &format!("-DPHOTON_HELPER_DIRECTORY={helper_directory}"),
            ],
            root,
            verbose,
        )
    })?;
    with_progress("Compile Qt Quick shell", !verbose, || {
        invoke("cmake", &["--build", &build_arg], root, verbose)
    })?;
    println!("Build succeeded in {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}

pub(crate) fn engine_build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    stage("Build Photon Engine");
    let dir = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
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
    let helper_path = if cfg!(any(target_os = "macos", target_os = "windows")) {
        root.join("build")
            .join(if release { "app-release" } else { "app-debug" })
    } else {
        dir.join("bin")
    };
    let helper_dir = path(&helper_path);
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
    with_progress(
        "Compile Photon Engine libraries and services",
        !verbose,
        || {
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
                    "LibPhotonEmbedder",
                ],
                root,
                verbose,
            )
        },
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

fn start_app(root: &Path, release: bool, verbose: bool) -> Result<Child, String> {
    let exe = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" })
        .join("photon");
    let mut process = Command::new(&exe);
    process.current_dir(root);
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    process
        .spawn()
        .map_err(|error| format!("cannot launch Photon: {error}"))
}

pub(crate) fn watch_run(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let (events_tx, events_rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(events_tx)
        .map_err(|error| format!("cannot start file watcher: {error}"))?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| format!("cannot watch {}: {error}", root.display()))?;

    println!("Watching {} for changes (Ctrl+C to stop).", root.display());
    let mut app = build_and_start(root, release, verbose);

    loop {
        if let Some(status) = take_app_exit_status(&mut app)? {
            return report_app_exit(status);
        }

        let event = match events_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                eprintln!("File watcher error: {error}");
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("file watcher stopped".into());
            }
        };
        if !event_should_rebuild(root, &event) {
            continue;
        }

        // Coalesce the burst of events produced by a save or generated files.
        loop {
            match events_rx.recv_timeout(Duration::from_millis(300)) {
                Ok(Ok(event)) if event_should_rebuild(root, &event) => continue,
                Ok(Err(error)) => eprintln!("File watcher error: {error}"),
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("file watcher stopped".into());
                }
            }
        }
        println!("\nChange detected; restarting Photon...");
        if let Some(status) = take_app_exit_status(&mut app)? {
            return report_app_exit(status);
        }
        stop_app(&mut app);
        app = build_and_start(root, release, verbose);
    }
}

fn take_app_exit_status(app: &mut Option<Child>) -> Result<Option<ExitStatus>, String> {
    let Some(child) = app.as_mut() else {
        return Ok(None);
    };
    let status = child
        .try_wait()
        .map_err(|error| format!("cannot check Photon process: {error}"))?;
    if status.is_some() {
        app.take();
    }
    Ok(status)
}

fn report_app_exit(status: ExitStatus) -> Result<(), String> {
    println!("Photon exited with {status}.");
    if status.success() {
        Ok(())
    } else {
        Err(format!("Photon exited with {status}"))
    }
}

fn build_and_start(root: &Path, release: bool, verbose: bool) -> Option<Child> {
    match build(root, release, verbose) {
        Ok(()) => match start_app(root, release, verbose) {
            Ok(child) => {
                println!("Photon started (pid {}).", child.id());
                Some(child)
            }
            Err(error) => {
                eprintln!("{error}");
                None
            }
        },
        Err(error) => {
            eprintln!("Build failed: {error}");
            None
        }
    }
}

fn stop_app(app: &mut Option<Child>) {
    let Some(mut child) = app.take() else {
        return;
    };
    match child.try_wait() {
        Ok(Some(status)) => eprintln!("Photon exited with {status}"),
        Ok(None) => {
            if let Err(error) = child.kill() {
                eprintln!("Could not stop Photon before rebuilding: {error}");
            }
            if let Err(error) = child.wait() {
                eprintln!("Could not wait for Photon to stop: {error}");
            }
        }
        Err(error) => eprintln!("Could not check Photon process state: {error}"),
    }
}

fn event_should_rebuild(root: &Path, event: &Event) -> bool {
    let changes_files = matches!(
        event.kind,
        EventKind::Create(CreateKind::File | CreateKind::Any)
            | EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Remove(RemoveKind::File | RemoveKind::Any)
            | EventKind::Any
    );
    changes_files && event.paths.iter().any(|path| should_rebuild(root, path))
}

#[cfg(test)]
mod watcher_tests {
    use super::*;
    use notify::event::{AccessKind, DataChange};

    #[test]
    fn file_saves_trigger_restart_but_reads_do_not() {
        let root = Path::new("/workspace/photon");
        let source = root.join("ui/Main.qml");
        let save = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(source.clone());
        let read = Event::new(EventKind::Access(AccessKind::Read)).add_path(source);
        assert!(event_should_rebuild(root, &save));
        assert!(!event_should_rebuild(root, &read));
    }

    #[test]
    fn generated_files_and_repository_metadata_are_ignored() {
        let root = Path::new("/workspace/photon");
        for path in [
            root.join("build/app-debug/photon"),
            root.join("target/debug/photon"),
            root.join(".git/index"),
            root.join("ui/.qmlls.ini"),
            root.join("ui/.qmlls.ini.tmpfd242"),
        ] {
            let event = Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path);
            assert!(!event_should_rebuild(root, &event));
        }
    }
}

fn should_rebuild(root: &Path, changed: &Path) -> bool {
    let Ok(relative) = changed.strip_prefix(root) else {
        return false;
    };
    if relative.as_os_str().is_empty() {
        return false;
    }
    let generated_qmlls_config = relative == Path::new("ui/.qmlls.ini")
        || relative
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".qmlls.ini.tmp"));
    !generated_qmlls_config
        && !relative.components().any(|component| {
            let component = component.as_os_str().to_string_lossy();
            matches!(component.as_ref(), ".git" | "build" | "target")
        })
}

pub(crate) fn clean(root: &Path, scope: Option<&str>) -> Result<(), String> {
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
        crate::commands::ide::remove_generated_config(root)?;
    }
    let helper_dir = root.join("build/bin");
    if helper_dir.exists() {
        std::fs::remove_dir_all(helper_dir).map_err(|e| e.to_string())?;
    }
    println!("Generated build trees removed.");
    Ok(())
}
