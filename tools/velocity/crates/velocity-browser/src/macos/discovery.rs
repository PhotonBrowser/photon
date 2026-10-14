use std::{
    collections::BTreeSet,
    env, fmt, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    BrowserEngine, BrowserId, BrowserInstallation, BrowserMetadata, BrowserRegistry,
    InstallationId, RegistryError,
};

const BROWSER_BUNDLES: &[(&str, &str, BrowserEngine)] = &[
    ("safari", "com.apple.Safari", BrowserEngine::WebKit),
    ("chrome", "com.google.Chrome", BrowserEngine::Chromium),
    ("helium", "net.imput.helium", BrowserEngine::Chromium),
    ("firefox", "org.mozilla.firefox", BrowserEngine::Gecko),
    ("brave", "com.brave.Browser", BrowserEngine::Chromium),
    ("arc", "company.thebrowser.Browser", BrowserEngine::Chromium),
    (
        "photon",
        "org.photonbrowser.photon.development",
        BrowserEngine::Ladybird,
    ),
];

const STANDARD_APPLICATION_DIRECTORIES: &[&str] = &["/Applications", "/System/Applications"];

/// Discovers supported macOS browser applications and Photon checkout builds.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacOSBrowserDiscovery;

impl MacOSBrowserDiscovery {
    pub fn discover(&self) -> Result<BrowserRegistry, BrowserDiscoveryError> {
        let mut candidates = BTreeSet::new();
        self.find_with_spotlight(&mut candidates);
        self.find_in_standard_directories(&mut candidates);
        self.find_photon_development_bundle(&mut candidates);

        let mut registry = BrowserRegistry::default();
        for candidate in candidates {
            if let Some(installation) = read_application_bundle(&candidate) {
                registry
                    .insert(installation)
                    .map_err(BrowserDiscoveryError::Registry)?;
            }
        }

        for root in photon_checkout_roots() {
            for (profile, suffix) in [("debug", "debug"), ("release", "release")] {
                let executable = root.join("target").join(profile).join("Photon");
                if !executable.is_file() {
                    continue;
                }
                let root = root.canonicalize().unwrap_or_else(|_| root.clone());
                let executable = executable.canonicalize().unwrap_or(executable);
                let installation_id = InstallationId::new(format!(
                    "photon-repo-{suffix}-{:016x}",
                    stable_path_hash(&root)
                ))
                .expect("generated Photon installation identifier is valid");
                registry
                    .insert(BrowserInstallation {
                        installation_id,
                        metadata: BrowserMetadata {
                            id: known_browser_id("photon"),
                            name: format!("Photon Development ({suffix})"),
                            bundle_identifier: None,
                            engine: Some(BrowserEngine::Ladybird),
                        },
                        version: None,
                        application_path: root,
                        executable_path: executable,
                    })
                    .map_err(BrowserDiscoveryError::Registry)?;
            }
        }

        Ok(registry)
    }

    fn find_with_spotlight(&self, candidates: &mut BTreeSet<PathBuf>) {
        for (_, bundle_identifier, _) in BROWSER_BUNDLES {
            let query = format!("kMDItemCFBundleIdentifier == '{bundle_identifier}'");
            let Ok(output) = Command::new("/usr/bin/mdfind").arg(query).output() else {
                continue;
            };
            if !output.status.success() {
                continue;
            }
            for path in String::from_utf8_lossy(&output.stdout).lines() {
                let path = PathBuf::from(path.trim());
                if path.extension().is_some_and(|extension| extension == "app") {
                    candidates.insert(path);
                }
            }
        }
    }

    fn find_in_standard_directories(&self, candidates: &mut BTreeSet<PathBuf>) {
        let mut directories = STANDARD_APPLICATION_DIRECTORIES
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if let Some(home) = env::var_os("HOME") {
            directories.push(PathBuf::from(home).join("Applications"));
        }
        for directory in directories {
            collect_application_bundles(&directory, 0, candidates);
        }
    }

    fn find_photon_development_bundle(&self, candidates: &mut BTreeSet<PathBuf>) {
        let Some(home) = env::var_os("HOME") else {
            return;
        };
        candidates.insert(
            PathBuf::from(home).join("Library/Application Support/Photon/Development/Photon.app"),
        );
    }
}

fn collect_application_bundles(directory: &Path, depth: usize, candidates: &mut BTreeSet<PathBuf>) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "app") {
            candidates.insert(path);
        } else if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_application_bundles(&path, depth + 1, candidates);
        }
    }
}

