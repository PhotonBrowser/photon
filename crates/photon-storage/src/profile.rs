//! A profile folder and what is kept in it.

use photon_core::{
    BrowserSettings, ClearBrowsingData, History, MAX_FAVOURITES, Session, WorkspaceSettings,
};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::favicons::{self, FaviconPixels};
use super::{ProfileLoadOutcome, atomic, history_file, session_file};

const HISTORY_FILE: &str = "History.json";
const SETTINGS_FILE: &str = "config.toml";
const SESSION_FILE: &str = "Session.json";
const FAVICONS_DIR: &str = "Favicons";
const ENGINE_DIR: &str = "Engine";
/// Where in its folder the Engine keeps its network cache.
const ENGINE_CACHE_DIR: &str = "cache";
/// Profile files are capped before their contents are parsed.
const MAX_SETTINGS_FILE_BYTES: usize = 1024 * 1024;
const MAX_SETTINGS_DEPTH: usize = 32;
const MAX_SETTINGS_COLLECTION_ITEMS: usize = 4_096;
const MAX_SETTINGS_STRING_BYTES: usize = 8 * 1024;
const MAX_SETTINGS_KEY_BYTES: usize = 256;
const MAX_WORKSPACE_PRESETS: usize = 32;
const MAX_LAYOUT_COMPONENTS: usize = 64;
const MAX_SPACES: usize = 128;
const MAX_SPACE_PINS: usize = 256;
const MAX_NEW_TAB_PINS: usize = 12;
const MAX_NEW_TAB_HIDDEN: usize = 2_048;
const MAX_LEGACY_FAVOURITES: usize = 256;
static NEXT_DAMAGED_BACKUP: AtomicU64 = AtomicU64::new(0);

/// How much disk a profile's data takes, in bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DataUsage {
    pub history: u64,
    pub favicons: u64,
    /// The Engine's network cache.
    pub cache: u64,
    /// The Engine's cookies and site storage.
    pub site_data: u64,
}

impl DataUsage {
    pub fn total(&self) -> u64 {
        self.history + self.favicons + self.cache + self.site_data
    }
}

/// One profile folder.
#[derive(Clone, Debug)]
pub struct Profile {
    root: PathBuf,
    settings_path: PathBuf,
    writes: Arc<ProfileWrites>,
}

/// Identifies a replaceable profile snapshot when it is scheduled for saving.
/// A newer ticket makes an older queued snapshot safe to skip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileWriteTicket {
    kind: SnapshotKind,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotKind {
    Settings,
    History,
    Session,
}

#[derive(Debug, Default)]
struct ProfileWrites {
    settings_generation: AtomicU64,
    history_generation: AtomicU64,
    session_generation: AtomicU64,
    history_source_checked: AtomicBool,
    session_source_checked: AtomicBool,
    history_write_blocked: AtomicBool,
    session_write_blocked: AtomicBool,
    serial: Mutex<()>,
}

impl ProfileWrites {
    fn reserve(&self, kind: SnapshotKind) -> ProfileWriteTicket {
        let generation = self
            .generation(kind)
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        ProfileWriteTicket { kind, generation }
    }

    fn generation(&self, kind: SnapshotKind) -> &AtomicU64 {
        match kind {
            SnapshotKind::Settings => &self.settings_generation,
            SnapshotKind::History => &self.history_generation,
            SnapshotKind::Session => &self.session_generation,
        }
    }

    fn write_blocked(&self, kind: SnapshotKind) -> Option<&AtomicBool> {
        match kind {
            SnapshotKind::History => Some(&self.history_write_blocked),
            SnapshotKind::Session => Some(&self.session_write_blocked),
            SnapshotKind::Settings => None,
        }
    }

    fn source_checked(&self, kind: SnapshotKind) -> Option<&AtomicBool> {
        match kind {
            SnapshotKind::History => Some(&self.history_source_checked),
            SnapshotKind::Session => Some(&self.session_source_checked),
            SnapshotKind::Settings => None,
        }
    }
}

