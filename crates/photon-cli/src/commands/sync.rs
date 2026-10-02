use std::path::Path;
use std::process::Command;

use crate::support::{command, output, stage, success};

fn fetch_upstream(
    repository: &Path,
    name: &str,
    branch: &str,
    verbose: bool,
) -> Result<String, String> {
    stage(&format!("Fetch {name} upstream/{branch}"));
    let refspec = format!("{branch}:refs/remotes/upstream/{branch}");
    command(
        "git",
        &["fetch", "--no-tags", "upstream", &refspec],
        repository,
        verbose,
    )?;
    Ok(format!("upstream/{branch}"))
}

pub(crate) fn check_upstream(
    repository: &Path,
    name: &str,
    branch: &str,
    verbose: bool,
) -> Result<(), String> {
    let upstream = fetch_upstream(repository, name, branch, verbose)?;
    let source = format!("refs/heads/{branch}");
    output("git", &["rev-parse", "--verify", &source], repository)
        .map_err(|_| format!("{name} has no local {branch} branch; run `./photon setup` first"))?;
    let counts = output(
        "git",
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{source}...{upstream}"),
        ],
        repository,
    )?;
    let (ahead, behind) = counts
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("cannot read {name} upstream commit counts: {counts:?}"))?;
    println!("{name} ({branch} vs {upstream}): {behind} behind, {ahead} ahead");
    let current_branch = output("git", &["branch", "--show-current"], repository)?;
    if current_branch.trim() != branch {
        println!(
            "  Current checkout is on {}; preview uses {branch}.",
            if current_branch.trim().is_empty() {
                "detached HEAD"
            } else {
                current_branch.trim()
            }
        );
    }
    if !output(
        "git",
        &["status", "--porcelain", "--ignore-submodules=all"],
        repository,
    )?
    .trim()
    .is_empty()
    {
        println!("  Local uncommitted changes are excluded from this merge preview.");
    }
    if behind == "0" {
        success(&format!("{name}: no upstream commits to merge"));
        return Ok(());
    }
    let preview = Command::new("git")
        .args([
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            &source,
            &upstream,
        ])
        .current_dir(repository)
        .output()
        .map_err(|error| format!("cannot preview {name} merge: {error}"))?;
    match preview.status.code() {
        Some(0) => success(&format!("{name}: no merge conflicts predicted")),
        Some(1) if !preview.stdout.is_empty() && preview.stderr.is_empty() => {
            println!("  Predicted merge conflicts:");
            let text = String::from_utf8_lossy(&preview.stdout);
            let mut paths = text
                .lines()
                .skip(1)
                .filter(|line| !line.trim().is_empty())
                .peekable();
            if paths.peek().is_none() {
                println!(
                    "    Git reported conflicts; inspect with `git merge-tree --write-tree {source} {upstream}`."
                );
            } else {
                for path in paths {
                    println!("    • {path}");
                }
            }
        }
        _ => {
            return Err(format!(
                "cannot preview {name} merge: {}",
                String::from_utf8_lossy(&preview.stderr).trim()
            ));
        }
    }
    Ok(())
}

pub(crate) fn merge_upstream(
    repository: &Path,
    name: &str,
    branch: &str,
    verbose: bool,
) -> Result<(), String> {
    let upstream = fetch_upstream(repository, name, branch, verbose)?;
    let behind = output(
        "git",
        &["rev-list", "--count", &format!("HEAD..{upstream}")],
        repository,
    )?;
    if behind.trim() == "0" {
        success(&format!("{name} is current with {upstream}"));
        return Ok(());
    }

    let before = output("git", &["rev-parse", "--short", "HEAD"], repository)?;
    let target = output("git", &["rev-parse", "--short", &upstream], repository)?;
    stage(&format!(
        "Merge {name} {} → {}",
        before.trim(),
        target.trim()
    ));
    let merge = Command::new("git")
        .args(["merge", "--no-edit", &upstream])
        .current_dir(repository)
        .output()
        .map_err(|error| format!("cannot run git merge in {}: {error}", repository.display()))?;
    if merge.status.success() {
        success(&format!("{name} merged {upstream}"));
        return Ok(());
    }

    let conflicts = output(
        "git",
        &["diff", "--name-only", "--diff-filter=U"],
        repository,
    )?;
    if conflicts.trim().is_empty() {
        return Err(format!(
            "{name} merge failed ({}):\n{}{}",
            merge.status,
            String::from_utf8_lossy(&merge.stdout),
            String::from_utf8_lossy(&merge.stderr)
        ));
    }

    println!("\n=== {name} upstream merge conflicts ===");
    println!("Repository: {}", repository.display());
    println!("Merge: {} into {}", upstream, branch);
    println!("\nUnresolved paths ({}):", conflicts.lines().count());
    for path in conflicts.lines() {
        println!("  • {path}");
    }
    let diff = output(
        "git",
        &["diff", "--no-ext-diff", "--binary", "--cc"],
        repository,
    )?;
    println!("\n=== Conflict diff ===");
    if diff.trim().is_empty() {
        println!(
            "No textual conflict diff is available; inspect the listed paths and Git index stages."
        );
    } else {
        print!("{diff}");
    }
    println!("\n=== Git merge output ===");
    print!(
        "{}{}",
        String::from_utf8_lossy(&merge.stdout),
        String::from_utf8_lossy(&merge.stderr)
    );
    println!("\n=== Agent handoff ===");
    println!(
        "Resolve the in-progress {name} upstream merge in {}. Read AGENTS.md and Documentation/Upstream.md in the Photon root. Inspect every unresolved path above and preserve Photon-specific changes. Do not abort the merge or choose all of ours/theirs. Stage each resolution, run the relevant dependency checks, then complete the merge commit. Use `git -C {} diff --cc` to revisit the conflict hunks.",
        repository.display(),
        repository.display()
    );
    Err(format!(
        "{name} merge is paused with {} conflicts",
        conflicts.lines().count()
    ))
}
