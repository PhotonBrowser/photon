use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::process_info::process_start_time;
use crate::{
    BrowserEngine, BrowserInstallation, ProcessDriver, ProcessError, ProcessIdentity,
    ProcessLaunchTiming, ProcessState,
};

const STARTUP_CHECK_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_CHECKS: usize = 6;
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const FORCE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

/// macOS process launcher and owner for Velocity-created browser sessions.
#[derive(Clone)]
pub struct MacOSProcessDriver {
    profile_root: PathBuf,
    profile_override: Option<PathBuf>,
    retain_profile: bool,
    chromium_devtools: bool,
    photon_automation: Option<(PathBuf, String)>,
}

impl std::fmt::Debug for MacOSProcessDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MacOSProcessDriver")
            .field("profile_root", &self.profile_root)
            .field("profile_override", &self.profile_override)
            .field("retain_profile", &self.retain_profile)
            .field("chromium_devtools", &self.chromium_devtools)
            .field(
                "photon_automation",
                &self.photon_automation.as_ref().map(|(path, _)| path),
            )
            .finish()
    }
}

impl MacOSProcessDriver {
    pub fn new(profile_root: PathBuf) -> Self {
        Self {
            profile_root,
            profile_override: None,
            retain_profile: false,
            chromium_devtools: false,
            photon_automation: None,
        }
    }

    /// Uses a caller-owned, isolated profile for benchmark iterations.
    pub fn with_benchmark_profile(mut self, profile_path: PathBuf, retain_profile: bool) -> Self {
        self.profile_override = Some(profile_path);
        self.retain_profile = retain_profile;
        self
    }

    /// Enables a loopback-only Chromium DevTools endpoint for an isolated
    /// benchmark profile. The endpoint's port is selected by Chromium.
    pub fn with_chromium_devtools(mut self) -> Self {
        self.chromium_devtools = true;
        self
    }

    /// Enables Photon automation with a caller-generated session token.
    /// The token is written to the private profile and is never passed as an
    /// application argument or included in this driver's Debug output.
    pub fn with_photon_automation(mut self, socket_path: PathBuf, token: String) -> Self {
        self.photon_automation = Some((socket_path, token));
        self
    }
}

impl ProcessDriver for MacOSProcessDriver {
    fn launch(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
        url: Option<&str>,
    ) -> Result<ProcessIdentity, ProcessError> {
        self.launch_inner(installation, session_id, url, &mut |_| {})
    }

    fn launch_observed(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
        url: Option<&str>,
        on_started: &mut dyn FnMut(ProcessLaunchTiming),
    ) -> Result<ProcessIdentity, ProcessError> {
        self.launch_inner(installation, session_id, url, on_started)
    }

    fn state(&self, identity: &ProcessIdentity) -> Result<ProcessState, ProcessError> {
        let rows = session_processes(identity.session_id)?;
        if rows.is_empty() {
            return Ok(ProcessState::Exited);
        }

        // The session ID is created by the exact process verified at launch.
        // Descendants cannot be joined by processes from another session.
        // If the original PID now leads a different process in this session,
        // treat the persisted identity as stale rather than signal that session.
        if let Some(leader) = process_row(identity.pid)? {
            if is_recorded_leader(&leader, identity) {
                return Ok(ProcessState::Running);
            }
            if leader.session_id == identity.session_id {
                return Ok(ProcessState::OwnershipUncertain);
            }
        }
        Ok(ProcessState::Running)
    }

    fn terminate(&self, identity: &ProcessIdentity) -> Result<(), ProcessError> {
        match self.state(identity)? {
            ProcessState::Exited => return Ok(()),
            ProcessState::OwnershipUncertain => {
                return Err(ProcessError(format!(
                    "cannot verify ownership of browser process {}; refusing to terminate it",
                    identity.pid
                )));
            }
            ProcessState::Running => {}
        }

        self.signal_session(identity.session_id, libc::SIGTERM)?;
        if self.wait_for_session_exit(identity.session_id, GRACEFUL_SHUTDOWN_TIMEOUT)? {
            return Ok(());
        }

        // The session ID is private to this launched process tree. Re-enumerate
        // it before escalation so only members of the managed session are signalled.
        self.signal_session(identity.session_id, libc::SIGKILL)?;
        if self.wait_for_session_exit(identity.session_id, FORCE_SHUTDOWN_TIMEOUT)? {
            Ok(())
        } else {
            Err(ProcessError(format!(
                "browser session {} did not exit after SIGKILL",
                identity.session_id
            )))
        }
    }

