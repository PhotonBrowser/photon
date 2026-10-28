//! Reading the history of other browsers installed for this user, so it can
//! be imported. Their databases may be open in the running browser, so each
//! is copied, with its write-ahead log, before it is read.

use photon_core::HistoryEntry;
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Seconds from 1601-01-01 (Chromium's epoch) to the Unix epoch.
const CHROMIUM_EPOCH_OFFSET: u64 = 11_644_473_600;
/// Seconds from the Unix epoch to 2001-01-01 (Safari's epoch).
const SAFARI_EPOCH_OFFSET: f64 = 978_307_200.0;
/// The most pages read from one browser, newest first.
const IMPORT_LIMIT: usize = 20_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    Chromium,
    Firefox,
    Safari,
}

/// A browser profile whose history can be imported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportSource {
    /// The browser's name, with the profile's when it has several.
    pub name: String,
    database: PathBuf,
    format: Format,
}

/// The browsers found for this user, with history to import.
pub fn import_sources() -> Vec<ImportSource> {
    let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) else {
        return Vec::new();
    };
    import_sources_in(Path::new(&home))
}

fn import_sources_in(home: &Path) -> Vec<ImportSource> {
    let mut sources = Vec::new();
    let (chromium_root, firefox_root): (PathBuf, PathBuf) = if cfg!(target_os = "macos") {
        let support = home.join("Library/Application Support");
        (support.clone(), support.join("Firefox"))
    } else {
        (home.join(".config"), home.join(".mozilla/firefox"))
    };
    let chromium_browsers: &[(&str, &str)] = if cfg!(target_os = "macos") {
        &[
            ("Google Chrome", "Google/Chrome"),
            ("Arc", "Arc/User Data"),
            ("Brave", "BraveSoftware/Brave-Browser"),
            ("Microsoft Edge", "Microsoft Edge"),
            ("Vivaldi", "Vivaldi"),
            ("Chromium", "Chromium"),
        ]
    } else {
        &[
            ("Google Chrome", "google-chrome"),
            ("Brave", "BraveSoftware/Brave-Browser"),
            ("Microsoft Edge", "microsoft-edge"),
            ("Vivaldi", "vivaldi"),
            ("Chromium", "chromium"),
        ]
    };
    for (name, folder) in chromium_browsers {
        sources.extend(chromium_profiles(name, &chromium_root.join(folder)));
    }
    sources.extend(firefox_profiles(&firefox_root));
    if cfg!(target_os = "macos") {
        let database = home.join("Library/Safari/History.db");
        // Reading it needs Full Disk Access, which an import reports.
        if database.exists() {
            sources.push(ImportSource {
                name: "Safari".into(),
                database,
                format: Format::Safari,
            });
        }
    }
    sources
}

fn chromium_profiles(browser: &str, root: &Path) -> Vec<ImportSource> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut profiles: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let folder = entry.file_name().to_string_lossy().into_owned();
            let is_profile = folder == "Default" || folder.starts_with("Profile ");
            let database = entry.path().join("History");
            (is_profile && database.is_file()).then_some((folder, database))
        })
        .collect();
    profiles.sort();
    let several = profiles.len() > 1;
    profiles
        .into_iter()
        .map(|(folder, database)| ImportSource {
            name: if several {
                format!("{browser} ({folder})")
            } else {
                browser.to_owned()
            },
            database,
            format: Format::Chromium,
        })
        .collect()
}

fn firefox_profiles(root: &Path) -> Vec<ImportSource> {
    let Ok(entries) = fs::read_dir(root.join("Profiles")).or_else(|_| fs::read_dir(root)) else {
        return Vec::new();
    };
    let mut profiles: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let database = entry.path().join("places.sqlite");
            let folder = entry.file_name().to_string_lossy().into_owned();
            // Folders are named "<salt>.<profile name>".
            let profile = folder
                .split_once('.')
                .map_or(folder.clone(), |(_, name)| name.to_owned());
            database.is_file().then_some((profile, database))
        })
        .collect();
    profiles.sort();
    let several = profiles.len() > 1;
    profiles
        .into_iter()
        .map(|(profile, database)| ImportSource {
            name: if several {
                format!("Firefox ({profile})")
            } else {
                "Firefox".into()
            },
            database,
            format: Format::Firefox,
        })
        .collect()
}

