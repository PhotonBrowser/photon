use std::path::Path;

use crate::support::{invoke, output, with_progress};

pub(crate) fn check(root: &Path, verbose: bool) -> Result<(), String> {
    check_with_engine(root, verbose, true)
}

pub(crate) fn check_ui(root: &Path, verbose: bool) -> Result<(), String> {
    check_with_engine(root, verbose, false)
}

fn check_with_engine(root: &Path, verbose: bool, engine_enabled: bool) -> Result<(), String> {
    match check_inner(root, verbose, engine_enabled) {
        Ok(()) => Ok(()),
        Err(error) => {
            print_fix_prompt(root, &error);
            Err(error)
        }
    }
}

fn check_inner(root: &Path, verbose: bool, engine_enabled: bool) -> Result<(), String> {
    crate::commands::build::ensure_gpuix_js(root, verbose)?;
    crate::commands::build::ensure_ui_dependencies(root, verbose)?;
    invoke(
        "cargo",
        &[
            "fmt",
            "--package",
            "photon-cli",
            "--package",
            "photon-core",
            "--package",
            "photon-gpui",
            "--package",
            "photon-native-addon",
            "--",
            "--check",
        ],
        root,
        verbose,
    )?;
    let mut cargo_args = vec!["check", "--workspace"];
    if !engine_enabled {
        cargo_args.push("--no-default-features");
    }
    with_progress("Check Rust workspace", !verbose, || {
        invoke("cargo", &cargo_args, root, verbose)
    })?;
    let ui = root.join("ui");
    invoke("bun", &["run", "lint"], &ui, verbose)?;
    invoke("bun", &["run", "typecheck"], &ui, verbose)?;
    if engine_enabled {
        let submodule = output("git", &["submodule", "status", "Engine"], root)?;
        if submodule.starts_with('-') {
            return Err(format!(
                "Engine submodule is not initialized: {}. Run `./photon setup`.",
                submodule.trim()
            ));
        }
        if submodule.starts_with('+') {
            let engine_branch = output("git", &["branch", "--show-current"], &root.join("Engine"))?;
            if !crate::commands::engine::edit_mode(root) || engine_branch.trim() != "master" {
                return Err(format!(
                    "Engine pin differs from the checkout: {}. Run `./photon engine edit` to use Engine/master, or `./photon engine pin` to restore the saved pin.",
                    submodule.trim()
                ));
            }
            println!("Using Engine/master development branch.");
        }
    }
    let gpuix = output("git", &["submodule", "status", "vendor/gpuix"], root)?;
    if gpuix.starts_with('-') {
        return Err(format!("GPUIX submodule mismatch: {}", gpuix.trim()));
    }
    if gpuix.starts_with('+') {
        let gpuix_branch = output(
            "git",
            &["branch", "--show-current"],
            &root.join("vendor/gpuix"),
        )?;
        let zed_branch = output(
            "git",
            &["branch", "--show-current"],
            &root.join("vendor/gpuix/zed"),
        )?;
        if !crate::commands::gpui::edit_mode(root)
            || gpuix_branch.trim() != "main"
            || zed_branch.trim() != "photon/live-image"
        {
            return Err(format!("GPUIX submodule mismatch: {}", gpuix.trim()));
        }
        println!("Using GPUIX main and Photon Zed branches in local edit mode.");
    }
    println!("Architecture checks passed.");
    Ok(())
}

pub(crate) fn print_fix_prompt(root: &Path, diagnostics: &str) {
    let instructions = std::fs::read_to_string(root.join("AGENTS.md")).unwrap_or_default();
    println!("\n--- Copy the prompt below into your AI agent ---\n");
    println!(
        "Fix the failing Photon check in this repository. Inspect the relevant files, make the smallest correct change, and rerun `./photon check`. Explain what you changed and report the check result. Preserve the project architecture and avoid unrelated edits.\n"
    );
    println!("Project instructions:\n{instructions}");
    println!("Check failure / diagnostics:\n{diagnostics}");
    println!("--- End prompt ---");
}