    fn cleanup(&self, session_id: &str, identity: &ProcessIdentity) -> Result<(), ProcessError> {
        let Some(profile_path) = identity.profile_path.as_deref() else {
            return Ok(());
        };
        let session_profile = self.profile_root.join(session_id);
        let benchmark_profile = self
            .profile_override
            .as_deref()
            .is_some_and(|override_path| override_path == profile_path);
        if profile_path != session_profile && !benchmark_profile {
            return Err(ProcessError(format!(
                "refusing to remove profile outside the Velocity profile directory: {}",
                profile_path.display()
            )));
        }
        if benchmark_profile && self.retain_profile {
            return Ok(());
        }
        match fs::remove_dir_all(profile_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ProcessError(format!(
                "cannot remove managed browser profile {}: {error}",
                profile_path.display()
            ))),
        }
    }
}

impl MacOSProcessDriver {
    fn launch_inner(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
        url: Option<&str>,
        on_started: &mut dyn FnMut(ProcessLaunchTiming),
    ) -> Result<ProcessIdentity, ProcessError> {
        if self.chromium_devtools && installation.metadata.engine != Some(BrowserEngine::Chromium) {
            return Err(ProcessError(
                "the DevTools endpoint can only be enabled for an isolated Chromium browser"
                    .to_owned(),
            ));
        }
        let profile_path = self.create_profile(installation, session_id)?;
        if self.chromium_devtools && profile_path.is_none() {
            return Err(ProcessError(
                "refusing to expose a DevTools endpoint without an isolated profile".to_owned(),
            ));
        }
        let mut command = self.command(installation, profile_path.as_deref(), session_id, url)?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        use std::os::unix::process::CommandExt;
        // A new session gives this launch a private process group. Its children
        // inherit that ownership boundary, so shutdown never targets by app name.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }

        let launch_started_at = Instant::now();
        let mut child = command.spawn().map_err(|error| {
            self.remove_profile(profile_path.as_deref());
            ProcessError(format!(
                "cannot launch {}: {error}",
                installation.executable_path.display()
            ))
        })?;
        let process_started_at = Instant::now();
        on_started(ProcessLaunchTiming {
            pid: child.id(),
            session_id: child.id(),
            executable_path: installation.executable_path.clone(),
            launch_started_at,
            process_started_at,
        });

        match self.identify_started_process(&mut child, &installation.executable_path) {
            Ok((pid, started_at)) => Ok(ProcessIdentity {
                pid,
                session_id: pid,
                started_at,
                start_time: process_start_time(pid),
                executable_path: installation.executable_path.clone(),
                profile_path,
            }),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                if session_processes(child.id()).is_ok_and(|rows| rows.is_empty()) {
                    self.remove_profile(profile_path.as_deref());
                }
                Err(error)
            }
        }
    }

    fn create_profile(
        &self,
        installation: &BrowserInstallation,
        session_id: &str,
    ) -> Result<Option<PathBuf>, ProcessError> {
        if let Some(profile) = self.profile_override.as_ref() {
            if !matches!(
                installation.metadata.engine,
                Some(BrowserEngine::Chromium | BrowserEngine::Gecko | BrowserEngine::Ladybird)
            ) {
                return Err(ProcessError(format!(
                    "{} does not support an isolated Velocity profile",
                    installation.metadata.name
                )));
            }
            if !profile.starts_with(&self.profile_root) {
                return Err(ProcessError(
                    "benchmark profile must be inside Velocity's profile directory".to_owned(),
                ));
            }
            let parent = profile.parent().ok_or_else(|| {
                ProcessError("benchmark profile has no parent directory".to_owned())
            })?;
            fs::create_dir_all(parent).map_err(|error| {
                ProcessError(format!(
                    "cannot create Velocity profile directory {}: {error}",
                    parent.display()
                ))
            })?;
            set_private_directory_permissions(parent)?;
            fs::create_dir_all(profile).map_err(|error| {
                ProcessError(format!(
                    "cannot create isolated browser profile {}: {error}",
                    profile.display()
                ))
            })?;
            set_private_directory_permissions(profile)?;
            return Ok(Some(profile.clone()));
        }

        let supports_managed_profile = matches!(
            installation.metadata.engine,
            Some(BrowserEngine::Chromium | BrowserEngine::Gecko)
        ) || (installation.metadata.engine
            == Some(BrowserEngine::Ladybird)
            && self.photon_automation.is_some());
        if !supports_managed_profile {
            return Ok(None);
        }

        fs::create_dir_all(&self.profile_root).map_err(|error| {
            ProcessError(format!(
                "cannot create Velocity profile directory {}: {error}",
                self.profile_root.display()
            ))
        })?;
        set_private_directory_permissions(&self.profile_root)?;
        let profile = self.profile_root.join(session_id);
        fs::create_dir(&profile).map_err(|error| {
            ProcessError(format!(
                "cannot create isolated browser profile {}: {error}",
                profile.display()
            ))
        })?;
        set_private_directory_permissions(&profile)?;
        Ok(Some(profile))
    }

    fn command(
        &self,
        installation: &BrowserInstallation,
        profile_path: Option<&Path>,
        session_id: &str,
        url: Option<&str>,
    ) -> Result<Command, ProcessError> {
        let mut command = Command::new(&installation.executable_path);
        match installation.metadata.engine {
            Some(BrowserEngine::Chromium) => {
                if let Some(profile_path) = profile_path {
                    let mut argument = OsString::from("--user-data-dir=");
                    argument.push(profile_path.as_os_str());
                    command.arg(argument);
                }
                command.arg("--no-first-run").arg("--new-window");
                if self.chromium_devtools {
                    command
                        .arg("--remote-debugging-address=127.0.0.1")
                        .arg("--remote-debugging-port=0");
                }
                if let Some(url) = url {
                    command.arg(url);
                }
            }
            Some(BrowserEngine::Gecko) => {
                if let Some(profile_path) = profile_path {
                    command
                        .arg("--no-remote")
                        .arg("--profile")
                        .arg(profile_path);
                }
                if let Some(url) = url {
                    command.arg("--new-window").arg(url);
                }
            }
            Some(BrowserEngine::Ladybird) => {
                if let Some(profile_path) = profile_path {
                    let isolated_home = profile_path.join("Home");
                    fs::create_dir_all(&isolated_home).map_err(|error| {
                        ProcessError(format!(
                            "cannot create isolated Photon home {}: {error}",
                            isolated_home.display()
                        ))
                    })?;
                    set_private_directory_permissions(&isolated_home)?;
                    let data_profile = profile_path.join("Data");
                    fs::create_dir_all(&data_profile).map_err(|error| {
                        ProcessError(format!(
                            "cannot create isolated Photon data profile {}: {error}",
                            data_profile.display()
                        ))
                    })?;
                    set_private_directory_permissions(&data_profile)?;
                    command
                        .env("HOME", isolated_home)
                        .env("PHOTON_PROFILE_DIR", data_profile);
                }
                self.configure_photon(&mut command, installation, session_id, profile_path, url)?;
            }
            Some(BrowserEngine::WebKit) | None => {
                if let Some(url) = url {
                    command.arg(url);
                }
            }
        }
        Ok(command)
    }

    fn configure_photon(
        &self,
        command: &mut Command,
        installation: &BrowserInstallation,
        session_id: &str,
        profile_path: Option<&Path>,
        url: Option<&str>,
    ) -> Result<(), ProcessError> {
        let (root, helper_directory, service, engine_directory) =
            photon_runtime_paths(installation)?;
        command.current_dir(root);
        command.env("PHOTON_HELPER_DIRECTORY", &helper_directory);
        command.env("PHOTON_PRESENTATION_XPC_SERVICE", &service);
        command
            .env_remove("PHOTON_URL")
            .env_remove("PHOTON_SHUTDOWN_AFTER_SECONDS")
            .env_remove("PHOTON_TEMPORARY_PROFILE");
        command.env(
            "PHOTON_PRESENTATION_CHANNEL_ID",
            format!("velocity-{session_id}"),
        );
        let library_paths = [engine_directory.join("lib64"), engine_directory.join("lib")]
            .into_iter()
            .filter(|path| path.is_dir())
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if !library_paths.is_empty() {
            command.env("DYLD_LIBRARY_PATH", library_paths.join(":"));
        }
        if let Some(url) = url {
            command.env("PHOTON_URL", url);
        }
        if let Some((socket_path, token)) = &self.photon_automation {
            if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(ProcessError(
                    "Photon automation token must be a 256-bit hexadecimal value".to_owned(),
                ));
            }
            profile_path.ok_or_else(|| {
                ProcessError("Photon automation requires an isolated profile".to_owned())
            })?;
            let automation_directory = socket_path.parent().ok_or_else(|| {
                ProcessError("Photon automation socket has no parent directory".to_owned())
            })?;
            let expected_directory = format!("vlt-{}", &token[..12]);
            if automation_directory.parent() != Some(Path::new("/tmp"))
                || automation_directory
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some(expected_directory.as_str())
            {
                return Err(ProcessError(
                    "Photon automation socket must use its token-scoped private /tmp directory"
                        .to_owned(),
                ));
            }
            fs::create_dir(automation_directory).map_err(|error| {
                ProcessError(format!(
                    "cannot create Photon automation directory: {error}"
                ))
            })?;
            if let Err(error) = set_private_directory_permissions(automation_directory)
                .and_then(|()| write_private_token(&automation_directory.join("token"), token))
            {
                let _ = fs::remove_dir_all(automation_directory);
                return Err(error);
            }
            command.arg("--automation-socket").arg(socket_path);
        }
        Ok(())
    }

    fn identify_started_process(
        &self,
        child: &mut Child,
        executable_path: &Path,
    ) -> Result<(u32, String), ProcessError> {
        let pid = child.id();
        for _ in 0..STARTUP_CHECKS {
            if child
                .try_wait()
                .map_err(|error| ProcessError(format!("cannot inspect launched process: {error}")))?
                .is_some()
            {
                self.cleanup_unrecorded_session(pid)?;
                return Err(ProcessError(
                    "browser exited or handed the launch to another instance before Velocity could verify ownership; that instance was left untouched".to_owned(),
                ));
            }
            if let Some(row) = process_row(pid)? {
                if row.session_id != pid || row.process_group != pid {
                    return Err(ProcessError(format!(
                        "browser process {pid} reported session {} and process group {}; refusing to manage it",
                        row.session_id, row.process_group
                    )));
                }
                if !command_matches_executable(&row.command, executable_path) {
                    self.cleanup_unrecorded_session(pid)?;
                    return Err(ProcessError(format!(
                        "launched process {} does not match executable {}; it was left running",
                        pid,
                        executable_path.display()
                    )));
                }
                return Ok((pid, row.started_at));
            }
            thread::sleep(STARTUP_CHECK_INTERVAL);
        }
        self.cleanup_unrecorded_session(pid)?;
        Err(ProcessError(format!(
            "could not verify launched browser process {pid}; refusing to manage it"
        )))
    }

    fn cleanup_unrecorded_session(&self, session_id: u32) -> Result<(), ProcessError> {
        if session_processes(session_id)?.is_empty() {
            return Ok(());
        }
        self.signal_session(session_id, libc::SIGTERM)?;
        if self.wait_for_session_exit(session_id, GRACEFUL_SHUTDOWN_TIMEOUT)? {
            return Ok(());
        }
        self.signal_session(session_id, libc::SIGKILL)?;
        if self.wait_for_session_exit(session_id, FORCE_SHUTDOWN_TIMEOUT)? {
            Ok(())
        } else {
            Err(ProcessError(format!(
                "unrecorded process session {session_id} did not exit"
            )))
        }
    }

    fn signal_session(&self, session_id: u32, signal: i32) -> Result<(), ProcessError> {
        let rows = session_processes(session_id)?;
        let groups = rows
            .into_iter()
            .map(|row| row.process_group)
            .filter(|group| *group > 0)
            .collect::<BTreeSet<_>>();
        for group in groups {
            // The process group was observed inside the private session created
            // by this launch. Negative IDs signal only that process group.
            let result = unsafe { libc::kill(-(group as libc::pid_t), signal) };
            if result == -1 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(ProcessError(format!(
                        "cannot signal managed process group {group}: {error}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn wait_for_session_exit(
        &self,
        session_id: u32,
        timeout: Duration,
    ) -> Result<bool, ProcessError> {
        let deadline = Instant::now() + timeout;
        loop {
            if session_processes(session_id)?.is_empty() {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(STARTUP_CHECK_INTERVAL);
        }
    }

    fn remove_profile(&self, profile_path: Option<&Path>) {
        if !self.retain_profile
            && let Some(profile_path) = profile_path
        {
            let _ = fs::remove_dir_all(profile_path);
        }
    }
}

#[derive(Debug)]
struct ProcessRow {
    pid: u32,
    session_id: u32,
    process_group: u32,
    started_at: String,
    state: String,
    command: String,
}

fn process_row(pid: u32) -> Result<Option<ProcessRow>, ProcessError> {
    let output = Command::new("/bin/ps")
        .args([
            "-ww", "-o", "pgid=", "-o", "lstart=", "-o", "command=", "-p",
        ])
        .arg(pid.to_string())
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| ProcessError(format!("cannot inspect process {pid}: {error}")))?;
    let output = String::from_utf8_lossy(&output.stdout);
    Ok(parse_process_row(pid, &output))
}

fn session_processes(session_id: u32) -> Result<Vec<ProcessRow>, ProcessError> {
    let output = Command::new("/bin/ps")
        .args(["-ww", "-axo", "pgid=,pid=,stat=,command="])
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| ProcessError(format!("cannot inspect process session: {error}")))?;
    let output = String::from_utf8_lossy(&output.stdout);
    Ok(output
        .lines()
        .filter_map(parse_session_row)
        .filter(|row| row.session_id == session_id && !row.state.starts_with('Z'))
        .collect())
}

pub(super) fn session_is_alive(session_id: u32) -> Result<bool, ProcessError> {
    Ok(!session_processes(session_id)?.is_empty())
}

pub(super) fn process_matches_session_leader(
    pid: u32,
    session_id: u32,
    executable_path: &Path,
) -> Result<bool, ProcessError> {
    let Some(row) = process_row(pid)? else {
        return Ok(false);
    };
    Ok(row.session_id == session_id && command_matches_executable(&row.command, executable_path))
}

pub(super) fn process_session_id(pid: u32) -> Option<u32> {
    let session_id = unsafe { libc::getsid(pid as libc::pid_t) };
    (session_id > 0).then_some(session_id as u32)
}

fn parse_process_row(pid: u32, line: &str) -> Option<ProcessRow> {
    let mut fields = line.split_whitespace();
    let process_group = fields.next()?.parse().ok()?;
    let started_at = fields.by_ref().take(5).collect::<Vec<_>>().join(" ");
    let command = fields.collect::<Vec<_>>().join(" ");
    if command.is_empty() {
        return None;
    }
    Some(ProcessRow {
        pid,
        session_id: process_session_id(pid)?,
        process_group,
        started_at,
        state: String::new(),
        command,
    })
}

fn parse_session_row(line: &str) -> Option<ProcessRow> {
    let mut fields = line.split_whitespace();
    let process_group = fields.next()?.parse().ok()?;
    let pid = fields.next()?.parse().ok()?;
    let state = fields.next()?.to_owned();
    let command = fields.collect::<Vec<_>>().join(" ");
    Some(ProcessRow {
        pid,
        session_id: process_session_id(pid)?,
        process_group,
        started_at: String::new(),
        state,
        command,
    })
}

fn is_recorded_leader(row: &ProcessRow, identity: &ProcessIdentity) -> bool {
    row.pid == identity.pid
        && row.session_id == identity.session_id
        && row.started_at == identity.started_at
        && identity
            .start_time
            .is_none_or(|start_time| process_start_time(row.pid) == Some(start_time))
        && command_matches_executable(&row.command, &identity.executable_path)
}

pub(super) fn command_matches_executable(command: &str, executable_path: &Path) -> bool {
    let executable = executable_path.to_string_lossy();
    command == executable
        || command.starts_with(&format!("{executable} "))
        || command.contains(&format!(" {executable}"))
}

fn set_private_directory_permissions(path: &Path) -> Result<(), ProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        ProcessError(format!(
            "cannot restrict permissions on {}: {error}",
            path.display()
        ))
    })
}

