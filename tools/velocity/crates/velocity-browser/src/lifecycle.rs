use std::{
    fmt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{BrowserInstallation, IdentifierError, SessionStore, SessionStoreError};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Identity recorded for a process session created by Velocity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub session_id: u32,
    pub started_at: String,
    #[serde(default)]
    pub start_time: Option<velocity_metrics::ProcessStartTime>,
    pub executable_path: PathBuf,
    pub profile_path: Option<PathBuf>,
}

/// One browser launch recorded across separate Velocity CLI invocations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManagedSession {
    pub session_id: String,
    pub browser_id: String,
    pub installation_id: String,
    pub browser_name: String,
    pub version: Option<String>,
    pub application_path: PathBuf,
    pub executable_path: PathBuf,
    pub identity: ProcessIdentity,
}

impl ManagedSession {
    fn new(
        session_id: String,
        installation: &BrowserInstallation,
        identity: ProcessIdentity,
    ) -> Self {
        Self {
            session_id,
            browser_id: installation.metadata.id.to_string(),
            installation_id: installation.installation_id.to_string(),
            browser_name: installation.metadata.name.clone(),
            version: installation.version.clone(),
            application_path: installation.application_path.clone(),
            executable_path: installation.executable_path.clone(),
            identity,
        }
    }
}

/// Current state reported by the platform-specific process driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessState {
    Running,
    Exited,
    OwnershipUncertain,
}

/// Timing and ownership identifiers captured when the operating system creates
/// a browser process.
#[derive(Clone, Debug)]
pub struct ProcessLaunchTiming {
    pub pid: u32,
    pub session_id: u32,
    pub executable_path: PathBuf,
    pub launch_started_at: Instant,
    pub process_started_at: Instant,
}

/// Platform-specific process operations. Implementations must refuse termination
/// when they cannot verify that the process belongs to Velocity.
pub trait ProcessDriver {
    fn launch(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
        url: Option<&str>,
    ) -> Result<ProcessIdentity, ProcessError>;

    /// Starts a process and reports the OS-confirmed spawn time as soon as it
    /// is available. Drivers should override this to bracket the actual spawn
    /// call; the default is suitable for simple test and future adapters.
    fn launch_observed(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
        url: Option<&str>,
        on_started: &mut dyn FnMut(ProcessLaunchTiming),
    ) -> Result<ProcessIdentity, ProcessError> {
        let launch_started_at = Instant::now();
        let identity = self.launch(installation, session_id, url)?;
        on_started(ProcessLaunchTiming {
            pid: identity.pid,
            session_id: identity.session_id,
            executable_path: identity.executable_path.clone(),
            launch_started_at,
            process_started_at: Instant::now(),
        });
        Ok(identity)
    }

    fn state(&self, identity: &ProcessIdentity) -> Result<ProcessState, ProcessError>;

    fn terminate(&self, identity: &ProcessIdentity) -> Result<(), ProcessError>;

    fn cleanup(&self, _session_id: &str, _identity: &ProcessIdentity) -> Result<(), ProcessError> {
        Ok(())
    }
}

/// Error returned by a platform process operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessError(pub String);

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProcessError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseOutcome {
    Closed(ManagedSession),
    AlreadyExited(ManagedSession),
}

/// Coordinates browser launches and persisted ownership records.
pub struct BrowserLifecycleManager<D, S> {
    driver: D,
    store: S,
}

impl<D: ProcessDriver, S: SessionStore> BrowserLifecycleManager<D, S> {
    pub fn new(driver: D, store: S) -> Self {
        Self { driver, store }
    }

    pub fn launch(
        &self,
        installation: &BrowserInstallation,
        url: Option<&str>,
    ) -> Result<ManagedSession, LifecycleError> {
        self.launch_observed(installation, url, &mut |_| {})
    }

    /// Launches a browser and reports its OS-confirmed spawn while the
    /// lifecycle manager continues to establish and persist ownership.
    pub fn launch_observed(
        &self,
        installation: &BrowserInstallation,
        url: Option<&str>,
        on_started: &mut dyn FnMut(ProcessLaunchTiming),
    ) -> Result<ManagedSession, LifecycleError> {
        let _store_lock = self.store.lock().map_err(LifecycleError::SessionStore)?;
        let mut sessions = self.store.load().map_err(LifecycleError::SessionStore)?;
        self.remove_exited(&mut sessions)?;

        let session_id = new_session_id();
        let identity = self
            .driver
            .launch_observed(installation, &session_id, url, on_started)
            .map_err(LifecycleError::Process)?;
        let session = ManagedSession::new(session_id, installation, identity);
        sessions.push(session.clone());

        if let Err(error) = self.store.save(&sessions) {
            let cleanup = self
                .driver
                .terminate(&session.identity)
                .and_then(|()| self.driver.cleanup(&session.session_id, &session.identity));
            let detail = match cleanup {
                Ok(()) => format!("could not record launched browser; it was stopped: {error}"),
                Err(cleanup_error) => format!(
                    "could not record launched browser ({error}) and cleanup failed ({cleanup_error})"
                ),
            };
            return Err(LifecycleError::SessionStore(SessionStoreError(detail)));
        }

        Ok(session)
    }