/// Reads `source`'s history, newest first.
pub fn read_history(source: &ImportSource) -> io::Result<Vec<HistoryEntry>> {
    let copy = tempfile::tempdir()?;
    let database = copy.path().join("history.sqlite");
    fs::copy(&source.database, &database)?;
    for suffix in ["-wal", "-shm"] {
        let mut companion = source.database.clone().into_os_string();
        companion.push(suffix);
        let companion = PathBuf::from(companion);
        if companion.is_file() {
            let mut target = database.clone().into_os_string();
            target.push(suffix);
            fs::copy(&companion, PathBuf::from(target))?;
        }
    }
    let connection = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(io::Error::other)?;
    read_entries(&connection, source.format).map_err(io::Error::other)
}

fn read_entries(connection: &Connection, format: Format) -> rusqlite::Result<Vec<HistoryEntry>> {
    let query = match format {
        Format::Chromium => {
            "SELECT url, title, visit_count, last_visit_time FROM urls \
             WHERE hidden = 0 ORDER BY last_visit_time DESC LIMIT ?1"
        }
        Format::Firefox => {
            "SELECT url, COALESCE(title, ''), visit_count, COALESCE(last_visit_date, 0) \
             FROM moz_places WHERE hidden = 0 AND visit_count > 0 \
             ORDER BY last_visit_date DESC LIMIT ?1"
        }
        Format::Safari => {
            "SELECT items.url, COALESCE(visits.title, ''), items.visit_count, MAX(visits.visit_time) \
             FROM history_items AS items JOIN history_visits AS visits ON visits.history_item = items.id \
             GROUP BY items.id ORDER BY MAX(visits.visit_time) DESC LIMIT ?1"
        }
    };
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([IMPORT_LIMIT as i64], |row| {
        let url: String = row.get(0)?;
        let title: String = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let last_visit = match format {
            // Microseconds since 1601.
            Format::Chromium => (row.get::<_, i64>(3)?.max(0) as u64 / 1_000_000)
                .saturating_sub(CHROMIUM_EPOCH_OFFSET),
            // Microseconds since 1970.
            Format::Firefox => row.get::<_, i64>(3)?.max(0) as u64 / 1_000_000,
            // Seconds since 2001.
            Format::Safari => (row.get::<_, f64>(3)? + SAFARI_EPOCH_OFFSET).max(0.0) as u64,
        };
        Ok(HistoryEntry {
            url,
            title,
            visit_count: visit_count.clamp(1, i64::from(u32::MAX)) as u32,
            last_visit,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chromium_database(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, \
                 last_visit_time INTEGER, hidden INTEGER);
                 INSERT INTO urls VALUES (1, 'https://a.example/', 'A', 3, 13400000000000000, 0);
                 INSERT INTO urls VALUES (2, 'https://hidden.example/', 'H', 1, 13400000000000000, 1);",
            )
            .unwrap();
    }

    #[test]
    fn finds_and_reads_chromium_profiles() {
        let home = tempfile::tempdir().unwrap();
        let root = if cfg!(target_os = "macos") {
            home.path()
                .join("Library/Application Support/Google/Chrome")
        } else {
            home.path().join(".config/google-chrome")
        };
        for profile in ["Default", "Profile 1"] {
            fs::create_dir_all(root.join(profile)).unwrap();
            chromium_database(&root.join(profile).join("History"));
        }
        let sources = import_sources_in(home.path());
        let names: Vec<&str> = sources.iter().map(|source| source.name.as_str()).collect();
        assert_eq!(
            names,
            ["Google Chrome (Default)", "Google Chrome (Profile 1)"]
        );
        let pages = read_history(&sources[0]).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].url, "https://a.example/");
        assert_eq!(pages[0].visit_count, 3);
        assert_eq!(pages[0].last_visit, 13_400_000_000 - CHROMIUM_EPOCH_OFFSET);
    }

    #[test]
    fn reads_firefox_places() {
        let home = tempfile::tempdir().unwrap();
        let root = if cfg!(target_os = "macos") {
            home.path()
                .join("Library/Application Support/Firefox/Profiles/abc.default-release")
        } else {
            home.path()
                .join(".mozilla/firefox/Profiles/abc.default-release")
        };
        fs::create_dir_all(&root).unwrap();
        let connection = Connection::open(root.join("places.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, \
                 last_visit_date INTEGER, hidden INTEGER);
                 INSERT INTO moz_places VALUES (1, 'https://f.example/', NULL, 2, 1700000000000000, 0);",
            )
            .unwrap();
        drop(connection);
        let sources = import_sources_in(home.path());
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "Firefox");
        let pages = read_history(&sources[0]).unwrap();
        assert_eq!(pages[0].last_visit, 1_700_000_000);
        assert_eq!(pages[0].title, "");
    }
}
