use std::path::Path;

use clap::ValueEnum;

use crate::support::{output, success};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Dependency {
    Engine,
    Gpui,
    Motion,
}

const DEPENDENCIES: [(Dependency, &str, &str, &str, &str); 3] = [
    (
        Dependency::Engine,
        "Engine",
        "Engine",
        "master",
        "upstream/master",
    ),
    (
        Dependency::Gpui,
        "GPUI-CE",
        "vendor/gpui-ce",
        "main",
        "upstream/main",
    ),
    (
        Dependency::Motion,
        "gpui-motion",
        "crates/gpui-motion",
        "main",
        "upstream/main",
    ),
];

pub(crate) fn status(root: &Path) -> Result<(), String> {
    println!("Photon dependencies");
    for (_, name, relative, expected_branch, upstream) in DEPENDENCIES {
        let repository = root.join(relative);
        if !repository.join(".git").exists() {
            println!("  {name:10} not initialized  (run `photon setup`)");
            continue;
        }

        let current_branch = output("git", &["branch", "--show-current"], &repository)?;
        let head = output("git", &["rev-parse", "HEAD"], &repository)?;
        let pinned = output("git", &["rev-parse", &format!("HEAD:{relative}")], root)?;
        let worktree = output(
            "git",
            &["status", "--porcelain", "--ignore-submodules=all"],
            &repository,
        )?;
        let pin_state = if head.trim() == pinned.trim() {
            "pinned"
        } else {
            "unpinned"
        };
        let worktree_state = if worktree.trim().is_empty() {
            "clean"
        } else {
            "dirty"
        };
        let branch = current_branch.trim();
        let branch_state = if branch.is_empty() {
            "detached".to_owned()
        } else {
            branch.to_owned()
        };
        let upstream_state = upstream_counts(&repository, upstream)
            .unwrap_or_else(|| "upstream unavailable".to_owned());

        println!(
            "  {name:10} {branch_state:10} {pin_state:8} {worktree_state:5}  {upstream_state}"
        );
        if branch != expected_branch {
            println!("             expected Photon branch: {expected_branch}");
        }
    }
    Ok(())
}

fn upstream_counts(repository: &Path, upstream: &str) -> Option<String> {
    let counts = output(
        "git",
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("HEAD...{upstream}"),
        ],
        repository,
    )
    .ok()?;
    let (ahead, behind) = counts.trim().split_once(char::is_whitespace)?;
    Some(format!("{behind} behind / {ahead} ahead"))
}

pub(crate) fn edit(
    root: &Path,
    dependency: Option<Dependency>,
    verbose: bool,
) -> Result<(), String> {
    for item in selected(dependency) {
        match item {
            Dependency::Engine => crate::commands::engine::edit(root, verbose)?,
            Dependency::Gpui => crate::commands::gpui::edit(root, verbose)?,
            Dependency::Motion => crate::commands::motion::edit(root, verbose)?,
        }
    }
    Ok(())
}

pub(crate) fn restore(
    root: &Path,
    dependency: Option<Dependency>,
    verbose: bool,
) -> Result<(), String> {
    for item in selected(dependency) {
        match item {
            Dependency::Engine => crate::commands::engine::pin(root, verbose)?,
            Dependency::Gpui => crate::commands::gpui::pin(root, verbose)?,
            Dependency::Motion => crate::commands::motion::pin(root, verbose)?,
        }
    }
    Ok(())
}

pub(crate) fn sync(
    root: &Path,
    dependency: Option<Dependency>,
    check: bool,
    no_review: bool,
    verbose: bool,
) -> Result<(), String> {
    if !no_review && (check || dependency.is_some()) {
        return Err("`--review` applies only to a completed sync of all dependencies".into());
    }
    if dependency.is_none() && !check && !no_review {
        crate::commands::sync::require_review_dispatch_auth(root)?;
    }
    let before = if dependency.is_none() && !check && !no_review {
        Some(crate::commands::sync::capture_sync_heads(root)?)
    } else {
        None
    };

    for item in selected(dependency) {
        match (item, check) {
            (Dependency::Engine, true) => crate::commands::engine::check_upstream(root, verbose)?,
            (Dependency::Gpui, true) => crate::commands::gpui::check_upstream(root, verbose)?,
            (Dependency::Motion, true) => crate::commands::motion::check_upstream(root, verbose)?,
            (Dependency::Engine, false) => {
                crate::commands::engine::engine(root, Some("sync"), false, verbose)?
            }
            (Dependency::Gpui, false) => crate::commands::gpui::sync(root, verbose)?,
            (Dependency::Motion, false) => crate::commands::motion::sync(root, verbose)?,
        }
    }

    if let Some(before) = before {
        let after = crate::commands::sync::capture_sync_heads(root)?;
        crate::commands::sync::dispatch_review(root, &before, &after, verbose)?;
    } else if dependency.is_none() && !check {
        success("All dependency syncs completed; GitHub review dispatch skipped");
    }
    Ok(())
}

fn selected(dependency: Option<Dependency>) -> impl Iterator<Item = Dependency> {
    match dependency {
        Some(dependency) => vec![dependency],
        None => vec![Dependency::Engine, Dependency::Gpui, Dependency::Motion],
    }
    .into_iter()
}
