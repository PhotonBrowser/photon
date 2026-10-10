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

/// The folder in the platform's application data folder that holds profiles.
pub const DATA_FOLDER: &str = NAME;

/// The logo, as SVG in its own colors.
pub const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");