impl Profile {
    /// Opens the profile at `root`, applying migrations before returning it.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        Self::open_with_settings_path(root, None)
    }

    fn open_with_settings_path(
        root: impl Into<PathBuf>,
        settings_path: Option<PathBuf>,
    ) -> io::Result<Self> {
        let mut profile = Self::create(root)?;
        if let Some(settings_path) = settings_path {
            profile.settings_path = settings_path;
        }
        super::migrations::run(&profile)?;
        profile.ensure_settings()?;
        Ok(profile)
    }

    fn create(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let profile = Self {
            settings_path: root.join(SETTINGS_FILE),
            root,
            writes: Arc::new(ProfileWrites::default()),
        };
        fs::create_dir_all(profile.favicons_dir())?;
        fs::create_dir_all(profile.engine_dir())?;
        Ok(profile)
    }

    /// Opens the default profile: `PHOTON_PROFILE_DIR` when set, otherwise
    /// the platform's application data folder.
    pub fn open_default() -> io::Result<Self> {
        let isolated_profile =
            std::env::var_os("PHOTON_PROFILE_DIR").is_some_and(|root| !root.is_empty());
        let root = Self::default_root()?;
        // Managed automation sessions must not read or write the user's
        // global Photon settings file.
        let settings_path = (!isolated_profile)
            .then(Self::default_settings_path)
            .transpose()?;
        Self::open_with_settings_path(root, settings_path)
    }

    fn ensure_settings(&self) -> io::Result<()> {
        if self.settings_path.exists() {
            return Ok(());
        }
        self.save_settings(&BrowserSettings::default())
    }

    fn default_settings_path() -> io::Result<PathBuf> {
        if cfg!(target_os = "macos") {
            let home = std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
            Ok(PathBuf::from(home)
                .join("Library/Application Support")
                .join(photon_brand::DATA_FOLDER)
                .join(SETTINGS_FILE))
        } else {
            let config = std::env::var_os("XDG_CONFIG_HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .filter(|home| !home.is_empty())
                        .map(|home| PathBuf::from(home).join(".config"))
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "XDG_CONFIG_HOME and HOME are not set",
                    )
                })?;
            Ok(config.join(photon_brand::DATA_FOLDER).join(SETTINGS_FILE))
        }
    }

    /// The user-editable browser settings file.
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
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
        Ok(data.join(photon_brand::DATA_FOLDER).join("Default"))
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

    /// The saved history, or an empty one when there is none or it cannot be
    /// read. Use [`Self::load_history_outcome`] to distinguish these cases.
    pub fn load_history(&self) -> History {
        self.load_history_outcome().into_value()
    }

    /// Loads history while reporting missing, partially recovered, unsupported
    /// and corrupt files separately.
    pub fn load_history_outcome(&self) -> ProfileLoadOutcome<History> {
        self.load_profile_data(
            SnapshotKind::History,
            HISTORY_FILE,
            history_file::MAX_HISTORY_FILE_BYTES,
            history_file::decode,
        )
    }

    pub fn save_history(&self, history: &History) -> io::Result<()> {
        let ticket = self.begin_history_write();
        self.save_history_if_current(ticket, history).map(|_| ())
    }

    /// Reserves an ordered position for a history snapshot before dispatching
    /// its disk write to background work.
    pub fn begin_history_write(&self) -> ProfileWriteTicket {
        self.writes.reserve(SnapshotKind::History)
    }

    /// Saves `history` only when no newer history snapshot has been scheduled.
    /// Returns `false` when a newer snapshot superseded this one.
    pub fn save_history_if_current(
        &self,
        ticket: ProfileWriteTicket,
        history: &History,
    ) -> io::Result<bool> {
        self.write_snapshot(ticket, SnapshotKind::History, || {
            atomic::write(
                &self.root.join(HISTORY_FILE),
                &history_file::encode(history)?,
            )
        })
    }

    /// The saved preferences, or defaults when there are none or they cannot
    /// be read. Unknown fields are ignored so older builds can open newer
    /// profiles safely.
    pub fn load_settings(&self) -> BrowserSettings {
        let bytes = match read_limited(&self.settings_path, MAX_SETTINGS_FILE_BYTES) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(_) => return BrowserSettings::default(),
        };
        let Ok(source) = std::str::from_utf8(&bytes) else {
            self.keep_unreadable_settings(&bytes);
            return BrowserSettings::default();
        };
        let Ok(document) = toml::from_str::<toml::Value>(source) else {
            self.keep_unreadable_settings(&bytes);
            return BrowserSettings::default();
        };
        if !valid_settings_document(&document) {
            self.keep_unreadable_settings(&bytes);
            return BrowserSettings::default();
        }

        let (mut settings, partially_unreadable) = decode_toml_settings(document);
        settings.sidebar.width = settings.sidebar.width();
        settings.new_tab.shortcut_count = settings.new_tab.shortcut_limit();
        if !valid_settings_state(&settings) {
            self.keep_unreadable_settings(&bytes);
            return BrowserSettings::default();
        }
        if partially_unreadable {
            self.keep_unreadable_settings(&bytes);
        }
        settings
    }

    /// Keeps a settings file that could not be read in full beside the
    /// profile, as the next save replaces it with what could be read.
    pub(super) fn keep_unreadable_settings(&self, bytes: &[u8]) {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let backup = self.root.join(format!("config.unreadable-{seconds}.bak"));
        let _ = atomic::write(&backup, bytes);
    }

    pub fn save_settings(&self, settings: &BrowserSettings) -> io::Result<()> {
        let ticket = self.begin_settings_write();
        self.save_settings_if_current(ticket, settings).map(|_| ())
    }

    /// Reserves an ordered position for a settings snapshot before dispatching
    /// its disk write to background work.
    pub fn begin_settings_write(&self) -> ProfileWriteTicket {
        self.writes.reserve(SnapshotKind::Settings)
    }

    /// Saves `settings` only when no newer settings snapshot has been scheduled.
    /// Returns `false` when a newer snapshot superseded this one.
    pub fn save_settings_if_current(
        &self,
        ticket: ProfileWriteTicket,
        settings: &BrowserSettings,
    ) -> io::Result<bool> {
        self.write_snapshot(ticket, SnapshotKind::Settings, || {
            let contents = toml::to_string_pretty(settings)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let document = toml::from_str::<toml::Value>(&contents)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if contents.len() > MAX_SETTINGS_FILE_BYTES
                || !valid_settings_state(settings)
                || !valid_settings_document(&document)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "settings exceed a profile data limit",
                ));
            }
            atomic::write(&self.settings_path, contents.as_bytes())
        })
    }

    /// The windows and tabs open when the browser last saved them, or none
    /// when there is no session or it cannot be read. Use
    /// [`Self::load_session_outcome`] to distinguish these cases.
    pub fn load_session(&self) -> Session {
        self.load_session_outcome().into_value()
    }

    /// Loads a session while reporting missing, partially recovered,
    /// unsupported and corrupt files separately.
    pub fn load_session_outcome(&self) -> ProfileLoadOutcome<Session> {
        self.load_profile_data(
            SnapshotKind::Session,
            SESSION_FILE,
            session_file::MAX_SESSION_FILE_BYTES,
            session_file::decode,
        )
    }

    pub fn save_session(&self, session: &Session) -> io::Result<()> {
        let ticket = self.begin_session_write();
        self.save_session_if_current(ticket, session).map(|_| ())
    }

    /// Reserves an ordered position for a session snapshot before dispatching
    /// its disk write to background work.
    pub fn begin_session_write(&self) -> ProfileWriteTicket {
        self.writes.reserve(SnapshotKind::Session)
    }

    /// Saves `session` only when no newer session snapshot has been scheduled.
    /// Returns `false` when a newer snapshot superseded this one.
    pub fn save_session_if_current(
        &self,
        ticket: ProfileWriteTicket,
        session: &Session,
    ) -> io::Result<bool> {
        self.write_snapshot(ticket, SnapshotKind::Session, || {
            let bytes = session_file::encode(session)?;
            atomic::write(&self.root.join(SESSION_FILE), &bytes)
        })
    }

    fn write_snapshot(
        &self,
        ticket: ProfileWriteTicket,
        kind: SnapshotKind,
        write: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<bool> {
        if ticket.kind != kind {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "profile write ticket has the wrong kind",
            ));
        }
        let _serial = self
            .writes
            .serial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.writes.generation(kind).load(Ordering::Acquire) != ticket.generation {
            return Ok(false);
        }
        if self
            .writes
            .source_checked(kind)
            .is_some_and(|checked| !checked.load(Ordering::Acquire))
        {
            self.preserve_existing_data_before_write(kind);
        }
        if self
            .writes
            .write_blocked(kind)
            .is_some_and(|blocked| blocked.load(Ordering::Acquire))
        {
            let name = match kind {
                SnapshotKind::History => "history",
                SnapshotKind::Session => "session",
                SnapshotKind::Settings => unreachable!(),
            };
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "damaged profile {name} data could not be preserved; refusing to replace it"
                ),
            ));
        }
        write()?;
        Ok(true)
    }

    fn load_profile_data<T>(
        &self,
        kind: SnapshotKind,
        filename: &str,
        limit: usize,
        decode: impl FnOnce(&[u8]) -> ProfileLoadOutcome<T>,
    ) -> ProfileLoadOutcome<T> {
        let _serial = self
            .writes
            .serial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.load_profile_data_unlocked(kind, filename, limit, decode)
    }

    fn load_profile_data_unlocked<T>(
        &self,
        kind: SnapshotKind,
        filename: &str,
        limit: usize,
        decode: impl FnOnce(&[u8]) -> ProfileLoadOutcome<T>,
    ) -> ProfileLoadOutcome<T> {
        let path = self.root.join(filename);
        let read = read_limited(&path, limit);
        let (outcome, read_error) = match read {
            Ok(Some(bytes)) => (decode(&bytes), None),
            Ok(None) => {
                self.mark_source_checked(kind);
                return ProfileLoadOutcome::Missing;
            }
            Err(error) => (ProfileLoadOutcome::Corrupt, Some(error)),
        };
        let Some(status) = damaged_outcome_name(&outcome) else {
            if let Some(blocked) = self.writes.write_blocked(kind) {
                blocked.store(false, Ordering::Release);
            }
            self.mark_source_checked(kind);
            return outcome;
        };

        let data_name = match kind {
            SnapshotKind::History => "history",
            SnapshotKind::Session => "session",
            SnapshotKind::Settings => unreachable!(),
        };
        if let Some(blocked) = self.writes.write_blocked(kind) {
            blocked.store(true, Ordering::Release);
        }
        match archive_damaged_file(&path) {
            Ok(backup) => {
                if let Some(blocked) = self.writes.write_blocked(kind) {
                    blocked.store(false, Ordering::Release);
                }
                let backup_name = backup.file_name().unwrap_or_default().to_string_lossy();
                let read_detail = read_error
                    .as_ref()
                    .map(|error| format!(" (read failed: {error})"))
                    .unwrap_or_default();
                eprintln!(
                    "Photon profile {data_name} data is {status}{read_detail}; preserved it as {backup_name}"
                );
            }
            Err(error) => {
                let backup_error = error.to_string();
                let detail = read_error
                    .as_ref()
                    .map(|read_error| {
                        format!("reading failed: {read_error}; backup failed: {backup_error}")
                    })
                    .unwrap_or_else(|| format!("backup failed: {backup_error}"));
                eprintln!(
                    "Photon profile {data_name} data is {status}; could not preserve it before replacement ({detail})"
                );
            }
        }
        self.mark_source_checked(kind);
        outcome
    }

    fn mark_source_checked(&self, kind: SnapshotKind) {
        if let Some(checked) = self.writes.source_checked(kind) {
            checked.store(true, Ordering::Release);
        }
    }

    fn preserve_existing_data_before_write(&self, kind: SnapshotKind) {
        match kind {
            SnapshotKind::History => {
                let _ = self.load_profile_data_unlocked(
                    kind,
                    HISTORY_FILE,
                    history_file::MAX_HISTORY_FILE_BYTES,
                    history_file::decode,
                );
            }
            SnapshotKind::Session => {
                let _ = self.load_profile_data_unlocked(
                    kind,
                    SESSION_FILE,
                    session_file::MAX_SESSION_FILE_BYTES,
                    session_file::decode,
                );
            }
            SnapshotKind::Settings => {}
        }
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
        let _serial = self
            .writes
            .serial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        atomic::write(
            &favicons::path(&self.favicons_dir(), url),
            &favicon.encode(),
        )
    }

    /// Removes the icons of pages no longer in `history`.
    pub fn prune_favicons(&self, history: &History) -> io::Result<()> {
        let _serial = self
            .writes
            .serial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.prune_favicons_unlocked(history)
    }

    fn prune_favicons_unlocked(&self, history: &History) -> io::Result<()> {
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
            cache: folder_size(&self.engine_dir().join(ENGINE_CACHE_DIR)),
            site_data: folder_size(&self.engine_dir())
                .saturating_sub(folder_size(&self.engine_dir().join(ENGINE_CACHE_DIR))),
        }
    }
}

