//! Photon's on-disk profile: what the browser remembers between launches.
//!
//! A [`Profile`] is one folder. It holds Photon's history and past searches,
//! the icons of visited pages, and the Engine's own website data (cookies,
//! site storage and the cache) in [`Profile::engine_dir`]. Everything here is
//! plain file access with no UI or Engine dependency, so settings pages and
//! internal tabs can list and delete it through the same calls.

mod atomic;
mod favicons;
mod history_file;
mod profile;

pub use favicons::FaviconPixels;
pub use profile::{DataUsage, Profile};
