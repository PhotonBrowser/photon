//! A profile folder and what is kept in it.

use photon_core::{ClearBrowsingData, History};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::favicons::{self, FaviconPixels};
use super::{atomic, history_file};

const HISTORY_FILE: &str = "History.json";
const FAVICONS_DIR: &str = "Favicons";
const ENGINE_DIR: &str = "Engine";

/// How much disk a profile's data takes, in bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DataUsage {
    pub history: u64,
    pub favicons: u64,
    /// The Engine's cookies, site storage and cache.
    pub website_data: u64,
}

impl DataUsage {
    pub fn total(&self) -> u64 {
        self.history + self.favicons + self.website_data
    }
}

/// One profile folder.
#[derive(Clone, Debug)]
pub struct Profile {
    root: PathBuf,
}

impl Profile {
    /// Opens the profile at `root`, creating its folders as needed.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let profile = Self { root: root.into() };
        fs::create_dir_all(profile.favicons_dir())?;
        fs::create_dir_all(profile.engine_dir())?;
        Ok(profile)
    }

    /// Opens the default profile: `PHOTON_PROFILE_DIR` when set, otherwise
    /// the platform's application data folder.
    pub fn open_default() -> io::Result<Self> {
        Self::open(Self::default_root()?)
    }

    /// Where the default profile lives.
    pub fn default_root() -> io::Result<PathBuf> {
        if let Some(root) = std::env::var_os("PHOTON_PROFILE_DIR").filter(|root| !root.is_empty()) {
            return Ok(PathBuf::from(root));
        }
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        let data = if cfg!(target_os = "macos") {
            PathBuf::from(home).join("Library/Application Support")
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .filter(|data| !data.is_empty())
                .map_or_else(|| PathBuf::from(home).join(".local/share"), PathBuf::from)
        };
        Ok(data.join("Photon").join("Default"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder the Engine keeps cookies, site storage and its cache in.
    pub fn engine_dir(&self) -> PathBuf {
        self.root.join(ENGINE_DIR)
    }

    fn favicons_dir(&self) -> PathBuf {
        self.root.join(FAVICONS_DIR)
    }

    /// The saved history, or an empty one when there is none or it cannot
    /// be read.
    pub fn load_history(&self) -> History {
        fs::read(self.root.join(HISTORY_FILE))
            .ok()
            .and_then(|bytes| history_file::decode(&bytes))
            .unwrap_or_default()
    }

    pub fn save_history(&self, history: &History) -> io::Result<()> {
        atomic::write(
            &self.root.join(HISTORY_FILE),
            &history_file::encode(history),
        )
    }

    pub fn load_favicon(&self, url: &str) -> Option<FaviconPixels> {
        let bytes = fs::read(favicons::path(&self.favicons_dir(), url)).ok()?;
        FaviconPixels::decode(&bytes)
    }

    /// Saves `url`'s icon. Icons too large to be page icons are skipped.
    pub fn save_favicon(&self, url: &str, favicon: &FaviconPixels) -> io::Result<()> {
        if !favicon.is_storable() {
            return Ok(());
        }
        atomic::write(
            &favicons::path(&self.favicons_dir(), url),
            &favicon.encode(),
        )
    }

    /// Removes the icons of pages no longer in `history`.
    pub fn prune_favicons(&self, history: &History) -> io::Result<()> {
        let directory = self.favicons_dir();
        let kept: HashSet<PathBuf> = history
            .pages()
            .iter()
            .map(|entry| favicons::path(&directory, &entry.url))
            .collect();
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if !kept.contains(&path) {
                remove_if_present(&path)?;
            }
        }
        Ok(())
    }

    /// Deletes Photon's own data that `request` asks for from `history` and
    /// disk. Website data is the Engine's to delete.
    pub fn clear(&self, request: &ClearBrowsingData, history: &mut History) -> io::Result<()> {
        history.clear(request);
        self.save_history(history)?;
        self.prune_favicons(history)
    }

    /// How much disk the profile's data takes.
    pub fn data_usage(&self) -> DataUsage {
        DataUsage {
            history: file_size(&self.root.join(HISTORY_FILE)),
            favicons: folder_size(&self.favicons_dir()),
            website_data: folder_size(&self.engine_dir()),
        }
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

fn folder_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_size(&entry.path()),
            Ok(_) => entry.metadata().map_or(0, |metadata| metadata.len()),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile in a fresh temporary folder, removed when dropped.
    struct TemporaryProfile(Profile);

    impl TemporaryProfile {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("photon-storage-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Self(Profile::open(root).expect("profile opens"))
        }
    }

    impl Drop for TemporaryProfile {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.root());
        }
    }

    fn icon() -> FaviconPixels {
        FaviconPixels {
            width: 2,
            height: 1,
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }
    }

    #[test]
    fn saves_and_loads_history() {
        let profile = TemporaryProfile::new("history");
        assert_eq!(profile.0.load_history(), History::default());
        let mut history = History::default();
        history.record_visit("https://example.com/", "Example", 5);
        history.record_search("photon browser", 6);
        profile.0.save_history(&history).expect("saves");
        assert_eq!(profile.0.load_history(), history);
    }

    #[test]
    fn ignores_an_unreadable_history() {
        let profile = TemporaryProfile::new("corrupt");
        fs::write(profile.0.root().join(HISTORY_FILE), b"{ not json").expect("writes");
        assert_eq!(profile.0.load_history(), History::default());
    }

    #[test]
    fn saves_icons_and_removes_those_of_forgotten_pages() {
        let profile = TemporaryProfile::new("favicons");
        let mut history = History::default();
        history.record_visit("https://kept.example/", "Kept", 1);
        history.record_visit("https://gone.example/", "Gone", 2);
        profile
            .0
            .save_favicon("https://kept.example/", &icon())
            .expect("saves");
        profile
            .0
            .save_favicon("https://gone.example/", &icon())
            .expect("saves");
        assert_eq!(
            profile.0.load_favicon("https://kept.example/"),
            Some(icon())
        );

        history.remove_page("https://gone.example/");
        profile.0.prune_favicons(&history).expect("prunes");
        assert_eq!(profile.0.load_favicon("https://gone.example/"), None);
        assert_eq!(
            profile.0.load_favicon("https://kept.example/"),
            Some(icon())
        );
    }

    #[test]
    fn clears_history_and_reports_usage() {
        let profile = TemporaryProfile::new("clear");
        let mut history = History::default();
        history.record_visit("https://example.com/", "Example", 1);
        profile.0.save_history(&history).expect("saves");
        profile
            .0
            .save_favicon("https://example.com/", &icon())
            .expect("saves");
        assert!(profile.0.data_usage().history > 0);
        assert!(profile.0.data_usage().favicons > 0);

        profile
            .0
            .clear(&ClearBrowsingData::everything(), &mut history)
            .expect("clears");
        assert!(history.pages().is_empty());
        assert_eq!(profile.0.load_history(), History::default());
        assert_eq!(profile.0.data_usage().favicons, 0);
    }
}
