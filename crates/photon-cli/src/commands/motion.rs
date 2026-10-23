use std::path::Path;

use crate::support::{command, output};

const MOTION_PATH: &str = "crates/gpui-motion";
const MOTION_BRANCH: &str = "main";

pub(crate) fn check_upstream(root: &Path, verbose: bool) -> Result<(), String> {
    let repository = root.join(MOTION_PATH);
    require_repository(&repository)?;
    crate::commands::sync::check_upstream(&repository, "gpui-motion", MOTION_BRANCH, verbose)
}

pub(crate) fn edit(root: &Path, verbose: bool) -> Result<(), String> {
    let repository = root.join(MOTION_PATH);
    require_repository(&repository)?;
    if output("git", &["branch", "--show-current"], &repository)?.trim() != MOTION_BRANCH {
        require_clean_source(&repository)?;
        switch_branch(&repository, MOTION_BRANCH, verbose)?;
    }
    command(
        "git",
        &["config", "--local", "photon.gpuiMotionEditMode", "true"],
        root,
        verbose,
    )?;
    println!("gpui-motion edit mode: {MOTION_PATH}/{MOTION_BRANCH}.");
    Ok(())
}

pub(crate) fn pin(root: &Path, verbose: bool) -> Result<(), String> {
    let repository = root.join(MOTION_PATH);
    require_repository(&repository)?;
    require_clean_source(&repository)?;
    let pinned = output("git", &["rev-parse", "HEAD:crates/gpui-motion"], root)?
        .trim()
        .to_owned();
    command(
        "git",
        &["checkout", "--detach", &pinned],
        &repository,
        verbose,
    )?;
    unset_edit_mode(root, verbose)?;
    println!("gpui-motion is back at the revision pinned by this Photon checkout.");
    Ok(())
}

pub(crate) fn sync(root: &Path, verbose: bool) -> Result<(), String> {
    let repository = root.join(MOTION_PATH);
    require_repository(&repository)?;
    edit(root, verbose)?;
    require_clean_source(&repository)?;
    crate::commands::sync::merge_upstream(&repository, "gpui-motion", MOTION_BRANCH, verbose)?;
    println!(
        "Run gpui-motion checks against vendor/gpui-ce, push main to PhotonBrowser/gpui-motion, then run `photon pin`."
    );
    Ok(())
}

fn require_repository(path: &Path) -> Result<(), String> {
    if path.join(".git").exists() {
        Ok(())
    } else {
        Err(format!(
            "{} is not initialized; run `photon setup` first",
            path.display()
        ))
    }
}

fn require_clean_source(repository: &Path) -> Result<(), String> {
    let status = output(
        "git",
        &["status", "--porcelain", "--ignore-submodules=all"],
        repository,
    )?;
    if status.trim().is_empty() {
        Ok(())
    } else {
        Err(
            "gpui-motion has uncommitted changes; commit or save them before switching revisions"
                .into(),
        )
    }
}

fn unset_edit_mode(root: &Path, verbose: bool) -> Result<(), String> {
    if output(
        "git",
        &["config", "--bool", "--get", "photon.gpuiMotionEditMode"],
        root,
    )
    .is_ok_and(|value| value.trim() == "true")
    {
        command(
            "git",
            &[
                "config",
                "--local",
                "--unset-all",
                "photon.gpuiMotionEditMode",
            ],
            root,
            verbose,
        )?;
    }
    Ok(())
}

fn switch_branch(repository: &Path, branch: &str, verbose: bool) -> Result<(), String> {
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if output(
        "git",
        &["show-ref", "--verify", "--quiet", &local_ref],
        repository,
    )
    .is_ok()
    {
        return command("git", &["switch", branch], repository, verbose);
    }
    if output(
        "git",
        &["show-ref", "--verify", "--quiet", &remote_ref],
        repository,
    )
    .is_err()
    {
        return Err(format!(
            "branch origin/{branch} is not available in {}; fetch origin first",
            repository.display()
        ));
    }
    command(
        "git",
        &[
            "switch",
            "--track",
            "-c",
            branch,
            &format!("origin/{branch}"),
        ],
        repository,
        verbose,
    )
}
