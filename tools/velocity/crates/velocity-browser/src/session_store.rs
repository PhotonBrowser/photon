use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::lifecycle::ManagedSession;

/// Persistence interface used by the platform-independent lifecycle manager.
pub trait SessionStore {
    fn lock(&self) -> Result<Box<dyn SessionStoreGuard>, SessionStoreError> {
        Ok(Box::new(NoopSessionStoreGuard))
    }

    fn load(&self) -> Result<Vec<ManagedSession>, SessionStoreError>;
    fn save(&self, sessions: &[ManagedSession]) -> Result<(), SessionStoreError>;
}

/// RAII lock held while lifecycle state is read, changed, and persisted.
pub trait SessionStoreGuard {}

struct NoopSessionStoreGuard;

impl SessionStoreGuard for NoopSessionStoreGuard {}

/// Session registry stored in the current user's Velocity application support directory.
#[derive(Clone, Debug)]
pub struct FileSessionStore {
    path: PathBuf,
}

impl FileSessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SessionStore for FileSessionStore {
    fn lock(&self) -> Result<Box<dyn SessionStoreGuard>, SessionStoreError> {
        let directory = self
            .path
            .parent()
            .ok_or_else(|| SessionStoreError("session file has no parent directory".to_owned()))?;
        fs::create_dir_all(directory).map_err(|error| SessionStoreError::at(directory, error))?;
        set_private_permissions(directory, 0o700)?;
        let lock_path = self.path.with_extension("json.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| SessionStoreError::at(&lock_path, error))?;
        set_private_permissions(&lock_path, 0o600)?;
        lock_file(&file).map_err(|error| SessionStoreError::at(&lock_path, error))?;
        Ok(Box::new(FileSessionStoreGuard { _file: file }))
    }

    fn load(&self) -> Result<Vec<ManagedSession>, SessionStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(SessionStoreError::at(&self.path, error)),
        };
        serde_json::from_slice(&bytes).map_err(|error| SessionStoreError::at(&self.path, error))
    }

    fn save(&self, sessions: &[ManagedSession]) -> Result<(), SessionStoreError> {
        let directory = self
            .path
            .parent()
            .ok_or_else(|| SessionStoreError("session file has no parent directory".to_owned()))?;
        fs::create_dir_all(directory).map_err(|error| SessionStoreError::at(directory, error))?;
        set_private_permissions(directory, 0o700)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = self
            .path
            .with_extension(format!("json.{}.{timestamp}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| SessionStoreError::at(&temporary, error))?;
        if let Err(error) = set_private_permissions(&temporary, 0o600) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        let result = write_sessions(&mut file, sessions)
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temporary, &self.path));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(SessionStoreError::at(&self.path, error));
        }
        Ok(())
    }
}

struct FileSessionStoreGuard {
    _file: File,
}

impl SessionStoreGuard for FileSessionStoreGuard {}

#[cfg(target_os = "macos")]
fn lock_file(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    loop {
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn lock_file(_file: &File) -> io::Result<()> {
    Ok(())
}

fn write_sessions(file: &mut File, sessions: &[ManagedSession]) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *file, sessions).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

#[cfg(unix)]
fn set_private_permissions(path: &Path, mode: u32) -> Result<(), SessionStoreError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| SessionStoreError::at(path, error))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path, _mode: u32) -> Result<(), SessionStoreError> {
    Ok(())
}

/// Error returned when the managed session file cannot be read or written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStoreError(pub String);

impl SessionStoreError {
    fn at(path: &Path, error: impl std::fmt::Display) -> Self {
        Self(format!("{}: {error}", path.display()))
    }
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SessionStoreError {}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::*;
    use crate::{ManagedSession, ProcessIdentity};

    #[test]
    fn missing_store_is_empty_and_saved_sessions_round_trip() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velocity-store-{unique}"));
        let store = FileSessionStore::new(directory.join("sessions.json"));
        assert!(store.load().unwrap().is_empty());

        let session = ManagedSession {
            session_id: "session-1-2-3".to_owned(),
            browser_id: "chrome".to_owned(),
            installation_id: "chrome-test".to_owned(),
            browser_name: "Chrome".to_owned(),
            version: None,
            application_path: PathBuf::from("/fixture/Chrome.app"),
            executable_path: PathBuf::from("/fixture/Chrome"),
            identity: ProcessIdentity {
                pid: 123,
                session_id: 123,
                started_at: "fixture".to_owned(),
                start_time: None,
                executable_path: PathBuf::from("/fixture/Chrome"),
                profile_path: Some(PathBuf::from("/fixture/Profile")),
            },
        };
        {
            let _lock = store.lock().unwrap();
            store.save(std::slice::from_ref(&session)).unwrap();
        }
        assert_eq!(store.load().unwrap(), vec![session]);
        fs::remove_dir_all(directory).unwrap();
    }
}
