use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::support::{invoke, output, path, stage, with_progress};

pub(crate) fn build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    stage("Run checks before building");
    crate::commands::check::check(root, verbose)?;

    let started = Instant::now();
    engine_build(root, release, verbose)?;
    stage("Build source native addon");
    let mut rust_args = vec![
        "build".to_owned(),
        "-p".to_owned(),
        "photon-native-addon".to_owned(),
    ];
    if release {
        rust_args.push("--release".to_owned());
    }
    let rust_refs: Vec<&str> = rust_args.iter().map(String::as_str).collect();
    let engine_dir = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    invoke_cargo_with_engine(&rust_refs, root, &engine_dir, verbose)?;
    let app_build = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" });
    std::fs::create_dir_all(&app_build).map_err(|error| error.to_string())?;
    let profile = if release { "release" } else { "debug" };
    let addon = root.join("target").join(profile).join(format!(
        "{}photon_native_addon{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    let staged_addon = app_build.join("photon-native-addon.node");
    std::fs::copy(&addon, &staged_addon).map_err(|error| {
        format!(
            "could not stage source-built GPUIX addon {}: {error}",
            addon.display()
        )
    })?;
    println!("Build succeeded in {:.1}s", started.elapsed().as_secs_f64());
    Ok(())
}

fn invoke_cargo_with_engine(
    args: &[&str],
    root: &Path,
    engine_dir: &Path,
    verbose: bool,
) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command
        .args(args)
        .current_dir(root)
        .env("PHOTON_ENGINE_BUILD_DIR", engine_dir);
    if verbose {
        return command
            .status()
            .map_err(|error| error.to_string())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("cargo exited with {status}"))
                }
            });
    }
    let result = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("cannot run cargo: {error}"))?;
    if result.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    Err(format!(
        "cargo failed: {}",
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

pub(crate) fn engine_build(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    let dir = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let fingerprint = engine_fingerprint(root, release)?;
    let fingerprint_path = dir.join(".photon-build-fingerprint");
    let required_outputs = [
        dir.join("lib64/liblagom-photonembedder.so"),
        dir.join("bin/WebContent"),
        dir.join("bin/WebWorker"),
        dir.join("bin/Compositor"),
        dir.join("bin/RequestServer"),
        dir.join("bin/ImageDecoder"),
    ];
    if std::fs::read_to_string(&fingerprint_path).ok().as_deref() == Some(fingerprint.as_str())
        && required_outputs.iter().all(|output| output.exists())
    {
        if verbose {
            println!("Photon Engine is up to date.");
        }
        return Ok(());
    }
    if !fingerprint_path.exists()
        && required_outputs.iter().all(|output| output.exists())
        && engine_targets_are_current(&dir, root)?
    {
        std::fs::write(&fingerprint_path, &fingerprint).map_err(|error| {
            format!("could not record Photon Engine build fingerprint: {error}")
        })?;
        if verbose {
            println!("Photon Engine outputs are current; recorded build fingerprint.");
        }
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
    )?;
    std::fs::write(&fingerprint_path, fingerprint).map_err(|error| {
        format!(
            "could not record Photon Engine build fingerprint {}: {error}",
            fingerprint_path.display()
        )
    })
}

fn engine_targets_are_current(engine_dir: &Path, root: &Path) -> Result<bool, String> {
    let engine_dir = path(engine_dir);
    let result = Command::new("ninja")
        .args([
            "-C",
            &engine_dir,
            "-n",
            "WebContent",
            "RequestServer",
            "ImageDecoder",
            "Compositor",
            "MediaServer",
            "WebWorker",
            "LibPhotonEmbedder",
        ])
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
    let diff = Command::new("git")
        .args(["-C", &engine_source, "diff", "--binary", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot inspect Photon Engine changes: {error}"))?;
    if !diff.status.success() {
        return Err(format!(
            "cannot inspect Photon Engine changes: {}",
            String::from_utf8_lossy(&diff.stderr).trim()
        ));
    }
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
    diff.stdout.hash(&mut hasher);
    status.hash(&mut hasher);
    release.hash(&mut hasher);
    "ENABLE_PHOTON_EMBEDDER=ON;ENABLE_LADYBIRD_UI=OFF;ENABLE_GUI_TARGETS=OFF;ENABLE_CRANELIFT_JIT=OFF"
        .hash(&mut hasher);
    for relative_path in untracked.lines() {
        let file_path = root.join("Engine").join(relative_path);
        relative_path.hash(&mut hasher);
        if file_path.is_file() {
            let contents = std::fs::read(&file_path).map_err(|error| {
                format!(
                    "cannot read untracked Engine file {}: {error}",
                    file_path.display()
                )
            })?;
            contents.hash(&mut hasher);
        }
    }
    Ok(format!("{:016x}", hasher.finish()))
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

pub(crate) fn run(root: &Path, release: bool, verbose: bool) -> Result<(), String> {
    ensure_gpuix_js(root, verbose)?;
    ensure_ui_dependencies(root, verbose)?;
    build(root, release, verbose)?;

    let app_build = root
        .join("build")
        .join(if release { "app-release" } else { "app-debug" });
    let addon_path = app_build.join("photon-native-addon.node");
    let engine = root.join("build").join(if release {
        "engine-release"
    } else {
        "engine-debug"
    });
    let helper_dir = if cfg!(any(target_os = "macos", target_os = "windows")) {
        app_build.clone()
    } else {
        engine.join("bin")
    };
    let mut process = Command::new("bun");
    process
        .args(["--hot", "src/main.tsx"])
        .current_dir(root.join("ui"))
        .env("NAPI_RS_NATIVE_LIBRARY_PATH", addon_path)
        .env("PHOTON_HELPER_DIRECTORY", helper_dir)
        .env("LD_LIBRARY_PATH", runtime_library_path(&engine));
    if verbose {
        process.env("PHOTON_VERBOSE", "1");
    }
    let status = process
        .status()
        .map_err(|error| format!("cannot start GPUIX Bun runtime: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Photon exited with {status}"))
    }
}

pub(crate) fn ensure_ui_dependencies(root: &Path, verbose: bool) -> Result<(), String> {
    let ui = root.join("ui");
    if ui.join("node_modules/@gpuix/react/package.json").is_file()
        && ui.join("node_modules/@gpuix/native/package.json").is_file()
        && ui
            .join("node_modules/@gpuix/native/dist/host.d.ts")
            .is_file()
    {
        return Ok(());
    }
    stage("Install pinned GPUIX React dependencies");
    invoke("bun", &["install", "--exact"], &ui, verbose)
}

pub(crate) fn ensure_gpuix_js(root: &Path, verbose: bool) -> Result<(), String> {
    let gpuix = root.join("vendor/gpuix");
    if !gpuix.join("node_modules/typescript/bin/tsc").is_file() {
        stage("Install pinned GPUIX development dependencies");
        invoke("bun", &["install", "--frozen-lockfile"], &gpuix, verbose)?;
    }
    let native = gpuix.join("packages/native");
    if !native.join("dist/host.d.ts").is_file() {
        stage("Build GPUIX native JavaScript bindings");
        invoke("bun", &["run", "build:js"], &native, verbose)?;
    }
    Ok(())
}

pub(crate) fn runtime_library_path(engine: &Path) -> String {
    let mut paths = vec![path(&engine.join("lib64")), path(&engine.join("lib"))];
    for triplet in ["x64-linux-dynamic", "x64-linux-dynamic/debug"] {
        paths.push(path(
            &engine.join("vcpkg_installed").join(triplet).join("lib"),
        ));
    }
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        paths.push(existing.to_string_lossy().into_owned());
    }
    paths.join(":")
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
