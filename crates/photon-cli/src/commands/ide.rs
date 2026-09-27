use std::path::Path;

use crate::support::{invoke, output, path, stage};

pub(crate) fn setup(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Prepare IDE metadata");
    output(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
        root,
    )
    .map_err(|error| format!("Cargo workspace metadata is unavailable: {error}"))?;
    refresh(root, verbose)?;

    println!("IDE setup ready:");
    println!("  clangd  {}", root.join(".clangd").display());
    println!(
        "  C++     {}",
        root.join("build/app-debug/compile_commands.json").display()
    );
    println!("  qmlls   {}", root.join("ui/.qmlls.ini").display());
    println!(
        "  QML     {}",
        root.join("build/app-debug/Photon/qmldir").display()
    );
    println!("  Rust    {}", root.join("Cargo.toml").display());
    Ok(())
}

/// Configure the canonical debug app tree and refresh generated QML type data.
pub(crate) fn refresh(root: &Path, verbose: bool) -> Result<(), String> {
    let app_build = root.join("build/app-debug");
    let engine_build = root.join("build/engine-debug");
    let helper_directory = if cfg!(any(target_os = "macos", target_os = "windows")) {
        &app_build
    } else {
        &root.join("build/engine-debug/bin")
    };
    invoke(
        "cmake",
        &[
            "-S",
            &path(&root.join("native/qt")),
            "-B",
            &path(&app_build),
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Debug",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
            &format!("-DPHOTON_ENGINE_BUILD_DIR={}", path(&engine_build)),
            &format!("-DPHOTON_HELPER_DIRECTORY={}", path(helper_directory)),
        ],
        root,
        verbose,
    )?;

    invoke(
        "cmake",
        &[
            "--build",
            &path(&app_build),
            "--target",
            "photon_qmltyperegistration",
        ],
        root,
        verbose,
    )?;

    write_qmlls_config(root, &app_build)?;
    validate_generated_files(root)?;
    Ok(())
}

pub(crate) fn check_cpp(root: &Path, verbose: bool) -> Result<(), String> {
    invoke(
        "cmake",
        &[
            "--build",
            &path(&root.join("build/app-debug")),
            "--target",
            "photon_cpp_check",
        ],
        root,
        verbose,
    )
}

pub(crate) fn remove_generated_config(root: &Path) -> Result<(), String> {
    let config = root.join("ui/.qmlls.ini");
    if config.exists() {
        std::fs::remove_file(config)
            .map_err(|error| format!("cannot remove qmlls config: {error}"))?;
    }
    Ok(())
}