fn read_limited(path: &Path, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "profile file exceeds its size limit",
        ));
    }
    Ok(Some(bytes))
}

fn valid_settings_document(document: &toml::Value) -> bool {
    if !matches!(document, toml::Value::Table(_)) || !valid_toml_value(document, 0) {
        return false;
    }

    let spaces = document.get("spaces");
    if !array_within(spaces.and_then(|spaces| spaces.get("spaces")), MAX_SPACES) {
        return false;
    }
    if let Some(toml::Value::Array(spaces)) = spaces.and_then(|spaces| spaces.get("spaces")) {
        for space in spaces {
            if !array_within(space.get("pinned_tabs"), MAX_SPACE_PINS)
                || !array_within(space.get("favourites"), MAX_FAVOURITES)
                || !layout_components_within(space.get("layout_override"))
            {
                return false;
            }
        }
    }

    let new_tab = document.get("new_tab");
    if !array_within(
        new_tab.and_then(|new_tab| new_tab.get("pinned")),
        MAX_NEW_TAB_PINS,
    ) || !array_within(
        new_tab.and_then(|new_tab| new_tab.get("hidden")),
        MAX_NEW_TAB_HIDDEN,
    ) || !array_within(
        document
            .get("sidebar")
            .and_then(|sidebar| sidebar.get("favourites")),
        MAX_LEGACY_FAVOURITES,
    ) {
        return false;
    }

    if let Some(workspace) = document.get("workspace") {
        if !array_within(workspace.get("presets"), MAX_WORKSPACE_PRESETS) {
            return false;
        }
        if let Some(toml::Value::Array(presets)) = workspace.get("presets")
            && presets
                .iter()
                .any(|preset| !layout_components_within(preset.get("layout")))
        {
            return false;
        }
    }
    true
}

