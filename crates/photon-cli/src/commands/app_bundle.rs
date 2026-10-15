//! The development app bundle `photon run` launches. macOS registers it as
//! able to open web links and HTML files, so it can be the default browser,
//! and launches it with what it needs when a link opens it.
//!
//! It lives beside the browser's profile rather than in the checkout:
//! macOS keeps the Dock from reading inside folders such as Documents, where
//! a checkout often is, so the Dock could not show its icon there.

use std::path::{Path, PathBuf};
use std::process::Command;

use photon_brand::{
    APP_ICON_DOCUMENT, APP_ICON_PNG, DATA_FOLDER, DEVELOPMENT_BUNDLE_IDENTIFIER, NAME,
};

use super::presentation_broker::xml_escape;
use crate::support::stage;

/// The oldest macOS the development app opens on.
const MINIMUM_SYSTEM_VERSION: &str = "13.0";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// Where the development app lives:
/// `~/Library/Application Support/<data folder>/Development/<name>.app`.
fn bundle_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support")
        .join(DATA_FOLDER)
        .join("Development")
        .join(format!("{NAME}.app")))
}

/// Puts the built browser in the development app with its Info.plist, has
/// macOS register it, and returns the executable to launch. `environment`
/// is set when macOS launches it, as for a link opened in another app.
pub(super) fn ensure(
    root: &Path,
    built: &Path,
    environment: &[(&str, String)],
) -> Result<PathBuf, String> {
    stage("Bundle Photon development app");
    let bundle = bundle_path()?;
    let executable = write_bundle(root, built, &bundle, info_plist(environment))?;
    register(&bundle);
    Ok(executable)
}

/// Puts the UI-only shell in an app bundle with Photon metadata and icon.
/// Its separate bundle identity and location keep it distinct from the
/// browser bundle, and it is deliberately not registered as a link handler.
pub(super) fn ensure_ui(root: &Path, built: &Path) -> Result<PathBuf, String> {
    stage("Bundle Photon UI-only shell");
    let bundle = bundle_path()?
        .with_file_name("UI-only")
        .join(format!("{NAME}.app"));
    write_bundle(root, built, &bundle, ui_info_plist())
}

fn write_bundle(
    root: &Path,
    built: &Path,
    bundle: &Path,
    plist: String,
) -> Result<PathBuf, String> {
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos).map_err(|error| error.to_string())?;
    let executable = macos.join(NAME);
    // Replace the executable by renaming a copy over it, so an instance
    // still running keeps the file it started from.
    let staged = macos.join(format!(".{NAME}.new"));
    std::fs::copy(built, &staged)
        .and_then(|_| std::fs::rename(&staged, &executable))
        .map_err(|error| {
            format!(
                "could not copy {} into {}: {error}",
                built.display(),
                bundle.display()
            )
        })?;
    std::fs::write(contents.join("Info.plist"), plist).map_err(|error| error.to_string())?;
    write_icon(root, &contents.join("Resources"))?;
    Ok(executable)
}

/// The icon's sizes in an `.iconset`, each also drawn at twice the size.
const ICON_SIZES: [u32; 5] = [16, 32, 128, 256, 512];

/// Writes the app icon into the bundle's resources. With Xcode, its asset
/// compiler turns the Icon Composer document into `Assets.car`, with every
/// appearance, and a `<name>.icns` for older macOS; without it, `sips` and
/// `iconutil` make the `.icns` from the PNG master. The icon is made again
/// only when its sources change.
fn write_icon(root: &Path, resources: &Path) -> Result<(), String> {
    let document = Path::new(APP_ICON_DOCUMENT);
    let mut sources = APP_ICON_PNG.to_vec();
    fingerprint(document, document, &mut sources)?;
    let work = root.join("build/app-icon");
    let stamp = work.join("sources");
    let icns = resources.join(format!("{NAME}.icns"));
    if std::fs::read(&stamp).is_ok_and(|saved| saved == sources) && icns.is_file() {
        return Ok(());
    }
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(resources).map_err(|error| error.to_string())?;
    let compiled = has_asset_compiler();
    if compiled {
        compile_icon_document(document, &work, resources)?;
    } else {
        eprintln!(
            "warning: Xcode's actool is missing; the app icon has no dark, clear or tinted appearance"
        );
        let catalog = resources.join("Assets.car");
        if catalog.exists() {
            std::fs::remove_file(catalog).map_err(|error| error.to_string())?;
        }
        icns_from_master(&work, &icns)?;
    }
    std::fs::write(stamp, sources).map_err(|error| error.to_string())
}

/// Adds every file under `directory`, with its path from `base`, to
/// `sources`, so a change to any of them changes the result.
fn fingerprint(base: &Path, directory: &Path, sources: &mut Vec<u8>) -> Result<(), String> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            fingerprint(base, &path, sources)?;
        } else {
            let relative = path.strip_prefix(base).unwrap_or(&path);
            sources.extend(relative.to_string_lossy().as_bytes());
            sources.extend(std::fs::read(&path).map_err(|error| error.to_string())?);
        }
    }
    Ok(())
}

fn has_asset_compiler() -> bool {
    Command::new("xcrun")
        .args(["--find", "actool"])
        .output()
        .is_ok_and(|result| result.status.success())
}

