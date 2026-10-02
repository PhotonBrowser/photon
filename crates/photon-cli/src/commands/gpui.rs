use std::path::Path;

use crate::support::{command, output};

const GPUI_CE_PATH: &str = "vendor/gpui-ce";
const GPUI_CE_BRANCH: &str = "main";

pub(crate) fn edit(root: &Path, verbose: bool) -> Result<(), String> {
    let gpui_ce = root.join(GPUI_CE_PATH);
    require_repository(&gpui_ce)?;
    if output("git", &["branch", "--show-current"], &gpui_ce)?.trim() != GPUI_CE_BRANCH {
        require_clean_source(&gpui_ce)?;
        switch_branch(&gpui_ce, GPUI_CE_BRANCH, verbose)?;
    }
    command(
        "git",
        &["config", "--local", "photon.gpuiCeEditMode", "true"],
        root,
        verbose,
    )?;
    println!("GPUI-CE edit mode: {GPUI_CE_PATH}/{GPUI_CE_BRANCH}.");
    Ok(())
}

pub(crate) fn pin(root: &Path, verbose: bool) -> Result<(), String> {
    let gpui_ce = root.join(GPUI_CE_PATH);
    require_repository(&gpui_ce)?;
    require_clean_source(&gpui_ce)?;
    let pinned = output("git", &["rev-parse", "HEAD:vendor/gpui-ce"], root)?
        .trim()
        .to_owned();
    command("git", &["checkout", "--detach", &pinned], &gpui_ce, verbose)?;
    unset_edit_mode(root, verbose)?;
    println!("GPUI-CE is back at the revision pinned by this Photon checkout.");
    Ok(())
}

pub(crate) fn sync(root: &Path, verbose: bool) -> Result<(), String> {
    let gpui_ce = root.join(GPUI_CE_PATH);
    require_repository(&gpui_ce)?;
    require_branch(&gpui_ce, GPUI_CE_BRANCH)?;
    require_clean_source(&gpui_ce)?;
    command(
        "git",
        &["fetch", "upstream", "main:refs/remotes/upstream/main"],
        &gpui_ce,
        verbose,
    )?;
    command(
        "git",
        &["merge", "--no-edit", "FETCH_HEAD"],
        &gpui_ce,
        verbose,
    )?;
    println!("Merged upstream GPUI-CE main into the persistent Photon branch.");
    println!(
        "Run `cargo check -p gpui_ce_apple` and `cargo test -p gpui_ce_apple` before pushing."
    );
    Ok(())
}

fn require_repository(path: &Path) -> Result<(), String> {
    if path.join(".git").exists() {
        Ok(())
    } else {
        Err(format!(
            "{} is not initialized; run `./photon setup` first",
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
        Err(format!(
            "GPUI-CE has uncommitted changes; commit or save them before switching revisions"
        ))
    }
}

fn require_branch(repository: &Path, expected: &str) -> Result<(), String> {
    let current = output("git", &["branch", "--show-current"], repository)?;
    if current.trim() == expected {
        Ok(())
    } else {
        Err(format!(
            "{} is on branch {:?}; run `./photon gpui edit` first to use {expected}",
            repository.display(),
            current.trim()
        ))
    }
}

pub(crate) fn edit_mode(root: &Path) -> bool {
    output(
        "git",
        &[
            "config",
            "--local",
            "--bool",
            "--get",
            "photon.gpuiCeEditMode",
        ],
        root,
    )
    .is_ok_and(|value| value.trim() == "true")
}

fn unset_edit_mode(root: &Path, verbose: bool) -> Result<(), String> {
    if edit_mode(root) {
        command(
            "git",
            &["config", "--local", "--unset-all", "photon.gpuiCeEditMode"],
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
