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
mod migrations;
mod profile;
mod session_file;

pub use favicons::FaviconPixels;
pub use profile::{DataUsage, Profile, ProfileWriteTicket};

/// What Photon could recover from a saved profile file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileLoadOutcome<T> {
    /// No saved file exists yet. This is normal for a new profile.
    Missing,
    /// The complete file was read and decoded.
    Loaded(T),
    /// Some records were recovered; damaged records were skipped.
    PartiallyRecovered(T),
    /// The file uses a format version this build does not understand.
    Unsupported,
    /// The file could not be decoded or exceeded a safety limit.
    Corrupt,
}

impl<T: Default> ProfileLoadOutcome<T> {
    /// Returns recovered data, or an empty value when no data was recoverable.
    pub fn into_value(self) -> T {
        match self {
            Self::Loaded(value) | Self::PartiallyRecovered(value) => value,
            Self::Missing | Self::Unsupported | Self::Corrupt => T::default(),
        }
    }
}