fn valid_toml_value(value: &toml::Value, depth: usize) -> bool {
    if depth > MAX_SETTINGS_DEPTH {
        return false;
    }
    match value {
        toml::Value::String(string) => string.len() <= MAX_SETTINGS_STRING_BYTES,
        toml::Value::Array(items) => {
            items.len() <= MAX_SETTINGS_COLLECTION_ITEMS
                && items.iter().all(|item| valid_toml_value(item, depth + 1))
        }
        toml::Value::Table(fields) => {
            fields.len() <= MAX_SETTINGS_COLLECTION_ITEMS
                && fields.iter().all(|(key, value)| {
                    key.len() <= MAX_SETTINGS_KEY_BYTES && valid_toml_value(value, depth + 1)
                })
        }
        _ => true,
    }
}

fn array_within(value: Option<&toml::Value>, limit: usize) -> bool {
    value.is_none_or(|value| match value {
        toml::Value::Array(items) => items.len() <= limit,
        _ => true,
    })
}

fn layout_components_within(value: Option<&toml::Value>) -> bool {
    array_within(
        value.and_then(|layout| layout.get("components")),
        MAX_LAYOUT_COMPONENTS,
    )
}

fn valid_settings_state(settings: &BrowserSettings) -> bool {
    if !settings.workspace.is_valid()
        || settings.spaces.all().len() > MAX_SPACES
        || settings.new_tab.pinned.len() > MAX_NEW_TAB_PINS
        || settings.new_tab.hidden.len() > MAX_NEW_TAB_HIDDEN
        || settings.sidebar.favourites.len() > MAX_LEGACY_FAVOURITES
        || settings.default_search_engine.len() > 1_024
    {
        return false;
    }
    let mut ids = HashSet::new();
    settings.spaces.all().iter().all(|space| {
        ids.insert(space.id)
            && space.name.len() <= 256
            && space.pinned_tabs.len() <= MAX_SPACE_PINS
            && space.favourites.len() <= MAX_FAVOURITES
            && space
                .pinned_tabs
                .iter()
                .chain(&space.favourites)
                .all(|shortcut| shortcut.title.len() <= 1_024 && shortcut.url.len() <= 8 * 1024)
    }) && settings
        .new_tab
        .pinned
        .iter()
        .all(|shortcut| shortcut.title.len() <= 1_024 && shortcut.url.len() <= 8 * 1024)
        && settings
            .new_tab
            .hidden
            .iter()
            .all(|url| url.len() <= 8 * 1024)
        && settings
            .sidebar
            .favourites
            .iter()
            .all(|shortcut| shortcut.title.len() <= 1_024 && shortcut.url.len() <= 8 * 1024)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn damaged_outcome_name<T>(outcome: &ProfileLoadOutcome<T>) -> Option<&'static str> {
    match outcome {
        ProfileLoadOutcome::PartiallyRecovered(_) => Some("partially recovered"),
        ProfileLoadOutcome::Unsupported => Some("unsupported"),
        ProfileLoadOutcome::Corrupt => Some("corrupt or unreadable"),
        ProfileLoadOutcome::Missing | ProfileLoadOutcome::Loaded(_) => None,
    }
}

fn archive_damaged_file(path: &Path) -> io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "profile file has no name"))?
        .to_string_lossy();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    for _ in 0..100 {
        let sequence = NEXT_DAMAGED_BACKUP.fetch_add(1, Ordering::Relaxed);
        let backup = parent.join(format!(
            "{filename}.damaged-{timestamp}-{}-{sequence}.bak",
            std::process::id()
        ));
        match fs::rename(path, &backup) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not find a unique damaged profile backup name",
    ))
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

fn decode_toml_settings(mut document: toml::Value) -> (BrowserSettings, bool) {
    let saved_workspace = document
        .as_table_mut()
        .and_then(|settings| settings.remove("workspace"))
        .filter(|workspace| {
            workspace.as_table().is_some_and(|workspace| {
                workspace.contains_key("default_preset_id") && workspace.contains_key("presets")
            })
        });
    let parsed = document.clone().try_into::<BrowserSettings>();
    let partially_unreadable = parsed.is_err();
    let mut settings = parsed.unwrap_or_else(|_| readable_toml_settings(document));
    settings.workspace = saved_workspace
        .and_then(|workspace| workspace.try_into::<WorkspaceSettings>().ok())
        .filter(WorkspaceSettings::is_valid)
        .unwrap_or_else(|| WorkspaceSettings::for_tab_layout(settings.tab_layout));
    settings.spaces.validate_layout_overrides();
    (settings, partially_unreadable)
}