    pub fn close(&self, selector: &str) -> Result<CloseOutcome, LifecycleError> {
        let _store_lock = self.store.lock().map_err(LifecycleError::SessionStore)?;
        let mut sessions = self.store.load().map_err(LifecycleError::SessionStore)?;
        let matching = sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                session.session_id == selector
                    || session.browser_id == selector
                    || session.installation_id == selector
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        let index = match matching.as_slice() {
            [] => {
                validate_selector(selector).map_err(LifecycleError::InvalidIdentifier)?;
                return Err(LifecycleError::NotManaged(selector.to_owned()));
            }
            [index] => *index,
            _ => {
                return Err(LifecycleError::AmbiguousSessions(
                    matching
                        .iter()
                        .map(|index| sessions[*index].session_id.clone())
                        .collect(),
                ));
            }
        };

        let session = sessions[index].clone();
        let outcome = match self
            .driver
            .state(&session.identity)
            .map_err(LifecycleError::Process)?
        {
            ProcessState::Running => {
                self.driver
                    .terminate(&session.identity)
                    .map_err(LifecycleError::Process)?;
                self.driver
                    .cleanup(&session.session_id, &session.identity)
                    .map_err(LifecycleError::Process)?;
                CloseOutcome::Closed(session)
            }
            ProcessState::Exited => {
                self.driver
                    .cleanup(&session.session_id, &session.identity)
                    .map_err(LifecycleError::Process)?;
                CloseOutcome::AlreadyExited(session)
            }
            ProcessState::OwnershipUncertain => {
                return Err(LifecycleError::OwnershipUncertain {
                    session_id: session.session_id,
                    pid: session.identity.pid,
                });
            }
        };

        sessions.remove(index);
        self.store
            .save(&sessions)
            .map_err(LifecycleError::SessionStore)?;
        Ok(outcome)
    }

    pub fn managed_sessions(&self) -> Result<Vec<ManagedSession>, LifecycleError> {
        let _store_lock = self.store.lock().map_err(LifecycleError::SessionStore)?;
        let mut sessions = self.store.load().map_err(LifecycleError::SessionStore)?;
        self.remove_exited(&mut sessions)?;
        Ok(sessions)
    }

    fn remove_exited(&self, sessions: &mut Vec<ManagedSession>) -> Result<(), LifecycleError> {
        let original_len = sessions.len();
        let mut retained = Vec::with_capacity(sessions.len());
        for session in sessions.drain(..) {
            match self
                .driver
                .state(&session.identity)
                .map_err(LifecycleError::Process)?
            {
                ProcessState::Exited => self
                    .driver
                    .cleanup(&session.session_id, &session.identity)
                    .map_err(LifecycleError::Process)?,
                ProcessState::Running | ProcessState::OwnershipUncertain => retained.push(session),
            }
        }
        if retained.len() != original_len {
            self.store
                .save(&retained)
                .map_err(LifecycleError::SessionStore)?;
        }
        *sessions = retained;
        Ok(())
    }
}

fn validate_selector(selector: &str) -> Result<(), IdentifierError> {
    let is_session_id = selector.strip_prefix("session-").is_some_and(|rest| {
        rest.split('-').count() == 3
            && rest
                .split('-')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    });
    if is_session_id {
        return Ok(());
    }
    crate::BrowserId::new(selector.to_owned()).map(|_| ())
}

