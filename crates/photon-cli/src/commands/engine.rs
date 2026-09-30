use std::path::Path;

use crate::commands::build::engine_build;
use crate::support::{command, output};
use std::process::{Command, Stdio};

const ENGINE_PATH: &str = "Engine";
const ENGINE_BRANCH: &str = "master";

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
            let branch = output("git", &["branch", "--show-current"], &engine)?;
            if branch.trim() != ENGINE_BRANCH {
                return Err(format!(
                    "Engine is on branch {:?}; run `./photon engine edit` before syncing",
                    branch.trim()
                ));
            }
            ensure_no_engine_build(root)?;
            if !output("git", &["status", "--porcelain"], &engine)?
                .trim()
                .is_empty()
            {
                return Err("Engine working tree must be clean before sync".into());
            }
            command("git", &["fetch", "upstream"], &engine, true)?;
            let behind = output(
                "git",
                &["rev-list", "--count", "HEAD..upstream/master"],
                &engine,
            )?;
            if behind.trim() == "0" {
                println!("Engine is already up to date with upstream/master.");
                return Ok(());
            }
            let current = output("git", &["rev-parse", "--short", "HEAD"], &engine)?;
            let target = output("git", &["rev-parse", "--short", "upstream/master"], &engine)?;
            println!("Merging Engine {current:.7} with upstream/master {target:.7}");
            let merge = Command::new("git")
                .args(["merge", "--no-edit", "upstream/master"])
                .current_dir(&engine)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|error| format!("cannot run git merge: {error}"))?;
            if !merge.status.success() {
                let conflicts =
                    output("git", &["diff", "--name-only", "--diff-filter=U"], &engine)?;
                if conflicts.trim().is_empty() {
                    return Err(format!(
                        "git merge failed:\n{}{}",
                        String::from_utf8_lossy(&merge.stdout),
                        String::from_utf8_lossy(&merge.stderr)
                    ));
                }
                let combined = output("git", &["diff", "--cc"], &engine).unwrap_or_default();
                let paths = conflicts.lines().collect::<Vec<_>>().join("\n");
                let diagnostics = format!(
                    "Upstream merge is paused with conflicts. Resolve these paths, then stage them and run `git commit` to complete the merge.\n\nConflicting paths:\n{paths}\n\nCombined conflict hunks:\n{combined}\n\nGit output:\n{}{}",
                    String::from_utf8_lossy(&merge.stdout),
                    String::from_utf8_lossy(&merge.stderr)
                );
                println!("{diagnostics}");
                crate::commands::check::print_fix_prompt(
                    root,
                    &format!(
                        "Resolve the in-progress Ladybird upstream merge in the Engine checkout. Preserve Photon-specific changes and follow the repository instructions. Resolve only the listed conflict paths, stage the resolutions, and complete the merge commit. Do not abort the merge.\n\n{diagnostics}"
                    ),
                );
                return Err("upstream merge has conflicts; an AI repair prompt was printed".into());
            }
            println!("Sync complete. Update and commit the Engine submodule pointer in photon.");
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
