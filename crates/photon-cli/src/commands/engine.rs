use std::path::Path;

use crate::commands::build::engine_build;
use crate::support::{command, output};

const ENGINE_PATH: &str = "Engine";
const ENGINE_BRANCH: &str = "master";

pub(crate) fn check_upstream(root: &Path, verbose: bool) -> Result<(), String> {
    let engine = root.join(ENGINE_PATH);
    require_repository(&engine)?;
    crate::commands::sync::check_upstream(&engine, "Engine", ENGINE_BRANCH, verbose)
}

pub(crate) fn edit(root: &Path, verbose: bool) -> Result<(), String> {
    let engine = root.join(ENGINE_PATH);
    require_repository(&engine)?;
    let current = output("git", &["branch", "--show-current"], &engine)?;
    if current.trim() != ENGINE_BRANCH {
        ensure_no_engine_build(root)?;
        require_clean(&engine)?;
        switch_branch(&engine, ENGINE_BRANCH, verbose)?;
    }
    command(
        "git",
        &["config", "--local", "photon.engineEditMode", "true"],
        root,
        verbose,
    )?;
    println!("Engine edit mode: {ENGINE_PATH}/{ENGINE_BRANCH}.");
    println!("Run `./photon engine pin` to return to this checkout's saved revision.");
    Ok(())
}

pub(crate) fn pin(root: &Path, verbose: bool) -> Result<(), String> {
    let engine = root.join(ENGINE_PATH);
    require_repository(&engine)?;
    ensure_no_engine_build(root)?;
    require_clean(&engine)?;
    let pinned = output("git", &["rev-parse", "HEAD:Engine"], root)?
        .trim()
        .to_owned();
    command("git", &["checkout", "--detach", &pinned], &engine, verbose)?;
    unset_edit_mode(root, verbose)?;
    println!("Engine is back at the revision pinned by this Photon checkout.");
    Ok(())
}

pub(crate) fn engine(
    root: &Path,
    sub: Option<&str>,
    release: bool,
    verbose: bool,
) -> Result<(), String> {
    let engine = root.join("Engine");
    match sub.unwrap_or("status") {
        "status" => {
            for args in [
                &["status", "--short", "--branch"][..],
                &["remote", "-v"],
                &["rev-parse", "--short", "HEAD"],
            ] {
                print!("{}", output("git", args, &engine)?);
            }
            let pinned = output("git", &["rev-parse", "HEAD:Engine"], root)?;
            println!("Photon pin {}", pinned.trim());
            println!(
                "edit mode {}",
                if edit_mode(root) {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            if let (Ok(a), Ok(b)) = (
                output(
                    "git",
                    &["rev-list", "--count", "HEAD..upstream/master"],
                    &engine,
                ),
                output(
                    "git",
                    &["rev-list", "--count", "upstream/master..HEAD"],
                    &engine,
                ),
            ) {
                println!("upstream behind {}, ahead {}", a.trim(), b.trim());
            } else {
                println!("upstream counts unavailable; fetch upstream to refresh tracking refs");
            }
            Ok(())
        }
        "build" => engine_build(root, release, verbose),
        "sync" => {
            ensure_no_engine_build(root)?;
            edit(root, verbose)?;
            if !output("git", &["status", "--porcelain"], &engine)?
                .trim()
                .is_empty()
            {
                return Err("Engine working tree must be clean before sync".into());
            }
            crate::commands::sync::merge_upstream(&engine, "Engine", ENGINE_BRANCH, verbose)?;
            println!("Push the tested Engine branch, then run `./photon pin`.");
            Ok(())
        }
        _ => Err("usage: ./photon engine [status|edit|pin|build|sync]".into()),
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
            "photon.engineEditMode",
        ],
        root,
    )
    .is_ok_and(|value| value.trim() == "true")
}

fn unset_edit_mode(root: &Path, verbose: bool) -> Result<(), String> {
    if edit_mode(root) {
        command(
            "git",
            &["config", "--local", "--unset-all", "photon.engineEditMode"],
            root,
            verbose,
        )?;
    }
    Ok(())
}

fn require_repository(engine: &Path) -> Result<(), String> {
    if engine.join(".git").exists() {
        Ok(())
    } else {
        Err("Engine is not initialized; run `./photon setup` first".into())
    }
}

fn require_clean(engine: &Path) -> Result<(), String> {
    let status = output(
        "git",
        &["status", "--porcelain", "--ignore-submodules=all"],
        engine,
    )?;
    if status.trim().is_empty() {
        Ok(())
    } else {
        Err("Engine has local changes; commit or save them before switching revisions".into())
    }
}

fn switch_branch(engine: &Path, branch: &str, verbose: bool) -> Result<(), String> {
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if output(
        "git",
        &["show-ref", "--verify", "--quiet", &local_ref],
        engine,
    )
    .is_ok()
    {
        return command("git", &["switch", branch], engine, verbose);
    }
    if output(
        "git",
        &["show-ref", "--verify", "--quiet", &remote_ref],
        engine,
    )
    .is_ok()
    {
        return command(
            "git",
            &[
                "switch",
                "--track",
                "-c",
                branch,
                &format!("origin/{branch}"),
            ],
            engine,
            verbose,
        );
    }
    Err(format!(
        "Photon Engine branch origin/{branch} is not available; fetch origin first"
    ))
}

fn ensure_no_engine_build(root: &Path) -> Result<(), String> {
    let args: &[&str] = if cfg!(target_os = "macos") {
        &["-axo", "command="]
    } else {
        &["-eo", "args"]
    };
    let processes = output("ps", args, root)?;
    let building = processes.lines().any(|process| {
        let process = process.to_ascii_lowercase();
        (process.contains("ninja") || process.contains("cmake"))
            && (process.contains("build/engine-debug") || process.contains("build/engine-release"))
    });
    if building {
        Err("An Engine CMake or Ninja build is active; wait for it to finish or stop it cleanly before switching or syncing Engine.".into())
    } else {
        Ok(())
    }
}
