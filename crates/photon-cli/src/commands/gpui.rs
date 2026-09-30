use std::path::Path;

use crate::support::{command, output};

const GPUIX_PATH: &str = "vendor/gpuix";
const ZED_PATH: &str = "vendor/gpuix/zed";
const GPUIX_BRANCH: &str = "main";
const ZED_BRANCH: &str = "photon/live-image";

pub(crate) fn edit(root: &Path, verbose: bool) -> Result<(), String> {
    let gpuix = root.join(GPUIX_PATH);
    let zed = root.join(ZED_PATH);
    require_repository(&gpuix)?;
    require_repository(&zed)?;
    if output("git", &["branch", "--show-current"], &gpuix)?.trim() == GPUIX_BRANCH
        && output("git", &["branch", "--show-current"], &zed)?.trim() == ZED_BRANCH
    {
        command(
            "git",
            &["config", "--local", "photon.gpuiEditMode", "true"],
            root,
            verbose,
        )?;
        return Ok(());
    }
    require_clean_source(&gpuix, "GPUIX")?;
    require_clean_source(&zed, "Zed")?;

    switch_branch(&gpuix, GPUIX_BRANCH, verbose)?;
    command(
        "git",
        &["submodule", "update", "--init", "--", "zed"],
        &gpuix,
        verbose,
    )?;
    switch_branch(&zed, ZED_BRANCH, verbose)?;
    command(
        "git",
        &["config", "--local", "photon.gpuiEditMode", "true"],
        root,
        verbose,
    )?;

    println!("GPUI edit mode: {GPUIX_PATH}/{GPUIX_BRANCH} and {ZED_PATH}/{ZED_BRANCH}.");
    println!(
        "Edit and commit GPUI under {ZED_PATH}/crates/gpui. Build and run against these branches without changing Photon’s saved pin."
    );
    println!("Run `./photon gpui pin` to return to this checkout's saved revisions.");
    Ok(())
}

pub(crate) fn pin(root: &Path, verbose: bool) -> Result<(), String> {
    let gpuix = root.join(GPUIX_PATH);
    let zed = root.join(ZED_PATH);
    require_repository(&gpuix)?;
    require_repository(&zed)?;
    require_clean_source(&gpuix, "GPUIX")?;
    require_clean_source(&zed, "Zed")?;

    let pinned_gpuix = output("git", &["rev-parse", "HEAD:vendor/gpuix"], root)?
        .trim()
        .to_owned();
    command(
        "git",
        &["checkout", "--detach", &pinned_gpuix],
        &gpuix,
        verbose,
    )?;
    command(
        "git",
        &["submodule", "update", "--init", "--", "zed"],
        &gpuix,
        verbose,
    )?;
    unset_edit_mode(root, verbose)?;
    println!("GPUIX and Zed are back at the revisions pinned by this Photon checkout.");
    Ok(())
}

pub(crate) fn sync(root: &Path, verbose: bool) -> Result<(), String> {
    let gpuix = root.join(GPUIX_PATH);
    let zed = root.join(ZED_PATH);
    require_repository(&gpuix)?;
    require_repository(&zed)?;
    require_branch(&gpuix, GPUIX_BRANCH)?;
    require_branch(&zed, ZED_BRANCH)?;
    require_clean_source(&gpuix, "GPUIX")?;
    require_clean_source(&zed, "Zed")?;

    command(
        "git",
        &["fetch", "upstream", "gpuix:refs/remotes/upstream/gpuix"],
        &zed,
        verbose,
    )?;
    command("git", &["merge", "--no-edit", "FETCH_HEAD"], &zed, verbose)?;
    command(
        "git",
        &["fetch", "upstream", "main:refs/remotes/upstream/main"],
        &gpuix,
        verbose,
    )?;
    command(
        "git",
        &["merge", "--no-edit", "FETCH_HEAD"],
        &gpuix,
        verbose,
    )?;
    command("git", &["add", "zed"], &gpuix, verbose)?;

    println!("Merged upstream Zed and GPUIX into the persistent Photon branches.");
    println!(
        "Review `git -C {GPUIX_PATH} diff --cached --submodule=log`, then commit the updated Zed pointer."
    );
    println!(
        "Run the GPUIX checks before pushing either branch. The Photon root pin stays unchanged."
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

fn require_clean_source(repository: &Path, name: &str) -> Result<(), String> {
    let status = output(
        "git",
        &["status", "--porcelain", "--ignore-submodules=all"],
        repository,
    )?;
    if status.trim().is_empty() {
        return Ok(());
    }
    Err(format!(
        "{name} has uncommitted source changes; commit or save them before switching GPUI revisions"
    ))
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
            "photon.gpuiEditMode",
        ],
        root,
    )
    .is_ok_and(|value| value.trim() == "true")
}

fn unset_edit_mode(root: &Path, verbose: bool) -> Result<(), String> {
    if edit_mode(root) {
        command(
            "git",
            &["config", "--local", "--unset-all", "photon.gpuiEditMode"],
            root,
            verbose,
        )?;
    }
    Ok(())
}

fn switch_branch(repository: &Path, branch: &str, verbose: bool) -> Result<(), String> {
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let local_exists = output(
        "git",
        &["show-ref", "--verify", "--quiet", &local_ref],
        repository,
    )
    .is_ok();
    if local_exists {
        return command("git", &["switch", branch], repository, verbose);
    }

    let remote_exists = output(
        "git",
        &["show-ref", "--verify", "--quiet", &remote_ref],
        repository,
    )
    .is_ok();
    if !remote_exists {
        return Err(format!(
            "branch origin/{branch} is not available in {}; fetch origin before entering GPUI edit mode",
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