/// Keeps readable settings and defaults only invalid fields when a newer
/// build writes a value this build does not understand.
fn readable_toml_settings(document: toml::Value) -> BrowserSettings {
    let toml::Value::Table(fields) = document else {
        return BrowserSettings::default();
    };
    let readable = fields
        .into_iter()
        .filter(|(key, value)| {
            let single =
                toml::Value::Table(toml::map::Map::from_iter([(key.clone(), value.clone())]));
            single.try_into::<BrowserSettings>().is_ok()
        })
        .collect();
    toml::Value::Table(readable)
        .try_into::<BrowserSettings>()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile in a fresh temporary folder, removed when dropped.
    struct TemporaryProfile(Profile);

    impl TemporaryProfile {
        fn new(name: &str) -> Self {
            let root = Self::root(name);
            Self(Profile::open(root).expect("profile opens"))
        }

        fn unmigrated(name: &str) -> Self {
            let root = Self::root(name);
            Self(Profile::create(root).expect("profile creates"))
        }

        fn root(name: &str) -> PathBuf {
            let root = std::env::temp_dir()
                .join(format!("photon-storage-test-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            root
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

    fn table<K: Into<String>>(fields: impl IntoIterator<Item = (K, toml::Value)>) -> toml::Value {
        toml::Value::Table(
            fields
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    fn array(count: usize) -> toml::Value {
        toml::Value::Array(vec![toml::Value::Boolean(true); count])
    }

    #[test]
    fn profile_file_reads_stop_at_the_limit_and_keep_the_source_unchanged() {
        let profile = TemporaryProfile::new("bounded-file-read");
        let path = profile.0.root().join("bounded.bin");
        fs::write(&path, b"12345678").expect("writes boundary-sized file");
        assert_eq!(
            read_limited(&path, 8).expect("reads at limit"),
            Some(b"12345678".to_vec())
        );

        fs::write(&path, b"123456789").expect("writes oversized file");
        assert!(read_limited(&path, 8).is_err());
        assert_eq!(fs::read(&path).expect("source stays intact"), b"123456789");
    }

    #[test]
    fn settings_input_checks_string_depth_and_collection_boundaries() {
        let at_string_limit = table([(
            "unknown",
            toml::Value::String("x".repeat(MAX_SETTINGS_STRING_BYTES)),
        )]);
        let over_string_limit = table([(
            "unknown",
            toml::Value::String("x".repeat(MAX_SETTINGS_STRING_BYTES + 1)),
        )]);
        assert!(valid_settings_document(&at_string_limit));
        assert!(!valid_settings_document(&over_string_limit));

        let at_collection_limit = table([("unknown", array(MAX_SETTINGS_COLLECTION_ITEMS))]);
        let over_collection_limit = table([("unknown", array(MAX_SETTINGS_COLLECTION_ITEMS + 1))]);
        assert!(valid_settings_document(&at_collection_limit));
        assert!(!valid_settings_document(&over_collection_limit));

        let at_depth_limit = (0..MAX_SETTINGS_DEPTH - 1)
            .fold(toml::Value::Boolean(true), |value, _| {
                toml::Value::Array(vec![value])
            });
        let at_depth_limit = table([("unknown", at_depth_limit)]);
        let over_depth_limit = (0..MAX_SETTINGS_DEPTH)
            .fold(toml::Value::Boolean(true), |value, _| {
                toml::Value::Array(vec![value])
            });
        let over_depth_limit = table([("unknown", over_depth_limit)]);
        assert!(valid_settings_document(&at_depth_limit));
        assert!(!valid_settings_document(&over_depth_limit));

        let at_space_limit = table([("spaces", table([("spaces", array(MAX_SPACES))]))]);
        let over_space_limit = table([("spaces", table([("spaces", array(MAX_SPACES + 1))]))]);
        assert!(valid_settings_document(&at_space_limit));
        assert!(!valid_settings_document(&over_space_limit));

        let at_pin_limit = table([("new_tab", table([("pinned", array(MAX_NEW_TAB_PINS))]))]);
        let over_pin_limit = table([("new_tab", table([("pinned", array(MAX_NEW_TAB_PINS + 1))]))]);
        assert!(valid_settings_document(&at_pin_limit));
        assert!(!valid_settings_document(&over_pin_limit));

        let layout = |count| table([("components", array(count))]);
        let preset_doc = |count, components| {
            table([(
                "workspace",
                table([(
                    "presets",
                    toml::Value::Array(vec![table([("layout", layout(components))]); count]),
                )]),
            )])
        };
        assert!(valid_settings_document(&preset_doc(
            MAX_WORKSPACE_PRESETS,
            0
        )));
        assert!(!valid_settings_document(&preset_doc(
            MAX_WORKSPACE_PRESETS + 1,
            0
        )));
        assert!(valid_settings_document(&preset_doc(
            1,
            MAX_LAYOUT_COMPONENTS
        )));
        assert!(!valid_settings_document(&preset_doc(
            1,
            MAX_LAYOUT_COMPONENTS + 1
        )));

        let space_doc = |count| {
            table([(
                "spaces",
                table([(
                    "spaces",
                    toml::Value::Array(vec![table([("pinned_tabs", array(count))])]),
                )]),
            )])
        };
        assert!(valid_settings_document(&space_doc(MAX_SPACE_PINS)));
        assert!(!valid_settings_document(&space_doc(MAX_SPACE_PINS + 1)));

        let key_at_limit = table([(
            "k".repeat(MAX_SETTINGS_KEY_BYTES),
            toml::Value::Boolean(true),
        )]);
        let key_over_limit = table([(
            "k".repeat(MAX_SETTINGS_KEY_BYTES + 1),
            toml::Value::Boolean(true),
        )]);
        assert!(valid_settings_document(&key_at_limit));
        assert!(!valid_settings_document(&key_over_limit));
    }

    fn migrate_legacy_json(profile: &TemporaryProfile, contents: &[u8]) {
        let legacy_path = profile.0.root().join("config.json");
        fs::write(&legacy_path, contents).expect("writes legacy settings");
        crate::migrations::run(&profile.0).expect("migrates legacy settings");
        assert!(!legacy_path.exists());
        assert!(profile.0.settings_path().exists());
        let backup_prefix = "config.json.migrated-";
        assert!(
            fs::read_dir(profile.0.root())
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(backup_prefix))
        );
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
        assert_eq!(
            profile.0.load_history_outcome(),
            ProfileLoadOutcome::Loaded(history)
        );
    }

    #[test]
    fn keeps_readable_settings_and_backs_up_the_file_when_one_is_not() {
        let profile = TemporaryProfile::new("lenient");
        let path = profile.0.settings_path().to_owned();
        let contents = "show_full_address = true\ntheme = \"from-a-newer-build\"\n";
        std::fs::write(&path, contents).unwrap();
        let settings = profile.0.load_settings();
        assert!(settings.show_full_address);
        assert_eq!(settings.theme, BrowserSettings::default().theme);
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
        let backups = std::fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.unreadable-")
            })
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn saves_and_loads_the_session() {
        let profile = TemporaryProfile::new("session");
        assert_eq!(profile.0.load_session(), Session::default());
        let session = Session {
            windows: vec![photon_core::WindowSession {
                tabs: vec![photon_core::SessionTab {
                    url: "https://example.com/".to_owned(),
                    title: "Example".to_owned(),
                    space: 1,
                }],
                active: Some(0),
            }],
        };
        profile.0.save_session(&session).expect("saves");
        assert_eq!(profile.0.load_session(), session);
        assert_eq!(
            profile.0.load_session_outcome(),
            ProfileLoadOutcome::Loaded(session)
        );
    }

    #[test]
    fn rejects_oversized_settings_without_changing_the_source_file() {
        let profile = TemporaryProfile::new("oversized-settings");
        let contents = vec![b'#'; MAX_SETTINGS_FILE_BYTES + 1];
        fs::write(profile.0.settings_path(), &contents).expect("writes oversized settings");

        assert_eq!(profile.0.load_settings(), BrowserSettings::default());
        assert_eq!(fs::read(profile.0.settings_path()).unwrap(), contents);
    }

    #[test]
    fn preserves_invalid_session_before_a_later_save_replaces_it() {
        let profile = TemporaryProfile::new("invalid-session");
        let contents = serde_json::to_vec(&serde_json::json!({
            "windows": [{"tabs": [], "active": 0}]
        }))
        .unwrap();
        let path = profile.0.root().join(SESSION_FILE);
        fs::write(&path, &contents).expect("writes invalid session");

        let ProfileLoadOutcome::PartiallyRecovered(session) = profile.0.load_session_outcome()
        else {
            panic!("recovers the session structure");
        };
        assert!(session.windows[0].tabs.is_empty());
        assert_eq!(session.windows[0].active, None);
        assert!(!path.exists());
        let backup = fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Session.json.damaged-")
            })
            .expect("preserves the invalid session");
        assert_eq!(fs::read(backup.path()).unwrap(), contents);

        let session = Session::default();
        profile
            .0
            .save_session(&session)
            .expect("saves recovered session");
        assert!(path.exists());
        assert_eq!(fs::read(backup.path()).unwrap(), contents);
    }

    #[test]
    fn direct_save_preserves_damaged_session_before_replacing_it() {
        let profile = TemporaryProfile::new("direct-damaged-session-save");
        let path = profile.0.root().join(SESSION_FILE);
        let contents = b"not session JSON";
        fs::write(&path, contents).expect("writes damaged session");

        profile
            .0
            .save_session(&Session::default())
            .expect("saves a new session");

        let backup = fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Session.json.damaged-")
            })
            .expect("preserves the file before saving");
        assert_eq!(fs::read(backup.path()).unwrap(), contents);
        assert_eq!(
            profile.0.load_session_outcome(),
            ProfileLoadOutcome::Loaded(Session::default())
        );
    }

    #[test]
    fn distinguishes_missing_and_unsupported_history_files() {
        let profile = TemporaryProfile::new("history-load-outcomes");
        let path = profile.0.root().join(HISTORY_FILE);
        assert_eq!(
            profile.0.load_history_outcome(),
            ProfileLoadOutcome::Missing
        );

        let unsupported = br#"{"version":99,"pages":[],"searches":[]}"#;
        fs::write(&path, unsupported).expect("writes an unsupported format");
        assert_eq!(
            profile.0.load_history_outcome(),
            ProfileLoadOutcome::Unsupported
        );
        assert!(!path.exists());
        let backup = fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("History.json.damaged-")
            })
            .expect("preserves the unsupported history file");
        assert_eq!(fs::read(backup.path()).unwrap(), unsupported);
    }

    #[test]
    fn preserves_source_data_when_history_is_partially_recovered() {
        let profile = TemporaryProfile::new("partial-history");
        let contents = br#"{"version":1,"pages":[{"url":"https://ok.example/","title":"OK","visit_count":1,"last_visit":1},{"url":7}],"searches":[]}"#;
        let path = profile.0.root().join(HISTORY_FILE);
        fs::write(&path, contents).expect("writes partially damaged history");

        let ProfileLoadOutcome::PartiallyRecovered(history) = profile.0.load_history_outcome()
        else {
            panic!("recovers the valid history row");
        };
        assert_eq!(history.pages().len(), 1);
        assert!(!path.exists());
        let backup = fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("History.json.damaged-")
            })
            .expect("preserves the original history file");
        assert_eq!(fs::read(backup.path()).unwrap(), contents);
    }

    #[test]
    fn refuses_to_replace_damaged_data_when_backup_cannot_be_made() {
        let profile = TemporaryProfile::new("blocked-damaged-session");
        let path = profile.0.root().join(SESSION_FILE);
        let contents = b"damaged session";
        fs::write(&path, contents).expect("writes invalid session");
        profile
            .0
            .writes
            .session_write_blocked
            .store(true, Ordering::Release);
        profile
            .0
            .writes
            .session_source_checked
            .store(true, Ordering::Release);

        assert!(profile.0.save_session(&Session::default()).is_err());
        assert_eq!(fs::read(path).unwrap(), contents);
    }

    #[test]
    fn saves_and_loads_settings() {
        let profile = TemporaryProfile::new("settings");
        assert_eq!(profile.0.load_settings(), BrowserSettings::default());
        let mut settings = BrowserSettings {
            theme: photon_core::ThemeMode::Dark,
            ..BrowserSettings::default()
        };
        settings.new_tab.remove("https://example.com/");
        let layout = settings.workspace.default_preset().unwrap().layout.clone();
        settings
            .workspace
            .presets
            .push(photon_core::WorkspacePreset {
                id: "research".to_owned(),
                name: "Research".to_owned(),
                layout,
            });
        let space_id = settings.spaces.add(
            Some("Work".to_owned()),
            photon_core::WindowColor::Blue,
            false,
        );
        assert!(
            settings
                .spaces
                .set_layout_override(space_id, &photon_core::WorkspaceLayout::vertical())
        );
        profile.0.save_settings(&settings).expect("saves");
        assert_eq!(profile.0.settings_path().extension().unwrap(), "toml");
        assert!(
            fs::read_to_string(profile.0.settings_path())
                .unwrap()
                .contains("theme = \"dark\"")
        );
        assert_eq!(profile.0.load_settings(), settings);
    }

    #[test]
    fn opening_a_profile_migrates_older_settings_automatically() {
        let root = TemporaryProfile::root("automatic-legacy-migration");
        fs::create_dir_all(&root).expect("creates profile folder");
        fs::write(root.join("Settings.json"), br#"{"theme":"dark"}"#).expect("writes old settings");

        let profile = Profile::open(&root).expect("opens and migrates profile");

        assert_eq!(profile.load_settings().theme, photon_core::ThemeMode::Dark);
        assert!(profile.settings_path().exists());
        assert!(!root.join("Settings.json").exists());
        assert!(
            fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Settings.json.migrated-"))
        );
        let reopened = Profile::open(&root).expect("reopens migrated profile");
        assert_eq!(reopened.load_settings().theme, photon_core::ThemeMode::Dark);
        let backup_count = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("Settings.json.migrated-")
            })
            .count();
        assert_eq!(backup_count, 1);
        fs::remove_dir_all(root).expect("removes test profile");
    }

    #[test]
    fn opening_a_profile_migrates_settings_next_to_its_profile() {
        let base = TemporaryProfile::root("external-settings-migration");
        let root = base.join("Default");
        let settings_path = base.join("config.toml");
        let legacy_path = base.join("config.json");
        fs::create_dir_all(&base).expect("creates settings folder");
        fs::write(&legacy_path, br#"{"theme":"dark"}"#).expect("writes old settings");

        let profile = Profile::open_with_settings_path(&root, Some(settings_path.clone()))
            .expect("opens and migrates profile");

        assert_eq!(profile.load_settings().theme, photon_core::ThemeMode::Dark);
        assert!(settings_path.exists());
        assert!(!legacy_path.exists());
        assert!(
            fs::read_dir(&base)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.json.migrated-"))
        );
        fs::remove_dir_all(base).expect("removes test profile");
    }

    #[test]
    fn older_profile_snapshots_cannot_overwrite_newer_saves() {
        let temporary = TemporaryProfile::new("latest-snapshots");
        let profile = temporary.0.clone();
        let clone = profile.clone();

        let older_settings_ticket = profile.begin_settings_write();
        let newer_settings_ticket = clone.begin_settings_write();
        let older_settings = BrowserSettings {
            theme: photon_core::ThemeMode::Light,
            ..BrowserSettings::default()
        };
        let newer_settings = BrowserSettings {
            theme: photon_core::ThemeMode::Dark,
            ..BrowserSettings::default()
        };
        assert!(
            clone
                .save_settings_if_current(newer_settings_ticket, &newer_settings)
                .expect("saves current settings")
        );
        assert!(
            !profile
                .save_settings_if_current(older_settings_ticket, &older_settings)
                .expect("skips stale settings")
        );
        assert_eq!(profile.load_settings(), newer_settings);

        let mut older_history = History::default();
        older_history.record_visit("https://old.example/", "Old", 1);
        let mut newer_history = History::default();
        newer_history.record_visit("https://new.example/", "New", 2);
        let older_history_ticket = profile.begin_history_write();
        let newer_history_ticket = clone.begin_history_write();
        assert!(
            clone
                .save_history_if_current(newer_history_ticket, &newer_history)
                .expect("saves current history")
        );
        assert!(
            !profile
                .save_history_if_current(older_history_ticket, &older_history)
                .expect("skips stale history")
        );
        assert_eq!(profile.load_history(), newer_history);

        let older_session = Session {
            windows: vec![photon_core::WindowSession {
                tabs: vec![photon_core::SessionTab {
                    url: "https://old.example/".to_owned(),
                    title: "Old".to_owned(),
                    space: 1,
                }],
                active: Some(0),
            }],
        };
        let newer_session = Session {
            windows: vec![photon_core::WindowSession {
                tabs: vec![photon_core::SessionTab {
                    url: "https://new.example/".to_owned(),
                    title: "New".to_owned(),
                    space: 1,
                }],
                active: Some(0),
            }],
        };
        let older_session_ticket = profile.begin_session_write();
        let newer_session_ticket = clone.begin_session_write();
        assert!(
            clone
                .save_session_if_current(newer_session_ticket, &newer_session)
                .expect("saves current session")
        );
        assert!(
            !profile
                .save_session_if_current(older_session_ticket, &older_session)
                .expect("skips stale session")
        );
        assert_eq!(profile.load_session(), newer_session);
    }

    #[test]
    fn overlapping_history_saves_leave_the_newest_reserved_snapshot() {
        let temporary = TemporaryProfile::new("overlapping-history-saves");
        let profile = temporary.0.clone();
        let older_profile = profile.clone();
        let newer_profile = profile.clone();
        let older_ticket = older_profile.begin_history_write();
        let newer_ticket = newer_profile.begin_history_write();
        let mut older_history = History::default();
        older_history.record_visit("https://old.example/", "Old", 1);
        let mut newer_history = History::default();
        newer_history.record_visit("https://new.example/", "New", 2);
        let expected_history = newer_history.clone();
        let start = Arc::new(std::sync::Barrier::new(3));

        let old_start = start.clone();
        let old_save = std::thread::spawn(move || {
            old_start.wait();
            older_profile.save_history_if_current(older_ticket, &older_history)
        });
        let new_start = start.clone();
        let new_save = std::thread::spawn(move || {
            new_start.wait();
            newer_profile.save_history_if_current(newer_ticket, &newer_history)
        });

        start.wait();
        old_save
            .join()
            .expect("older save thread completes")
            .expect("older save succeeds or is superseded");
        new_save
            .join()
            .expect("newer save thread completes")
            .expect("newer save succeeds or is superseded");
        assert_eq!(profile.load_history(), expected_history);
    }

    #[test]
    fn migrates_a_legacy_vertical_layout_to_its_matching_preset() {
        let profile = TemporaryProfile::unmigrated("legacy-vertical-layout");
        migrate_legacy_json(&profile, br#"{"theme":"dark","tab_layout":"vertical"}"#);

        let settings = profile.0.load_settings();
        assert_eq!(settings.theme, photon_core::ThemeMode::Dark);
        assert_eq!(settings.tab_layout, photon_core::TabLayout::Vertical);
        assert_eq!(
            settings.workspace.default_preset_id,
            photon_core::VERTICAL_PRESET_ID
        );
        profile
            .0
            .save_settings(&settings)
            .expect("saves migrated settings");
        assert_eq!(profile.0.load_settings(), settings);
    }

    #[test]
    fn retains_legacy_workspace_presets_for_shell_expansion() {
        let profile = TemporaryProfile::unmigrated("legacy-workspace-presets");
        migrate_legacy_json(
            &profile,
            br#"{
                "tab_layout":"vertical",
                "workspace":{
                    "default_preset_id":"vertical",
                    "presets":[
                        {"id":"horizontal","name":"Horizontal","layout":{"components":[
                            {"id":"tab-strip","region":"top","order":0,"visible":true},
                            {"id":"address-toolbar","region":"top","order":1,"visible":true},
                            {"id":"sidebar","region":"leading","order":0,"visible":false}
                        ]}},
                        {"id":"vertical","name":"Vertical","layout":{"components":[
                            {"id":"tab-strip","region":"top","order":0,"visible":false},
                            {"id":"address-toolbar","region":"top","order":1,"visible":false},
                            {"id":"sidebar","region":"leading","order":0,"visible":true}
                        ]}}
                    ]
                }
            }"#,
        );

        let settings = profile.0.load_settings();
        assert_eq!(
            settings.workspace.default_preset_id,
            photon_core::VERTICAL_PRESET_ID
        );
        assert!(settings.workspace.is_valid());
        assert!(
            settings
                .workspace
                .default_preset()
                .unwrap()
                .layout
                .component(photon_core::SIDEBAR_COMPONENT_ID)
                .unwrap()
                .visible
        );
    }

    #[test]
    fn invalid_workspace_data_falls_back_without_losing_other_settings() {
        let profile = TemporaryProfile::unmigrated("invalid-workspace");
        migrate_legacy_json(
            &profile,
            br#"{"theme":"dark","tab_layout":"vertical","workspace":{"default_preset_id":"missing","presets":[]}}"#,
        );

        let settings = profile.0.load_settings();
        assert_eq!(settings.theme, photon_core::ThemeMode::Dark);
        assert_eq!(
            settings.workspace.default_preset_id,
            photon_core::VERTICAL_PRESET_ID
        );
    }

    #[test]
    fn incomplete_workspace_data_migrates_the_legacy_layout() {
        let profile = TemporaryProfile::unmigrated("incomplete-workspace");
        migrate_legacy_json(
            &profile,
            br#"{"theme":"dark","tab_layout":"vertical","workspace":{"presets":[]}}"#,
        );

        let settings = profile.0.load_settings();
        assert_eq!(settings.theme, photon_core::ThemeMode::Dark);
        assert_eq!(
            settings.workspace.default_preset_id,
            photon_core::VERTICAL_PRESET_ID
        );
    }

    #[test]
    fn invalid_space_layout_falls_back_to_the_profile_default() {
        let profile = TemporaryProfile::new("invalid-space-layout");
        let mut settings = BrowserSettings::default();
        let space_id = settings.spaces.active().id;
        let mut invalid = photon_core::WorkspaceLayout::vertical();
        invalid.components[0].order = 1;
        settings.spaces.get_mut(space_id).unwrap().layout_override = Some(invalid);
        profile.0.save_settings(&settings).expect("saves settings");

        let loaded = profile.0.load_settings();
        assert_eq!(loaded.spaces.get(space_id).unwrap().layout_override, None);
        assert_eq!(
            loaded.workspace.default_preset_id,
            photon_core::VERTICAL_PRESET_ID
        );
    }

    #[test]
    fn ignores_an_unreadable_history() {
        let profile = TemporaryProfile::new("corrupt");
        let path = profile.0.root().join(HISTORY_FILE);
        let contents = b"{ not json";
        fs::write(&path, contents).expect("writes");
        assert_eq!(
            profile.0.load_history_outcome(),
            ProfileLoadOutcome::Corrupt
        );
        assert!(!path.exists());
        let backup = fs::read_dir(profile.0.root())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("History.json.damaged-")
            })
            .expect("preserves the unreadable history");
        assert_eq!(fs::read(backup.path()).unwrap(), contents);
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
