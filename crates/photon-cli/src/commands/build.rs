use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use notify::{RecursiveMode, Watcher};

use crate::support::{invoke, path, stage, with_progress};

pub(crate) fn build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    stage("Run checks before building");
    crate::commands::check::check(root, verbose)?;

    let started = Instant::now();
    engine_build(root, release, verbose)?;
    let source = root.join("native/qt");
    let app_build = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" });
    stage("Build Photon shell");
    let source_arg = path(&source);
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
                &source_arg,
                "-B",
                &build_arg,
                "-G",
                "Ninja",
                &build_type,
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

pub(crate) fn run_app(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let exe = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" })
        .join("photon");
    let mut process = Command::new(&exe);
    process.current_dir(root);
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    let status = process
        .status()
        .map_err(|error| format!("cannot launch Photon: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Photon exited with {status}"))
    }
}

pub(crate) fn watch_run(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let (events_tx, events_rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(events_tx)
        .map_err(|error| format!("cannot start file watcher: {error}"))?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| format!("cannot watch {}: {error}", root.display()))?;

    println!("Watching {} for changes (Ctrl+C to stop).", root.display());
    match build(root, release, verbose) {
        Ok(()) => {
            if let Err(error) = run_app(root, release, verbose) {
                eprintln!("{error}");
            }
        }
        Err(error) => eprintln!("Build failed: {error}"),
    }

    loop {
        let event = match events_rx.recv() {
            Ok(event) => event,
            Err(error) => return Err(format!("file watcher stopped: {error}")),
        };
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                eprintln!("File watcher error: {error}");
                continue;
            }
        };
        if !event.paths.iter().any(|path| should_rebuild(root, path)) {
            continue;
        }

        // Coalesce the burst of events produced by a save or generated files.
        while events_rx
            .recv_timeout(std::time::Duration::from_millis(250))
            .is_ok()
        {}
        println!("\nChange detected; rebuilding Photon...");
        match build(root, release, verbose) {
            Ok(()) => {
                if let Err(error) = run_app(root, release, verbose) {
                    eprintln!("{error}");
                }
            }
            Err(error) => eprintln!("Build failed: {error}"),
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
    !relative.components().any(|component| {
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
    let helper_dir = root.join("build/bin");
    if helper_dir.exists() {
        std::fs::remove_dir_all(helper_dir).map_err(|e| e.to_string())?;
    }
    println!("Generated build trees removed.");
    Ok(())
}