pub(crate) fn doctor(root: &Path) {
    println!("IDE");
    let mut setup_needed = false;
    let clangd = output("clangd", &["--version"], root).ok();
    report(
        "clangd server",
        clangd.is_some(),
        clangd
            .as_deref()
            .and_then(|version| version.lines().next())
            .unwrap_or("not found; install the recommended clangd VS Code extension"),
    );
    let compile_db = root.join("build/app-debug/compile_commands.json");
    setup_needed |= !report(
        "clangd configuration",
        std::fs::read_to_string(root.join(".clangd"))
            .map(|contents| contents.contains("CompilationDatabase: build/app-debug"))
            .unwrap_or(false),
        &root.join(".clangd").display().to_string(),
    );
    setup_needed |= !report(
        "app compile database",
        compile_db.is_file(),
        &compile_db.display().to_string(),
    );

    let qmlls_config = root.join("ui/.qmlls.ini");
    let expected_build = root
        .join("build/app-debug")
        .canonicalize()
        .unwrap_or_else(|_| root.join("build/app-debug"));
    let expected_build = expected_build
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let configured_build = std::fs::read_to_string(&qmlls_config)
        .map(|contents| {
            contents.contains(&format!("buildDir=\"{expected_build}\""))
                && contents.contains("no-cmake-calls=true")
        })
        .unwrap_or(false);
    setup_needed |= !report(
        "qmlls configuration",
        configured_build,
        &qmlls_config.display().to_string(),
    );
    let qmlls = photon_qml_tools::tool_path("qmlls", root);
    setup_needed |= !report(
        "qmlls executable",
        qmlls.is_some(),
        qmlls
            .as_deref()
            .unwrap_or("not found; install Qt 6 QML development tools"),
    );
    setup_needed |= !report_generated(
        root,
        "QML generated module",
        "build/app-debug/Photon/qmldir",
    );
    setup_needed |= !report_generated(
        root,
        "QML type metadata",
        "build/app-debug/Photon/photon.qmltypes",
    );
    setup_needed |= !report_generated(
        root,
        "qmlls build metadata",
        "build/app-debug/.qt/.qmlls.build.ini",
    );
    let engine_header = [
        root.join("build/engine-debug/Libraries/LibPhotonEmbedder/Export.h"),
        root.join("build/app-debug/photon_ide/include/LibPhotonEmbedder/Export.h"),
    ]
    .into_iter()
    .find(|path| path.is_file());
    setup_needed |= !report(
        "generated C++ API header",
        engine_header.is_some(),
        engine_header
            .as_ref()
            .map(|path| path.display().to_string())
            .as_deref()
            .unwrap_or("not generated; run `./photon ide setup`"),
    );

    let rust_workspace = output(
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1"],
        root,
    )
    .is_ok();
    setup_needed |= !report(
        "Rust workspace",
        rust_workspace,
        &root.join("Cargo.toml").display().to_string(),
    );

    let engine_db = root.join("build/engine-debug/compile_commands.json");
    let engine_clangd = root.join("Engine/.clangd");
    report(
        "Engine workspace separation",
        engine_clangd.is_file() && root.join("Engine/Cargo.toml").is_file(),
        &format!(
            "{} and {}; Photon app database stays separate",
            engine_clangd.display(),
            root.join("Engine/Cargo.toml").display()
        ),
    );
    println!(
        "  ⚠ Engine clangd: Engine/.clangd expects Engine/Build/release; use {} in a separate Engine window{}",
        engine_db.display(),
        if engine_db.is_file() {
            String::new()
        } else {
            " (run `./photon engine build` first)".to_owned()
        }
    );
    if setup_needed {
        println!("  Run: ./photon ide setup");
    }
}

fn write_qmlls_config(root: &Path, build_dir: &Path) -> Result<(), String> {
    let build_dir = build_dir
        .canonicalize()
        .map_err(|error| format!("cannot resolve app build directory: {error}"))?;
    let build_dir = build_dir
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let config = root.join("ui/.qmlls.ini");
    std::fs::write(
        &config,
        format!("[General]\nbuildDir=\"{build_dir}\"\nno-cmake-calls=true\n"),
    )
    .map_err(|error| format!("cannot write qmlls config: {error}"))
}

fn validate_generated_files(root: &Path) -> Result<(), String> {
    for relative in [
        "build/app-debug/compile_commands.json",
        "build/app-debug/.qt/.qmlls.build.ini",
        "build/app-debug/Photon/qmldir",
        "build/app-debug/Photon/photon.qmltypes",
        "build/app-debug/photon_ide/include/LibPhotonEmbedder/Export.h",
        "ui/.qmlls.ini",
    ] {
        if !root.join(relative).is_file()
            && !(relative == "build/app-debug/photon_ide/include/LibPhotonEmbedder/Export.h"
                && root
                    .join("build/engine-debug/Libraries/LibPhotonEmbedder/Export.h")
                    .is_file())
        {
            return Err(format!(
                "IDE metadata is missing {}; run `./photon ide setup` after checking Qt and CMake",
                root.join(relative).display()
            ));
        }
    }
    Ok(())
}

fn report_generated(root: &Path, label: &str, relative: &str) -> bool {
    let path = root.join(relative);
    report(label, path.is_file(), &path.display().to_string())
}

fn report(label: &str, ok: bool, detail: &str) -> bool {
    println!("  {} {label}: {detail}", if ok { "✓" } else { "✗" });
    ok
}