fn new_session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("session-{}-{timestamp}-{counter}", std::process::id())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    InvalidIdentifier(IdentifierError),
    NotManaged(String),
    AmbiguousSessions(Vec<String>),
    OwnershipUncertain { session_id: String, pid: u32 },
    Process(ProcessError),
    SessionStore(SessionStoreError),
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(error) => error.fmt(formatter),
            Self::NotManaged(selector) => write!(
                formatter,
                "no Velocity-managed instance matches '{selector}'"
            ),
            Self::AmbiguousSessions(sessions) => write!(
                formatter,
                "multiple managed instances match; close one by session ID: {}",
                sessions.join(", ")
            ),
            Self::OwnershipUncertain { session_id, pid } => write!(
                formatter,
                "cannot verify ownership of session {session_id} (process {pid}); refusing to terminate it"
            ),
            Self::Process(error) => error.fmt(formatter),
            Self::SessionStore(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LifecycleError {}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use super::*;
    use crate::{BrowserEngine, BrowserId, BrowserMetadata, InstallationId};

    #[derive(Clone, Default)]
    struct MemoryStore(Arc<Mutex<Vec<ManagedSession>>>);

    impl SessionStore for MemoryStore {
        fn load(&self) -> Result<Vec<ManagedSession>, SessionStoreError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&self, sessions: &[ManagedSession]) -> Result<(), SessionStoreError> {
            *self.0.lock().unwrap() = sessions.to_vec();
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct MockDriver {
        next_pid: Arc<Mutex<u32>>,
        states: Arc<Mutex<HashMap<u32, ProcessState>>>,
        launches: Arc<Mutex<Vec<LaunchRecord>>>,
    }

    type LaunchRecord = (String, Option<String>);

    impl ProcessDriver for MockDriver {
        fn launch(
            &self,
            installation: &BrowserInstallation,
            session_id: &str,
            url: Option<&str>,
        ) -> Result<ProcessIdentity, ProcessError> {
            let mut next_pid = self.next_pid.lock().unwrap();
            *next_pid = next_pid.saturating_add(1).max(1000);
            let pid = *next_pid;
            self.states
                .lock()
                .unwrap()
                .insert(pid, ProcessState::Running);
            self.launches
                .lock()
                .unwrap()
                .push((session_id.to_owned(), url.map(str::to_owned)));
            Ok(ProcessIdentity {
                pid,
                session_id: pid,
                started_at: "fixture-start".to_owned(),
                start_time: None,
                executable_path: installation.executable_path.clone(),
                profile_path: None,
            })
        }

        fn state(&self, identity: &ProcessIdentity) -> Result<ProcessState, ProcessError> {
            Ok(self
                .states
                .lock()
                .unwrap()
                .get(&identity.pid)
                .copied()
                .unwrap_or(ProcessState::Exited))
        }

        fn terminate(&self, identity: &ProcessIdentity) -> Result<(), ProcessError> {
            self.states
                .lock()
                .unwrap()
                .insert(identity.pid, ProcessState::Exited);
            Ok(())
        }
    }

    fn fixture_installation() -> BrowserInstallation {
        let unique = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let executable_path = std::env::temp_dir().join(format!(
            "velocity-lifecycle-{}-{unique}",
            std::process::id()
        ));
        fs::write(&executable_path, b"fixture").unwrap();
        BrowserInstallation {
            installation_id: InstallationId::new("chrome-test").unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new("chrome").unwrap(),
                name: "Chrome fixture".to_owned(),
                bundle_identifier: None,
                engine: Some(BrowserEngine::Chromium),
            },
            version: None,
            application_path: PathBuf::new(),
            executable_path,
        }
    }

    #[test]
    fn launch_and_close_follow_managed_process_state() {
        let driver = MockDriver::default();
        let store = MemoryStore::default();
        let manager = BrowserLifecycleManager::new(driver.clone(), store.clone());
        let installation = fixture_installation();

        let launched = manager
            .launch(&installation, Some("https://example.com"))
            .unwrap();
        assert_eq!(manager.managed_sessions().unwrap(), vec![launched.clone()]);
        assert_eq!(
            manager.close("chrome").unwrap(),
            CloseOutcome::Closed(launched.clone())
        );
        assert!(manager.managed_sessions().unwrap().is_empty());
        assert_eq!(
            driver.launches.lock().unwrap()[0].1.as_deref(),
            Some("https://example.com")
        );

        fs::remove_file(installation.executable_path).unwrap();
    }

    #[test]
    fn detects_unexpected_exit_and_refuses_uncertain_ownership() {
        let driver = MockDriver::default();
        let store = MemoryStore::default();
        let manager = BrowserLifecycleManager::new(driver.clone(), store.clone());
        let installation = fixture_installation();
        let launched = manager.launch(&installation, None).unwrap();

        driver
            .states
            .lock()
            .unwrap()
            .insert(launched.identity.pid, ProcessState::Exited);
        assert_eq!(
            manager.close("chrome-test").unwrap(),
            CloseOutcome::AlreadyExited(launched.clone())
        );

        let uncertain = manager.launch(&installation, None).unwrap();
        driver
            .states
            .lock()
            .unwrap()
            .insert(uncertain.identity.pid, ProcessState::OwnershipUncertain);
        assert!(matches!(
            manager.close("chrome"),
            Err(LifecycleError::OwnershipUncertain { .. })
        ));
        assert_eq!(manager.managed_sessions().unwrap(), vec![uncertain]);

        fs::remove_file(installation.executable_path).unwrap();
    }

    #[test]
    fn multiple_sessions_require_an_explicit_session_selector() {
        let manager = BrowserLifecycleManager::new(MockDriver::default(), MemoryStore::default());
        let installation = fixture_installation();
        let first = manager.launch(&installation, None).unwrap();
        let second = manager.launch(&installation, None).unwrap();
        assert!(matches!(
            manager.close("chrome"),
            Err(LifecycleError::AmbiguousSessions(_))
        ));
        manager.close(&first.session_id).unwrap();
        manager.close(&second.session_id).unwrap();
        fs::remove_file(installation.executable_path).unwrap();
    }
}
