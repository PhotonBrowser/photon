use std::collections::HashSet;
use std::env;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::support::{command, compiler_version, output, require_tool, run_tool, version_at_least};

pub(crate) fn setup(root: &Path, verbose: bool) -> Result<(), String> {
    let config = SetupConfig::load(root)?;
    for submodule in &config.submodules {
        let path = submodule.path.as_str();
        let mut args = vec!["submodule", "update", "--init"];
        if submodule.recursive {
            args.push("--recursive");
        }
        args.extend(["--", path]);
        command("git", &args, root, verbose)?;
    }
    for tool in ["git", "cargo", "rustc", "cmake", "ninja", "python3"] {
        require_tool(tool)?;
    }
    if compiler_version(root).is_none() {
        return Err(
            "a C++ compiler is missing; install Clang or your platform's C++ build tools".into(),
        );
    }
    for (tool, minimum) in [("cmake", (3, 25)), ("ninja", (1, 10)), ("rustc", (1, 85))] {
        let version = output(tool, &["--version"], root)?;
        if !version_at_least(&version, minimum) {
            return Err(format!(
                "{tool} {} or newer is required; found {}",
                minimum.0,
                version.trim()
            ));
        }
    }
    require_tool("bun")?;
    if !root.join("Engine/CMakeLists.txt").is_file() {
        return Err(
            "Engine submodule is missing; run git submodule update --init --recursive".into(),
        );
    }
    for repository in &config.repositories {
        let path = root.join(&repository.path);
        if !path.join(".git").exists() {
            return Err(format!(
                "repository {} from photon.toml is missing or not initialized",
                repository.path
            ));
        }
        for remote in &repository.remotes {
            ensure_remote(&path, &remote.name, &remote.url)?;
        }
    }
    for path in ["build/engine-debug", "build/app-debug", "build/bin"] {
        std::fs::create_dir_all(root.join(path)).map_err(|e| e.to_string())?;
    }
    crate::commands::ide::setup(root, verbose)?;
    crate::commands::build::ensure_gpuix_js(root, verbose)?;
    crate::commands::build::ensure_ui_dependencies(root, verbose)?;
    println!(
        "Setup ready. The first engine build prepares Ladybird's pinned dependencies under build/."
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupConfig {
    submodules: Vec<SubmoduleConfig>,
    repositories: Vec<RepositoryConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmoduleConfig {
    path: String,
    #[serde(default)]
    recursive: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryConfig {
    path: String,
    remotes: Vec<RemoteConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteConfig {
    name: String,
    url: String,
}

impl SetupConfig {
    fn load(root: &Path) -> Result<Self, String> {
        let config_path = root.join("photon.toml");
        let source = std::fs::read_to_string(&config_path)
            .map_err(|error| format!("cannot read {}: {error}", config_path.display()))?;
        let config: Self =
            toml::from_str(&source).map_err(|error| format!("invalid photon.toml: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        let mut submodule_paths = HashSet::new();
        for submodule in &self.submodules {
            validate_relative_path(&submodule.path, false)?;
            if !submodule_paths.insert(&submodule.path) {
                return Err(format!(
                    "photon.toml lists submodule {} more than once",
                    submodule.path
                ));
            }
        }

        let mut repository_paths = HashSet::new();
        for repository in &self.repositories {
            validate_relative_path(&repository.path, true)?;
            if !repository_paths.insert(&repository.path) {
                return Err(format!(
                    "photon.toml configures repository {} more than once",
                    repository.path
                ));
            }
            let mut remote_names = HashSet::new();
            for remote in &repository.remotes {
                if remote.name.is_empty() || remote.name.starts_with('-') || remote.url.is_empty() {
                    return Err(format!(
                        "photon.toml has an empty remote name or URL for {}",
                        repository.path
                    ));
                }
                if !remote_names.insert(&remote.name) {
                    return Err(format!(
                        "photon.toml configures remote {} more than once for {}",
                        remote.name, repository.path
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_relative_path(path: &str, allow_repository_root: bool) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if path.is_empty()
        || path.contains('\\')
        || path_buf.is_absolute()
        || path_buf.components().any(|component| {
            !matches!(component, Component::Normal(_))
                && !(allow_repository_root && path == "." && matches!(component, Component::CurDir))
        })
    {
        return Err(format!(
            "photon.toml path must be a safe, repository-relative path: {path:?}"
        ));
    }
    Ok(())
}

fn ensure_remote(repository: &Path, name: &str, url: &str) -> Result<(), String> {
    let existing = output("git", &["remote", "get-url", name], repository);
    if existing.as_deref().unwrap_or_default().trim_end() == url {
        return Ok(());
    }

    let action = if existing.is_ok() { "set-url" } else { "add" };
    command("git", &["remote", action, name, url], repository, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_config_accepts_multiple_remotes_and_recursive_submodules() {
        let config: SetupConfig = toml::from_str(
            r#"
                [[submodules]]
                path = "vendor/gpuix"
                recursive = true

                [[repositories]]
                path = "vendor/gpuix/zed"
                [[repositories.remotes]]
                name = "origin"
                url = "https://example.com/fork.git"
                [[repositories.remotes]]
                name = "upstream"
                url = "https://example.com/upstream.git"
            "#,
        )
        .unwrap();

        config.validate().unwrap();
        assert!(config.submodules[0].recursive);
        assert_eq!(config.repositories[0].remotes.len(), 2);
    }

    #[test]
    fn setup_config_rejects_paths_that_escape_the_repository() {
        assert!(validate_relative_path("../outside", true).is_err());
        assert!(validate_relative_path("/outside", true).is_err());
        assert!(validate_relative_path("vendor/gpuix", false).is_ok());
        assert!(validate_relative_path(".", true).is_ok());
        assert!(validate_relative_path(".", false).is_err());
    }
}

pub(crate) fn doctor(root: &Path) -> Result<(), String> {
    println!("Photon environment");
    println!("  OS         {}/{}", env::consts::OS, env::consts::ARCH);
    for (name, args) in [
        ("Rust", vec!["rustc", "--version"]),
        ("Cargo", vec!["cargo", "--version"]),
        ("CMake", vec!["cmake", "--version"]),
        ("Ninja", vec!["ninja", "--version"]),
        ("Bun", vec!["bun", "--version"]),
    ] {
        let line = run_tool(args[0], &args[1..], root).unwrap_or_else(|_| "not found".into());
        println!("  {name:10} {}", line.lines().next().unwrap_or("unknown"));
    }
    println!(
        "  Compiler   {}",
        compiler_version(root).unwrap_or_else(|| "not found".into())
    );
    println!(
        "  Engine     {}",
        output(
            "git",
            &["rev-parse", "--short", "HEAD"],
            &root.join("Engine")
        )
        .unwrap_or_else(|_| "missing".into())
        .trim()
    );
    println!(
        "  branch     {}",
        output("git", &["branch", "--show-current"], &root.join("Engine"))
            .unwrap_or_else(|_| "unavailable".into())
            .trim()
    );
    println!(
        "  submodule  {}",
        output("git", &["submodule", "status", "Engine"], root)
            .unwrap_or_else(|_| "unavailable".into())
            .trim()
    );
    for (name, args) in [
        ("origin", vec!["remote", "get-url", "origin"]),
        ("upstream", vec!["remote", "get-url", "upstream"]),
    ] {
        println!(
            "  {name:10} {}",
            output("git", &args, &root.join("Engine"))
                .unwrap_or_else(|_| "missing".into())
                .trim()
        );
    }
    println!(
        "  working tree {}",
        if output("git", &["status", "--porcelain"], &root.join("Engine"))
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            "clean"
        } else {
            "dirty"
        }
    );
    println!(
        "  build dir  {}",
        if root.join("build").exists() {
            "present"
        } else {
            "absent"
        }
    );
    for path in ["engine-debug", "app-debug", "vcpkg-debug"] {
        println!(
            "  {path:10} {}",
            if root.join("build").join(path).exists() {
                "present"
            } else {
                "not configured"
            }
        );
    }
    crate::commands::ide::doctor(root);
    Ok(())
}
