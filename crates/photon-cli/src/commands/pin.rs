use std::path::Path;

use crate::support::{command, output, stage, success};

const DEPENDENCIES: [(&str, &str); 2] = [("Engine", "master"), ("vendor/gpui-ce", "main")];

pub(crate) fn pin(root: &Path, verbose: bool) -> Result<(), String> {
    let mut changed = Vec::new();
    for (relative, branch) in DEPENDENCIES {
        let repository = root.join(relative);
        let current = output("git", &["rev-parse", "HEAD"], &repository)?;
        let pinned = output("git", &["rev-parse", &format!("HEAD:{relative}")], root)?;
        if current.trim() == pinned.trim() {
            continue;
        }
        if !output(
            "git",
            &["status", "--porcelain", "--ignore-submodules=all"],
            &repository,
        )?
        .trim()
        .is_empty()
        {
            return Err(format!(
                "{relative} has local changes; commit or save them before pinning"
            ));
        }
        let current_branch = output("git", &["branch", "--show-current"], &repository)?;
        if current_branch.trim() != branch {
            return Err(format!(
                "{relative} must be on its Photon {branch} branch before pinning"
            ));
        }
        stage(&format!(
            "Verify {relative} is available on origin/{branch}"
        ));
        command(
            "git",
            &["fetch", "--no-tags", "origin", branch],
            &repository,
            verbose,
        )?;
        command(
            "git",
            &["merge-base", "--is-ancestor", "HEAD", "FETCH_HEAD"],
            &repository,
            verbose,
        )
        .map_err(|_| {
            format!(
                "{relative} HEAD is not on origin/{branch}; push the tested dependency commit first"
            )
        })?;
        changed.push(relative);
    }
    if changed.is_empty() {
        success("Dependency pins are already current");
        return Ok(());
    }

    stage("Validate Photon before advancing dependency pins");
    if changed.contains(&"Engine") {
        crate::commands::build::engine_build(root, false, verbose)?;
    }
    if changed.contains(&"vendor/gpui-ce") {
        let gpui_ce = root.join("vendor/gpui-ce");
        command("cargo", &["check", "-p", "gpui_ce_apple"], &gpui_ce, verbose)?;
        command("cargo", &["test", "-p", "gpui_ce_apple"], &gpui_ce, verbose)?;
    }
    crate::commands::check::check(root, verbose)?;
    crate::commands::tests::run(root, verbose)?;

    stage("Commit dependency pins");
    let mut commit = vec![
        "commit",
        "--only",
        "-m",
        "Pin tested Photon dependencies",
        "--",
    ];
    commit.extend(changed.iter().copied());
    command("git", &commit, root, verbose)?;
    let status = output("git", &["submodule", "status", "--recursive"], root)?;
    print!("{status}");
    success("Dependency pins committed");
    Ok(())
}