/// Compiles the Icon Composer document, named for the app so its icon is
/// found by that name, into `resources`.
fn compile_icon_document(document: &Path, work: &Path, resources: &Path) -> Result<(), String> {
    let named = work.join(format!("{NAME}.icon"));
    copy_directory(document, &named)?;
    let partial_plist = work.join("icon.plist");
    run(
        "xcrun",
        &[
            "actool".as_ref(),
            named.as_os_str(),
            "--compile".as_ref(),
            resources.as_os_str(),
            "--platform".as_ref(),
            "macosx".as_ref(),
            "--minimum-deployment-target".as_ref(),
            MINIMUM_SYSTEM_VERSION.as_ref(),
            "--app-icon".as_ref(),
            NAME.as_ref(),
            "--output-partial-info-plist".as_ref(),
            partial_plist.as_os_str(),
        ],
    )
}

fn copy_directory(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let target = to.join(path.file_name().unwrap_or_default());
        if path.is_dir() {
            copy_directory(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Makes `icns` from the PNG master with `sips` and `iconutil`.
fn icns_from_master(work: &Path, icns: &Path) -> Result<(), String> {
    let master = work.join("app-icon.png");
    let iconset = work.join(format!("{NAME}.iconset"));
    std::fs::create_dir_all(&iconset).map_err(|error| error.to_string())?;
    std::fs::write(&master, APP_ICON_PNG).map_err(|error| error.to_string())?;
    for size in ICON_SIZES {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let pixels = (size * scale).to_string();
            let output = iconset.join(format!("icon_{size}x{size}{suffix}.png"));
            run(
                "sips",
                &[
                    "-z".as_ref(),
                    pixels.as_ref(),
                    pixels.as_ref(),
                    master.as_os_str(),
                    "--out".as_ref(),
                    output.as_os_str(),
                ],
            )?;
        }
    }
    run(
        "iconutil",
        &[
            "-c".as_ref(),
            "icns".as_ref(),
            iconset.as_os_str(),
            "-o".as_ref(),
            icns.as_os_str(),
        ],
    )
}

fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let result = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ))
    }
}

/// Tells Launch Services about the bundle so macOS can offer it as a
/// browser. Without it the browser still runs, so a failure only warns.
fn register(bundle: &Path) {
    let registered = Command::new(LSREGISTER)
        .arg("-f")
        .arg(bundle)
        .status()
        .is_ok_and(|status| status.success());
    if !registered {
        eprintln!(
            "warning: macOS did not register {}; it cannot be the default browser",
            bundle.display()
        );
    }
}

fn info_plist(environment: &[(&str, String)]) -> String {
    info_plist_for(environment, DEVELOPMENT_BUNDLE_IDENTIFIER, true)
}

fn ui_info_plist() -> String {
    info_plist_for(
        &[],
        &format!("{DEVELOPMENT_BUNDLE_IDENTIFIER}.ui-only"),
        false,
    )
}

fn info_plist_for(
    environment: &[(&str, String)],
    bundle_identifier: &str,
    handles_browser_documents: bool,
) -> String {
    let string = |value: &str| format!("<string>{}</string>", xml_escape(value));
    let environment = environment
        .iter()
        .map(|(key, value)| format!("<key>{}</key>{}", xml_escape(key), string(value)))
        .collect::<String>();
    let browser_document_handlers = if handles_browser_documents {
        r#"<key>CFBundleURLTypes</key><array><dict>
<key>CFBundleURLName</key><string>Web site URL</string>
<key>CFBundleTypeRole</key><string>Viewer</string>
<key>CFBundleURLSchemes</key><array><string>http</string><string>https</string></array>
</dict></array>
<key>CFBundleDocumentTypes</key><array><dict>
<key>CFBundleTypeName</key><string>HTML document</string>
<key>CFBundleTypeRole</key><string>Viewer</string>
<key>LSHandlerRank</key><string>Alternate</string>
<key>LSItemContentTypes</key><array><string>public.html</string><string>public.xhtml</string></array>
</dict></array>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key>{identifier}
<key>CFBundleName</key>{name}
<key>CFBundleDisplayName</key>{name}
<key>CFBundleExecutable</key>{name}
<key>CFBundleIconFile</key>{name}
<key>CFBundleIconName</key>{name}
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key>{version}
<key>CFBundleVersion</key>{version}
<key>LSMinimumSystemVersion</key>{minimum}
<key>NSHighResolutionCapable</key><true/>
{browser_document_handlers}
<key>LSEnvironment</key><dict>{environment}</dict>
</dict></plist>
"#,
        identifier = string(bundle_identifier),
        name = string(NAME),
        version = string(env!("CARGO_PKG_VERSION")),
        minimum = string(MINIMUM_SYSTEM_VERSION),
        browser_document_handlers = browser_document_handlers,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_web_links_and_launch_environment() {
        let plist = info_plist(&[("PHOTON_HELPER_DIRECTORY", "/a & b".to_owned())]);
        assert!(plist.contains("<string>https</string>"));
        assert!(plist.contains("<string>public.html</string>"));
        assert!(plist.contains("<key>PHOTON_HELPER_DIRECTORY</key><string>/a &amp; b</string>"));
    }

    #[test]
    fn ui_bundle_has_metadata_without_browser_handlers() {
        let plist = ui_info_plist();
        assert!(plist.contains("<string>org.photonbrowser.photon.development.ui-only</string>"));
        assert!(plist.contains("<key>CFBundleIconFile</key>"));
        assert!(!plist.contains("CFBundleURLTypes"));
        assert!(!plist.contains("CFBundleDocumentTypes"));
    }
}