fn read_application_bundle(path: &Path) -> Option<BrowserInstallation> {
    let application_path = path.canonicalize().ok()?;
    let info_plist = application_path.join("Contents/Info.plist");
    let bundle_identifier = read_plist_string(&info_plist, "CFBundleIdentifier")?;
    let (browser_id, _, engine) = BROWSER_BUNDLES
        .iter()
        .find(|(_, identifier, _)| *identifier == bundle_identifier)?;
    let executable_name = read_plist_string(&info_plist, "CFBundleExecutable")?;
    if executable_name.contains('/') || executable_name.contains('\\') {
        return None;
    }
    let executable_path = application_path
        .join("Contents/MacOS")
        .join(executable_name)
        .canonicalize()
        .ok()?;
    if !is_executable_file(&executable_path) {
        return None;
    }

    let display_name = read_plist_string(&info_plist, "CFBundleDisplayName")
        .or_else(|| read_plist_string(&info_plist, "CFBundleName"))
        .or_else(|| {
            application_path
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })?;
    let version = read_plist_string(&info_plist, "CFBundleShortVersionString")
        .or_else(|| read_plist_string(&info_plist, "CFBundleVersion"));
    let installation_id = installation_identifier(browser_id, &application_path);

    Some(BrowserInstallation {
        installation_id,
        metadata: BrowserMetadata {
            id: known_browser_id(browser_id),
            name: display_name,
            bundle_identifier: Some(bundle_identifier),
            engine: Some(*engine),
        },
        version,
        application_path,
        executable_path,
    })
}

fn read_plist_string(info_plist: &Path, key: &str) -> Option<String> {
    let command = format!("Print :{key}");
    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(command)
        .arg(info_plist)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn installation_identifier(browser_id: &str, application_path: &Path) -> InstallationId {
    let flavor = if browser_id == "photon" {
        "photon-development".to_owned()
    } else {
        browser_id.to_owned()
    };
    InstallationId::new(format!(
        "{flavor}-{:016x}",
        stable_path_hash(application_path)
    ))
    .expect("generated browser installation identifier is valid")
}

fn known_browser_id(value: &str) -> BrowserId {
    BrowserId::new(value).expect("supported browser identifiers are valid")
}

fn stable_path_hash(path: &Path) -> u64 {
    path.as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn photon_checkout_roots() -> Vec<PathBuf> {
    let mut ancestors = Vec::new();
    if let Ok(directory) = env::current_dir() {
        ancestors.extend(directory.ancestors().map(Path::to_path_buf));
    }
    if let Some(directory) = env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    {
        ancestors.extend(directory.ancestors().map(Path::to_path_buf));
    }
    ancestors
        .into_iter()
        .filter(|path| {
            path.join("photon").is_file() && path.join("crates/photon-app/Cargo.toml").is_file()
        })
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserDiscoveryError {
    Registry(RegistryError),
}

impl fmt::Display for BrowserDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BrowserDiscoveryError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fixture_application_bundle(bundle_identifier: &str, display_name: &str) -> PathBuf {
        let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let application = std::env::temp_dir().join(format!(
            "Velocity Fixture {} {sequence}.app",
            std::process::id()
        ));
        let contents = application.join("Contents");
        let executable_directory = contents.join("MacOS");
        fs::create_dir_all(&executable_directory).unwrap();
        fs::write(
            contents.join("Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{bundle_identifier}</string>
<key>CFBundleDisplayName</key><string>{display_name}</string>
<key>CFBundleExecutable</key><string>Browser</string>
<key>CFBundleShortVersionString</key><string>999.0</string>
</dict></plist>"#
            ),
        )
        .unwrap();
        let executable = executable_directory.join("Browser");
        fs::write(&executable, b"fixture binary").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        application
    }

    #[test]
    fn reads_fixture_bundle_metadata_without_installed_browsers() {
        let application = fixture_application_bundle("com.google.Chrome", "Fixture Chrome");

        let installation = read_application_bundle(&application).unwrap();
        assert_eq!(installation.metadata.id.as_str(), "chrome");
        assert_eq!(installation.metadata.name, "Fixture Chrome");
        assert_eq!(installation.version.as_deref(), Some("999.0"));
        assert_eq!(
            installation.metadata.bundle_identifier.as_deref(),
            Some("com.google.Chrome")
        );
        assert_eq!(installation.metadata.engine, Some(BrowserEngine::Chromium));
        assert!(
            installation
                .executable_path
                .ends_with("Contents/MacOS/Browser")
        );

        fs::remove_dir_all(application).unwrap();
    }

    #[test]
    fn recognizes_helium_bundle_metadata() {
        let application = fixture_application_bundle("net.imput.helium", "Helium");

        let installation = read_application_bundle(&application).unwrap();
        assert_eq!(installation.metadata.id.as_str(), "helium");
        assert_eq!(installation.metadata.name, "Helium");
        assert_eq!(
            installation.metadata.bundle_identifier.as_deref(),
            Some("net.imput.helium")
        );
        assert_eq!(installation.metadata.engine, Some(BrowserEngine::Chromium));

        fs::remove_dir_all(application).unwrap();
    }
}
