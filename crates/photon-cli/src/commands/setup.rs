use std::env;
use std::path::Path;

use crate::support::{command, compiler_version, output, require_tool, run_tool, version_at_least};

pub(crate) fn setup(root: &Path, verbose: bool) -> Result<(), String> {
    let engine = root.join("Engine");
    if !engine.join(".git").exists() {
        command(
            "git",
            &["submodule", "update", "--init", "--recursive"],
            root,
            true,
        )?;
    }
    let gpuix = root.join("vendor/gpuix");
    if !gpuix.join(".git").exists() {
        command(
            "git",
            &[
                "submodule",
                "update",
                "--init",
                "--recursive",
                "vendor/gpuix",
            ],
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
    require_tool("bun")?;
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
    crate::commands::ide::setup(root, verbose)?;
    crate::commands::build::ensure_gpuix_js(root, verbose)?;
    crate::commands::build::ensure_ui_dependencies(root, verbose)?;
    println!(
        "Setup ready. The first engine build prepares Ladybird's pinned dependencies under build/."
    );
    Ok(())
}

pub(crate) fn doctor(root: &Path) -> Result<(), String> {
    println!("Photon environment");
    println!("  OS         {}/{}", env::consts::OS, env::consts::ARCH);
    for (name, args) in [
        ("Rust", vec!["rustc", "--version"]),
        ("Cargo", vec!["cargo", "--version"]),
        ("CMake", vec!["cmake", "--version"]),
        ("Ninja", vec!["ninja", "--version"]),
        ("Bun", vec!["bun", "--version"]),
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
    crate::commands::ide::doctor(root);
    Ok(())
}
