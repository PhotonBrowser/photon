use std::path::Path;

use crate::support::{invoke, output, stage, success, with_progress};

pub(crate) fn check(root: &Path, verbose: bool) -> Result<(), String> {
    match check_inner(root, verbose) {
        Ok(()) => Ok(()),
        Err(error) => {
            print_fix_prompt(root, &error);
            Err(error)
        }
    }
}

fn check_inner(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Check Rust formatting");
    invoke(
        "cargo",
        &[
            "fmt",
            "--package",
            "photon-cli",
            "--package",
            "photon-core",
            "--package",
            "photon-app",
            "--package",
            "photon-presentation-broker",
            "--package",
            "photon-omnibox",
            "--",
            "--check",
        ],
        root,
        verbose,
    )?;
    let cargo_args = ["check", "--workspace"];
    with_progress("Check Rust workspace", !verbose, || {
        invoke("cargo", &cargo_args, root, verbose)
    })?;
    let engine = output("git", &["submodule", "status", "Engine"], root)?;
    if engine.starts_with('-') {
        return Err(format!(
            "Engine submodule is not initialized: {}. Run `./photon setup`.",
            engine.trim()
        ));
    }
    if engine.starts_with('+') {
        let engine_branch = output("git", &["branch", "--show-current"], &root.join("Engine"))?;
        if !crate::commands::engine::edit_mode(root) || engine_branch.trim() != "master" {
            return Err(format!(
                "Engine pin differs from the checkout: {}. Run `./photon engine edit` to use Engine/master, or `./photon engine pin` to restore the saved pin.",
                engine.trim()
            ));
        }
        println!("Using Engine/master development branch.");
    }
    let gpui_ce = output("git", &["submodule", "status", "vendor/gpui-ce"], root)?;
    if gpui_ce.starts_with('-') {
        return Err(format!("GPUI-CE submodule mismatch: {}", gpui_ce.trim()));
    }
    if gpui_ce.starts_with('+') {
        let branch = output(
            "git",
            &["branch", "--show-current"],
            &root.join("vendor/gpui-ce"),
        )?;
        if !crate::commands::gpui::edit_mode(root) || branch.trim() != "main" {
            return Err(format!("GPUI-CE submodule mismatch: {}", gpui_ce.trim()));
        }
        println!("Using Photon GPUI-CE main development branch.");
    }
    success("Formatting, workspace, and architecture checks passed");
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
