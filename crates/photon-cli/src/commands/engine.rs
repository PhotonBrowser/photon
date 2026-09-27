use std::path::Path;

use crate::commands::build::engine_build;
use crate::support::{command, output};
use std::process::{Command, Stdio};

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
        _ => Err("usage: ./photon engine [status|build|sync]".into()),
    }
}
