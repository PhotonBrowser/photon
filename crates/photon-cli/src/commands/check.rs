use std::path::Path;

use crate::support::{invoke, output, with_progress};

pub(crate) fn check(root: &Path, verbose: bool) -> Result<(), String> {
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
    crate::commands::ide::refresh(root, verbose)?;
    crate::commands::ide::check_cpp(root, verbose)?;
    invoke("cargo", &["fmt", "--all", "--", "--check"], root, verbose)?;
    with_progress("Check Rust workspace", !verbose, || {
        invoke("cargo", &["check", "--workspace"], root, verbose)
    })?;
    println!("==> Lint QML");
    let lint = photon_qml_tools::lint(root)?;
    print!("{}", lint.diagnostics);
    diagnostics.push_str(&lint.diagnostics);
    if !lint.succeeded
        || lint
            .diagnostics
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