fn write_private_token(path: &Path, token: &str) -> Result<(), ProcessError> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProcessError(
            "Photon automation token must be a 256-bit hexadecimal value".to_owned(),
        ));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| {
            ProcessError(format!(
                "cannot create private Photon automation credential {}: {error}",
                path.display()
            ))
        })?;
    file.write_all(token.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|error| {
            let _ = fs::remove_file(path);
            ProcessError(format!(
                "cannot write Photon automation credential: {error}"
            ))
        })
}

fn photon_runtime_paths(
    installation: &BrowserInstallation,
) -> Result<(PathBuf, PathBuf, String, PathBuf), ProcessError> {
    let info = if installation
        .application_path
        .extension()
        .is_some_and(|ext| ext == "app")
    {
        installation.application_path.join("Contents/Info.plist")
    } else {
        PathBuf::new()
    };

    let (root, helper_directory, service) = if info.is_file() {
        let helper =
            read_plist_key(&info, "LSEnvironment:PHOTON_HELPER_DIRECTORY").ok_or_else(|| {
                ProcessError("Photon bundle is missing PHOTON_HELPER_DIRECTORY metadata".to_owned())
            })?;
        let service = read_plist_key(&info, "LSEnvironment:PHOTON_PRESENTATION_XPC_SERVICE")
            .ok_or_else(|| {
                ProcessError(
                    "Photon bundle is missing its presentation service metadata".to_owned(),
                )
            })?;
        let helper = PathBuf::from(helper);
        let root = helper
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                ProcessError("Photon helper directory does not identify its checkout".to_owned())
            })?
            .to_path_buf();
        (root, helper, service)
    } else {
        let root = installation.application_path.clone();
        let profile = installation
            .executable_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .filter(|value| matches!(*value, "debug" | "release"))
            .ok_or_else(|| {
                ProcessError(
                    "Photon development executable is outside target/debug or target/release"
                        .to_owned(),
                )
            })?;
        let helper = root.join("build").join(format!("app-{profile}"));
        let uid = Command::new("/usr/bin/id")
            .arg("-u")
            .output()
            .map_err(|error| {
                ProcessError(format!("cannot determine the current user ID: {error}"))
            })?;
        let uid = String::from_utf8_lossy(&uid.stdout).trim().to_owned();
        let service = photon_service_name(&root, &uid);
        (root, helper, service)
    };

    if !helper_directory.is_dir() {
        return Err(ProcessError(format!(
            "Photon helper directory is missing at {}; build Photon first",
            helper_directory.display()
        )));
    }
    let profile = helper_directory
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("app-"))
        .ok_or_else(|| {
            ProcessError("Photon helper directory has an unknown build profile".to_owned())
        })?;
    let engine_directory = root.join("build").join(format!("engine-{profile}"));
    if !engine_directory
        .join("lib/liblagom-photonembedder.dylib")
        .is_file()
    {
        return Err(ProcessError(format!(
            "Photon Engine libraries are missing under {}; run ./photon build first",
            engine_directory.display()
        )));
    }
    let uid = Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .map_err(|error| ProcessError(format!("cannot determine the current user ID: {error}")))?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_owned();
    let target = format!("gui/{uid}/{service}");
    let service_status = Command::new("launchctl")
        .args(["print", &target])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            ProcessError(format!(
                "cannot inspect Photon presentation service: {error}"
            ))
        })?;
    if !service_status.success() {
        return Err(ProcessError(
            "Photon's presentation service is not registered; run './photon run' once to initialize this development build".to_owned(),
        ));
    }
    Ok((root, helper_directory, service, engine_directory))
}

