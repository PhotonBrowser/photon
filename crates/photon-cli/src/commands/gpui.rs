use std::path::Path;

use crate::support::{command, output};

const GPUIX_PATH: &str = "vendor/gpuix";
const ZED_PATH: &str = "vendor/gpuix/zed";
const GPUX_BRANCH: &str = "main";
const ZED_BRANCH: &str = "photon/live-image";

pub(crate) fn edit(root: &Path, verbose: bool) -> Result<(), String> {
    let gpuix = root.join(GPUIX_PATH);
    let zed = root.join(ZED_PATH);
    require_repository(&gpuix)?;
    require_repository(&zed)?;
    require_clean_source(&gpuix, "GPUIX")?;
    require_clean_source(&zed, "Zed")?;

    switch_branch(&gpuix, GPUX_BRANCH, verbose)?;
    command(
        "git",
        &["submodule", "update", "--init", "--", "zed"],
        &gpuix,
        verbose,
    )?;
    switch_branch(&zed, ZED_BRANCH, verbose)?;

    println!("GPUI edit mode: {GPUIX_PATH}/{GPUX_BRANCH} and {ZED_PATH}/{ZED_BRANCH}.");
    println!(
        "Edit GPUI under {ZED_PATH}/crates/gpui, commit there, then commit the `zed` pin in GPUIX."
    );
    println!("Run `./photon gpui pin` to return to this checkout's pinned revisions.");
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
    println!("GPUIX and Zed are back at the revisions pinned by this Photon checkout.");
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
