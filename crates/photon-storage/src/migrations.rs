//! Ordered, one-time migrations for files stored in a profile.

use crate::{atomic, profile::Profile};
use photon_core::{BrowserSettings, WorkspaceSettings};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const VERSION_FILE: &str = ".photon-profile-version";
const LEGACY_SETTINGS_FILE: &str = "config.json";
const OLDER_SETTINGS_FILE: &str = "Settings.json";

// Keep completed migrations in order. Add new migrations at the end.
const MIGRATIONS: &[fn(&Profile) -> io::Result<()>] = &[migrate_legacy_settings];

pub(super) fn run(profile: &Profile) -> io::Result<()> {
    let version_path = profile.root().join(VERSION_FILE);
    let mut version = read_version(&version_path)?;
    if version as usize > MIGRATIONS.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "profile was written by a newer version of Photon",
        ));
    }

    while (version as usize) < MIGRATIONS.len() {
        MIGRATIONS[version as usize](profile)?;
        version += 1;
        atomic::write(&version_path, format!("{version}\n").as_bytes())?;
    }
    Ok(())
}

fn read_version(path: &std::path::Path) -> io::Result<u32> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    source.trim().parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid profile migration version: {error}"),
        )
    })
}

fn migrate_legacy_settings(profile: &Profile) -> io::Result<()> {
    let candidates = [
        profile.settings_path().with_file_name(LEGACY_SETTINGS_FILE),
        profile.root().join(LEGACY_SETTINGS_FILE),
        profile.root().join(OLDER_SETTINGS_FILE),
    ];
    let mut legacy_paths = Vec::with_capacity(candidates.len());
    for path in candidates {
        if !legacy_paths.contains(&path) {
            legacy_paths.push(path);
        }
    }

    if !profile.settings_path().exists()
        && let Some(legacy_path) = legacy_paths.iter().find(|path| path.is_file())
    {
        convert_legacy_settings(profile, legacy_path)?;
    }

    for legacy_path in legacy_paths {
        archive_if_present(&legacy_path)?;
    }
    Ok(())
}

fn convert_legacy_settings(profile: &Profile, path: &Path) -> io::Result<()> {
    let bytes = fs::read(path)?;
    let settings = match decode_json_settings(&bytes) {
        Some((settings, partially_unreadable)) => {
            if partially_unreadable {
                profile.keep_unreadable_settings(&bytes);
            }
            settings
        }
        None => {
            profile.keep_unreadable_settings(&bytes);
            BrowserSettings::default()
        }
    };
    profile.save_settings(&settings)
}

fn decode_json_settings(bytes: &[u8]) -> Option<(BrowserSettings, bool)> {
    let mut document = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let saved_workspace = document
        .as_object_mut()
        .and_then(|settings| settings.remove("workspace"))
        .filter(|workspace| {
            workspace.as_object().is_some_and(|workspace| {
                workspace.contains_key("default_preset_id") && workspace.contains_key("presets")
            })
        });
    let parsed = serde_json::from_value::<BrowserSettings>(document.clone());
    let partially_unreadable = parsed.is_err();
    let mut settings = parsed.unwrap_or_else(|_| readable_json_settings(document));
    settings.workspace = saved_workspace
        .and_then(|workspace| serde_json::from_value::<WorkspaceSettings>(workspace).ok())
        .filter(WorkspaceSettings::is_valid)
        .unwrap_or_else(|| WorkspaceSettings::for_tab_layout(settings.tab_layout));
    settings.spaces.validate_layout_overrides();
    Some((settings, partially_unreadable))
}

/// Reads older JSON settings, keeping each field this build understands.
fn readable_json_settings(document: serde_json::Value) -> BrowserSettings {
    let serde_json::Value::Object(fields) = document else {
        return BrowserSettings::default();
    };
    let readable = fields
        .into_iter()
        .filter(|(key, value)| {
            let single = serde_json::Map::from_iter([(key.clone(), value.clone())]);
            serde_json::from_value::<BrowserSettings>(serde_json::Value::Object(single)).is_ok()
        })
        .collect();
    serde_json::from_value(serde_json::Value::Object(readable)).unwrap_or_default()
}

fn archive_if_present(path: &Path) -> io::Result<()> {
    match fs::rename(path, migrated_path(path)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn migrated_path(path: &Path) -> PathBuf {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let base = format!("{name}.migrated-{seconds}-{}", std::process::id());
    let mut backup = path.with_file_name(&base);
    let mut suffix = 1;
    while backup.exists() {
        backup = path.with_file_name(format!("{base}-{suffix}"));
        suffix += 1;
    }
    backup
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_profiles_from_a_newer_version() {
        let root = std::env::temp_dir().join(format!(
            "photon-storage-newer-profile-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let profile = Profile::open(&root).expect("opens profile");
        atomic::write(&root.join(VERSION_FILE), b"999\n").expect("writes version");

        let error = run(&profile).expect_err("rejects newer profile version");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(root).expect("removes test profile");
    }
}