fn read_plist_key(info: &Path, key: &str) -> Option<String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Print :{key}"))
        .arg(info)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn photon_service_name(root: &Path, uid: &str) -> String {
    let hash = root
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("org.photonbrowser.photon.presentation.u{uid}.h{hash:016x}.gpui-ce")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use super::*;
    use crate::{BrowserId, BrowserMetadata, InstallationId};

    #[test]
    fn launches_and_closes_only_its_private_process_session() {
        let unique = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let profile_root = std::env::temp_dir().join(format!("velocity-process-{unique}"));
        let executable_path = PathBuf::from("/bin/sleep");
        let installation = BrowserInstallation {
            installation_id: InstallationId::new("fixture-sleep").unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new("fixture").unwrap(),
                name: "Fixture Process".to_owned(),
                bundle_identifier: None,
                engine: None,
            },
            version: None,
            application_path: executable_path.clone(),
            executable_path,
        };
        let driver = MacOSProcessDriver::new(profile_root.clone());
        let identity = driver
            .launch(&installation, "session-test", Some("30"))
            .unwrap();
        assert_eq!(driver.state(&identity).unwrap(), ProcessState::Running);
        driver.terminate(&identity).unwrap();
        assert_eq!(driver.state(&identity).unwrap(), ProcessState::Exited);
        driver.cleanup("session-test", &identity).unwrap();
        let _ = fs::remove_dir_all(profile_root);
    }

    #[test]
    fn photon_token_file_is_private_and_never_overwritten() {
        use std::os::unix::fs::PermissionsExt;

        let unique = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("velocity-token-{unique}"));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("token");
        let token = "e".repeat(64);
        write_private_token(&path, &token).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&path).unwrap().trim(), token);
        assert!(write_private_token(&path, &"f".repeat(64)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap().trim(), token);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn photon_automation_receives_a_velocity_owned_isolated_profile() {
        use std::os::unix::fs::PermissionsExt;

        let unique = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let profile_root = std::env::temp_dir().join(format!("velocity-photon-profile-{unique}"));
        let automation_directory = std::env::temp_dir().join(format!("vlt-{}", "a".repeat(12)));
        let driver = MacOSProcessDriver::new(profile_root.clone())
            .with_photon_automation(automation_directory.join("p.sock"), "a".repeat(64));
        let profile = driver
            .create_profile(
                &fixture_installation(BrowserEngine::Ladybird),
                "photon-session",
            )
            .unwrap()
            .unwrap();

        assert_eq!(profile, profile_root.join("photon-session"));
        assert_eq!(
            fs::metadata(&profile_root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&profile).unwrap().permissions().mode() & 0o777,
            0o700
        );
        driver
            .cleanup(
                "photon-session",
                &ProcessIdentity {
                    pid: std::process::id(),
                    session_id: std::process::id(),
                    started_at: String::new(),
                    start_time: process_start_time(std::process::id()),
                    executable_path: PathBuf::from("/bin/true"),
                    profile_path: Some(profile),
                },
            )
            .unwrap();
        let _ = fs::remove_dir_all(profile_root);
    }

    fn fixture_installation(engine: BrowserEngine) -> BrowserInstallation {
        BrowserInstallation {
            installation_id: InstallationId::new("fixture-photon").unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new("photon").unwrap(),
                name: "Fixture Photon".to_owned(),
                bundle_identifier: None,
                engine: Some(engine),
            },
            version: None,
            application_path: PathBuf::from("/bin/true"),
            executable_path: PathBuf::from("/bin/true"),
        }
    }
}
