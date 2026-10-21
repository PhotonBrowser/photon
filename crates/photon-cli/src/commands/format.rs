use std::path::Path;

use crate::support::{CLANG_FORMAT_VERSION, clang_format, invoke, output, path, stage, success};

pub(crate) fn format(root: &Path, verbose: bool) -> Result<(), String> {
    stage("Format Photon Rust sources");
    invoke("cargo", &["fmt", "--all"], root, verbose)?;
    let sources = native_sources(root)?;
    format_native_sources(
        root,
        "Photon native sources",
        &sources,
        CLANG_FORMAT_VERSION,
        verbose,
    )?;

    let engine_sources = engine_embedder_sources(root)?;
    format_native_sources(
        root,
        "Photon Engine embedder sources",
        &engine_sources,
        CLANG_FORMAT_VERSION,
        verbose,
    )?;

    success("Formatting complete");
    Ok(())
}

fn format_native_sources(
    root: &Path,
    label: &str,
    sources: &[String],
    version: &str,
    verbose: bool,
) -> Result<(), String> {
    if sources.is_empty() {
        return Ok(());
    }
    let clang_format = clang_format(root, version)?;
    stage(&format!("Format {label} with clang-format {version}"));
    let mut args = vec!["--style=file", "-i"];
    args.extend(sources.iter().map(String::as_str));
    invoke(&path(&clang_format), &args, root, verbose)
}

/// Photon's own C, C++ and Objective-C files, including new ones not yet
/// added. Submodules and ignored build output are not Photon's to format.
pub(super) fn native_sources(root: &Path) -> Result<Vec<String>, String> {
    let listing = output(
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.c",
            "*.cc",
            "*.cpp",
            "*.h",
            "*.hpp",
            "*.m",
            "*.mm",
        ],
        root,
    )?;
    Ok(listing
        .lines()
        .filter(|path| root.join(path).is_file())
        .map(str::to_owned)
        .collect())
}

/// Photon owns the embedder library in the Engine submodule. Keep its files on
/// the Engine clang-format version while leaving upstream Engine sources alone.
pub(super) fn engine_embedder_sources(root: &Path) -> Result<Vec<String>, String> {
    let engine_root = root.join("Engine");
    if !engine_root.join("CMakeLists.txt").is_file() {
        return Ok(Vec::new());
    }

    let listing = output(
        "git",
        &[
            "-C",
            &path(&engine_root),
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "Libraries/LibPhotonEmbedder",
        ],
        root,
    )?;
    Ok(listing
        .lines()
        .filter(|relative| {
            matches!(
                Path::new(relative).extension().and_then(|ext| ext.to_str()),
                Some("cpp" | "h" | "mm")
            ) && engine_root.join(relative).is_file()
        })
        .map(|relative| path(&root.join("Engine").join(relative)))
        .collect())
}
