//! The browser's name and marks, in one place so it can be renamed.
//!
//! Everything people see that names the browser — window titles, notices,
//! internal page addresses, the profile folder and the logo — comes from
//! here. Change these to rebrand.

/// The browser's name, as shown to people.
pub const NAME: &str = "Photon";

/// The name of the web engine, as shown in notices about it.
pub const ENGINE_NAME: &str = "Photon Engine";

/// The scheme of the browser's own pages, as `photon` in `photon://settings`.
pub const PAGE_SCHEME: &str = "photon";

/// The macOS bundle identifier of the app `photon run` builds for
/// development, so macOS can open web links with it.
pub const DEVELOPMENT_BUNDLE_IDENTIFIER: &str = "org.photonbrowser.photon.development";

/// The folder in the platform's application data folder that holds profiles.
pub const DATA_FOLDER: &str = NAME;

/// The macOS app icon's Icon Composer document, which holds its artwork
/// and how it looks in each appearance: default, dark, clear and tinted.
pub const APP_ICON_DOCUMENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/AppIcon.icon");

/// The macOS app icon as a 1024-pixel PNG on Apple's icon grid, the shape
/// 824 pixels across on a transparent canvas, for building without Xcode,
/// which compiles the document.
pub const APP_ICON_PNG: &[u8] = include_bytes!("../assets/app-icon.png");

/// The logo, as SVG in its own colors.
pub const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");
