//! Optional, same-user automation protocol for isolated Photon sessions.

use futures::StreamExt;
use gpui::{App, BorrowAppContext, Global, WindowHandle};
use photon_core::BrowserCommand;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::FileTypeExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{BrowserWindow, BrowserWindowRegistry};
use crate::platform::app_update::update_app;

const PROTOCOL_VERSION: u32 = 1;
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const TOKEN_FILE: &str = "token";

/// A launch flag opts into automation. The credential itself is read from a
/// private sibling file and is never passed on the process command line.
pub(super) struct AutomationLaunch {
    socket_path: PathBuf,
}

impl AutomationLaunch {
    pub(super) fn from_process_arguments() -> Result<Option<Self>, String> {
        let mut arguments = std::env::args_os().skip(1);
        let mut socket_path = None;
        while let Some(argument) = arguments.next() {
            if argument == "--automation-socket" {
                if socket_path.is_some() {
                    return Err("--automation-socket may be specified only once".to_owned());
                }
                let Some(path) = arguments.next() else {
                    return Err("--automation-socket requires a path".to_owned());
                };
                let path = PathBuf::from(path);
                if !path.is_absolute() || path.file_name().is_none() {
                    return Err("automation socket path must be an absolute file path".to_owned());
                }
                socket_path = Some(path);
            } else if argument
                .to_string_lossy()
                .starts_with("--automation-socket=")
            {
                return Err("use '--automation-socket <absolute-path>'".to_owned());
            }
        }
        Ok(socket_path.map(|socket_path| Self { socket_path }))
    }
}

#[derive(Clone, Deserialize)]
struct Request {
    protocol_version: u32,
    request_id: String,
    authentication: String,
    #[serde(flatten)]
    command: Command,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "command", content = "params", rename_all = "snake_case")]
enum Command {
    Ping,
    GetVersion,
    ListWindows,
    ListProcesses,
    ListTabs { window_id: Option<u64> },
    NewTab { window_id: u64, url: String },
    CloseTab { tab_id: u64 },
    ActivateTab { tab_id: u64 },
    Navigate { tab_id: u64, url: String },
    Reload { tab_id: u64 },
    GetTabStatus { tab_id: u64 },
    ProbeSpeedometerResult { tab_id: u64 },
}

#[derive(Clone, Debug, Serialize)]
struct Response {
    protocol_version: u32,
    request_id: Option<String>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ProtocolError>,
}

#[derive(Clone, Debug, Serialize)]
struct ProtocolError {
    code: &'static str,
    message: String,
}

struct PendingRequest {
    request: Request,
    reply: SyncSender<Response>,
}

struct AutomationServerOwner(Option<AutomationServer>);

impl Global for AutomationServerOwner {}

pub(super) fn install(launch: AutomationLaunch, cx: &mut App) -> Result<(), String> {
    let (server, receiver) = AutomationServer::start(launch.socket_path)?;
    cx.set_global(AutomationServerOwner(Some(server)));
    cx.on_app_quit(|cx| {
        let server = cx.update_global::<AutomationServerOwner, _>(|owner, _| owner.0.take());
        async move {
            if let Some(server) = server {
                server.stop();
            }
        }
    })
    .detach();

    cx.spawn(async move |cx| {
        let mut receiver = receiver;
        while let Some(pending) = receiver.next().await {
            let response = update_app(cx, |cx| dispatch(pending.request, cx))
                .await
                .unwrap_or_else(|| {
                    failure(
                        None,
                        "application_unavailable",
                        "Photon's UI event loop is shutting down".to_owned(),
                    )
                });
            let _ = pending.reply.send(response);
        }
    })
    .detach();
    Ok(())
}

struct AutomationServer {
    stop: Arc<AtomicBool>,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
    worker: Option<JoinHandle<()>>,
}

impl AutomationServer {
    fn start(
        requested_socket_path: PathBuf,
    ) -> Result<(Self, futures::channel::mpsc::Receiver<PendingRequest>), String> {
        validate_isolated_profile()?;
        let parent = requested_socket_path
            .parent()
            .ok_or_else(|| "automation socket has no parent directory".to_owned())?;
        let parent = fs::canonicalize(parent)
            .map_err(|error| format!("cannot resolve automation socket directory: {error}"))?;
        validate_private_directory(&parent, "automation socket directory")?;
        let file_name = requested_socket_path
            .file_name()
            .ok_or_else(|| "automation socket has no file name".to_owned())?;
        let socket_path = parent.join(file_name);
        let token = read_token(&parent.join(TOKEN_FILE))?;
        let listener = UnixListener::bind(&socket_path).map_err(|error| {
            format!(
                "cannot bind automation socket {}: {error}",
                socket_path.display()
            )
        })?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            let _ = fs::remove_file(&socket_path);
            format!("cannot restrict automation socket permissions: {error}")
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot configure automation socket: {error}"))?;
        let metadata = fs::symlink_metadata(&socket_path)
            .map_err(|error| format!("cannot inspect bound automation socket: {error}"))?;
        let (sender, receiver) = futures::channel::mpsc::channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("photon-automation-socket".to_owned())
            .spawn(move || serve(listener, token, sender, worker_stop))
            .map_err(|error| format!("cannot start automation socket worker: {error}"))?;
        Ok((
            Self {
                stop,
                socket_path,
                socket_device: metadata.dev(),
                socket_inode: metadata.ino(),
                worker: Some(worker),
            },
            receiver,
        ))
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        remove_owned_socket(&self.socket_path, self.socket_device, self.socket_inode);
    }
}

impl Drop for AutomationServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        remove_owned_socket(&self.socket_path, self.socket_device, self.socket_inode);
    }
}

fn serve(
    listener: UnixListener,
    token: String,
    mut sender: futures::channel::mpsc::Sender<PendingRequest>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => handle_connection(stream, &token, &mut sender),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn handle_connection(
    mut stream: UnixStream,
    token: &str,
    sender: &mut futures::channel::mpsc::Sender<PendingRequest>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut bytes = Vec::with_capacity(1024);
    let read = BufReader::new(&mut stream)
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes);
    match read {
        Ok(size) if size > MAX_REQUEST_BYTES => {
            write_response(
                &mut stream,
                failure(
                    None,
                    "request_too_large",
                    "request exceeds 64 KiB".to_owned(),
                ),
            );
            return;
        }
        Ok(0) => return,
        Err(_) => {
            write_response(
                &mut stream,
                failure(
                    None,
                    "malformed_request",
                    "could not read request".to_owned(),
                ),
            );
            return;
        }
        Ok(_) => {}
    }
    let request = match decode_authenticated_request(&bytes, token) {
        Ok(request) => request,
        Err(response) => {
            write_response(&mut stream, *response);
            return;
        }
    };
    let request_id = request.request_id.clone();

    let (reply, response) = mpsc::sync_channel(1);
    let pending = PendingRequest { request, reply };
    if sender.try_send(pending).is_err() {
        write_response(
            &mut stream,
            failure(
                Some(request_id),
                "server_busy",
                "automation dispatcher is busy".to_owned(),
            ),
        );
        return;
    }
    let response = response.recv_timeout(REQUEST_TIMEOUT).unwrap_or_else(|_| {
        failure(
            Some(request_id),
            "request_timeout",
            "Photon did not process the request before the deadline".to_owned(),
        )
    });
    write_response(&mut stream, response);
}

fn decode_authenticated_request(bytes: &[u8], token: &str) -> Result<Request, Box<Response>> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(Box::new(failure(
            None,
            "request_too_large",
            "request exceeds 64 KiB".to_owned(),
        )));
    }
    let request: Request = serde_json::from_slice(bytes).map_err(|_| {
        Box::new(failure(
            None,
            "malformed_request",
            "request is not valid protocol JSON".to_owned(),
        ))
    })?;
    let request_id = request.request_id.clone();
    if request_id.is_empty() || request_id.len() > 128 {
        return Err(Box::new(failure(
            None,
            "invalid_request_id",
            "request ID must be 1–128 bytes".to_owned(),
        )));
    }
    if !constant_time_equal(request.authentication.as_bytes(), token.as_bytes()) {
        return Err(Box::new(failure(
            Some(request_id),
            "unauthorized",
            "authentication failed".to_owned(),
        )));
    }
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(Box::new(failure(
            Some(request_id),
            "unsupported_protocol",
            format!(
                "protocol version {} is unsupported",
                request.protocol_version
            ),
        )));
    }
    Ok(request)
}

fn write_response(stream: &mut UnixStream, response: Response) {
    if let Ok(bytes) = serde_json::to_vec(&response)
        && bytes.len() <= MAX_RESPONSE_BYTES
    {
        let _ = stream.write_all(&bytes);
        let _ = stream.write_all(b"\n");
    }
}

fn read_token(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect automation credential file: {error}"))?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 128
    {
        return Err(
            "automation credential file must be a current-user-owned private regular file"
                .to_owned(),
        );
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("cannot open automation credential file: {error}"))?;
    let mut token = String::new();
    file.read_to_string(&mut token)
        .map_err(|error| format!("cannot read automation credential file: {error}"))?;
    let token = token.trim_end_matches(['\r', '\n']);
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("automation credential must contain a 256-bit hexadecimal token".to_owned());
    }
    Ok(token.to_owned())
}

fn validate_private_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "{label} must be a current-user-owned directory with mode 0700"
        ));
    }
    Ok(())
}

fn validate_isolated_profile() -> Result<(), String> {
    let profile = std::env::var_os("PHOTON_PROFILE_DIR")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            "automation requires PHOTON_PROFILE_DIR for an isolated profile".to_owned()
        })?;
    let source_metadata = fs::symlink_metadata(&profile)
        .map_err(|error| format!("cannot inspect isolated Photon profile: {error}"))?;
    if !source_metadata.is_dir() || source_metadata.file_type().is_symlink() {
        return Err("automation profile must be a real directory, not a symlink".to_owned());
    }
    let profile = fs::canonicalize(profile)
        .map_err(|error| format!("cannot resolve isolated Photon profile: {error}"))?;
    let metadata = fs::symlink_metadata(&profile)
        .map_err(|error| format!("cannot inspect isolated Photon profile: {error}"))?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err("automation profile must be a directory owned by the current user".to_owned());
    }
    let home = std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "automation requires an isolated HOME directory".to_owned())?;
    let home_metadata = fs::symlink_metadata(&home)
        .map_err(|error| format!("cannot inspect isolated Photon HOME: {error}"))?;
    let home = fs::canonicalize(home)
        .map_err(|error| format!("cannot resolve isolated Photon HOME: {error}"))?;
    let profile_parent = profile.parent();
    if !home_metadata.is_dir()
        || home_metadata.file_type().is_symlink()
        || home_metadata.uid() != unsafe { libc::geteuid() }
        || home_metadata.mode() & 0o077 != 0
        || profile_parent != home.parent()
    {
        return Err(
            "automation requires private profile and HOME directories inside the same session root"
                .to_owned(),
        );
    }
    if let Some(session_root) = profile_parent {
        validate_private_directory(session_root, "automation session root")?;
    }
    Ok(())
}

fn remove_owned_socket(path: &Path, device: u64, inode: u64) {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.file_type().is_socket()
        && metadata.dev() == device
        && metadata.ino() == inode
    {
        let _ = fs::remove_file(path);
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn dispatch(request: Request, cx: &mut App) -> Response {
    let request_id = request.request_id;
    let result = match request.command {
        Command::Ping => Ok(json!({"alive": true})),
        Command::GetVersion => Ok(json!({
            "application_version": env!("CARGO_PKG_VERSION"),
            "protocol_version": PROTOCOL_VERSION,
            "capabilities": ["ping", "get_version", "list_windows", "list_processes", "list_tabs", "new_tab", "close_tab", "activate_tab", "navigate", "reload", "get_tab_status", "probe_speedometer_result"]
        })),
        Command::ListWindows => Ok(list_windows(cx)),
        Command::ListProcesses => list_processes(cx),
        Command::ListTabs { window_id } => list_tabs(window_id, cx),
        Command::NewTab { window_id, url } => new_tab(window_id, &url, cx),
        Command::CloseTab { tab_id } => close_tab(tab_id, cx),
        Command::ActivateTab { tab_id } => activate_tab(tab_id, cx),
        Command::Navigate { tab_id, url } => navigate(tab_id, &url, &request_id, cx),
        Command::Reload { tab_id } => reload(tab_id, cx),
        Command::GetTabStatus { tab_id } => tab_status(tab_id, cx),
        Command::ProbeSpeedometerResult { tab_id } => probe_speedometer_result(tab_id, cx),
    };
    match result {
        Ok(value) => Response {
            protocol_version: PROTOCOL_VERSION,
            request_id: Some(request_id),
            ok: true,
            result: Some(value),
            error: None,
        },
        Err((code, message)) => failure(Some(request_id), code, message),
    }
}

type DispatchResult = Result<Value, (&'static str, String)>;

fn browser_windows(cx: &App) -> Vec<WindowHandle<BrowserWindow>> {
    BrowserWindowRegistry::ordered_handles(cx)
        .into_iter()
        .filter_map(|handle| handle.downcast::<BrowserWindow>())
        .collect()
}

fn list_windows(cx: &App) -> Value {
    let windows = browser_windows(cx)
        .into_iter()
        .filter_map(|handle| {
            let window_id = handle.window_id().as_u64();
            handle
                .read(cx)
                .ok()
                .map(|browser| json!({"window_id": window_id, "tab_count": browser.tabs.len()}))
        })
        .collect::<Vec<_>>();
    json!({"windows": windows})
}

fn list_processes(cx: &App) -> DispatchResult {
    let mut registered = None;
    for handle in browser_windows(cx) {
        let browser = handle
            .read(cx)
            .map_err(|error| ("window_unavailable", error.to_string()))?;
        for tab in &browser.tabs {
            if let Some(webview) = tab.content.webview() {
                let view = webview.read(cx);
                registered = Some(view.registered_engine_processes());
                break;
            }
        }
        if registered.is_some() {
            break;
        }
    }
    let Some(processes) = registered else {
        return Err((
            "engine_process_list_unavailable",
            "no active Photon Engine view can provide its process-manager snapshot".to_owned(),
        ));
    };

    let mut seen = std::collections::BTreeSet::new();
    let unique = processes
        .iter()
        .all(|process| process.pid > 0 && !process.role.is_empty() && seen.insert(process.pid));
    let includes_application = processes
        .iter()
        .any(|process| process.pid == std::process::id() && process.role == "application");
    let identities_complete = processes.iter().all(|process| {
        process.start_time_in_microseconds.is_some()
            && process.parent_pid.is_some()
            && process.parent_start_time_in_microseconds.is_some()
    });
    let records = processes
        .iter()
        .map(|process| {
            json!({
                "pid": process.pid,
                "role": process.role,
                "state": "running",
                "start_time": process.start_time_in_microseconds.map(|value| {
                    json!({ "seconds": value / 1_000_000, "microseconds": value % 1_000_000 })
                }),
                "launch_generation": process.start_time_in_microseconds.map(|value| value.to_string()),
                "parent_identity": process.parent_pid.map(|pid| json!({
                    "pid": pid,
                    "start_time": process.parent_start_time_in_microseconds.map(|value| {
                        json!({ "seconds": value / 1_000_000, "microseconds": value % 1_000_000 })
                    })
                }))
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "scope": "active WebView::ProcessManager processes and its ProcessReaper on macOS",
        "lifecycle_source": "WebView::ProcessManager and ProcessReaper",
        "complete": unique && includes_application && identities_complete,
        "processes": records
    }))
}

fn list_tabs(window_id: Option<u64>, cx: &App) -> DispatchResult {
    let mut tabs = Vec::new();
    let handles = browser_windows(cx);
    let window_exists = handles
        .iter()
        .any(|handle| Some(handle.window_id().as_u64()) == window_id);
    for handle in handles {
        let current_window_id = handle.window_id().as_u64();
        if window_id.is_some_and(|requested| requested != current_window_id) {
            continue;
        }
        let browser = handle
            .read(cx)
            .map_err(|error| ("window_unavailable", error.to_string()))?;
        let window_tabs = browser
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                tab_value(
                    current_window_id,
                    index,
                    tab,
                    browser.active_tab == Some(index),
                    cx,
                )
            })
            .collect::<Vec<_>>();
        tabs.extend(window_tabs);
    }
    if window_id.is_some() && !window_exists {
        return Err((
            "window_not_found",
            "window identifier was not found".to_owned(),
        ));
    }
    Ok(json!({"tabs": tabs}))
}

fn tab_value(
    window_id: u64,
    index: usize,
    tab: &super::content::BrowserTab,
    active: bool,
    cx: &App,
) -> Value {
    let (url, title, loading, error_state) = tab
        .content
        .webview()
        .map(|webview| {
            let view = webview.read(cx);
            let error_state = view.crash_notice.map(|notice| match notice {
                super::super::CrashNotice::Reloading => "recovering_from_crash",
                super::super::CrashNotice::Reloaded(_) => "recovered_from_crash",
                super::super::CrashNotice::KeepsCrashing => "page_crashed",
            });
            (
                Some(view.state.url.clone()),
                Some(view.state.title.clone()),
                Some(view.state.loading),
                error_state,
            )
        })
        .unwrap_or_else(|| (tab.content.page_address(), None, Some(false), None));
    json!({
        "tab_id": tab.id,
        "window_id": window_id,
        "url": url,
        "title": title,
        "active": active,
        "loading": loading,
        "navigation_state": if loading == Some(true) { "loading" } else { "idle" },
        "error_state": error_state,
        "document_ready": Value::Null,
        "readiness_source": "Velocity run-and-tab fixture beacon",
        "position": index
    })
}

fn probe_speedometer_result(tab_id: u64, cx: &mut App) -> DispatchResult {
    let (handle, index) = find_tab(tab_id, cx)?;
    handle
        .update(cx, |browser, _window, cx| {
            let Some(webview) = browser.tabs[index].content.webview() else {
                return Err(("tab_unavailable", "tab has no WebView yet".to_owned()));
            };
            let allowed = webview
                .update(cx, |view, _cx| is_speedometer_url(&view.state.url));
            if !allowed {
                return Err((
                    "unsupported_origin",
                    "the read-only Speedometer observer is restricted to the official Speedometer 3.1 page or a loopback Speedometer 3.1 source".to_owned(),
                ));
            }
            let installed = webview.update(cx, |view, _cx| view.probe_speedometer_result());
            if !installed {
                return Err((
                    "page_unavailable",
                    "the benchmark document does not have an active engine page yet".to_owned(),
                ));
            }
            Ok::<(), (&'static str, String)>(())
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "observer": "speedometer_3_1", "state": "installed"}))
}

fn is_speedometer_url(url: &str) -> bool {
    let Some((scheme, remainder)) = url.split_once("://") else {
        return false;
    };
    let Some((authority, path_and_query)) = remainder.split_once('/') else {
        return false;
    };
    let path = format!(
        "/{}",
        path_and_query.split(['?', '#']).next().unwrap_or_default()
    );
    let expected_path = path == "/Speedometer3.1" || path == "/Speedometer3.1/";
    let official =
        scheme.eq_ignore_ascii_case("https") && authority.eq_ignore_ascii_case("browserbench.org");
    let loopback = scheme.eq_ignore_ascii_case("http")
        && authority
            .strip_prefix("127.0.0.1:")
            .and_then(|port| port.parse::<u16>().ok())
            .is_some_and(|port| port != 0);
    expected_path && (official || loopback)
}

fn new_tab(window_id: u64, url: &str, cx: &mut App) -> DispatchResult {
    validate_url(url)?;
    let handle = find_window(window_id, cx)?;
    let tab_id = handle
        .update(cx, |browser, window, cx| {
            let previous_count = browser.tabs.len();
            browser.open_in_new_tab(url, window, cx);
            browser
                .tabs
                .get(previous_count)
                .map(|tab| tab.id)
                .ok_or_else(|| {
                    (
                        "tab_creation_failed",
                        "Photon could not create a tab".to_owned(),
                    )
                })
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "window_id": window_id}))
}

fn close_tab(tab_id: u64, cx: &mut App) -> DispatchResult {
    let (handle, index) = find_tab(tab_id, cx)?;
    handle
        .update(cx, |browser, window, cx| {
            browser.close_tab(index, window, cx);
            Ok::<_, (&'static str, String)>(())
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "state": "closing"}))
}

fn activate_tab(tab_id: u64, cx: &mut App) -> DispatchResult {
    let (handle, index) = find_tab(tab_id, cx)?;
    handle
        .update(cx, |browser, window, cx| {
            browser.activate_tab(index, true, window, cx);
            Ok::<_, (&'static str, String)>(())
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "active": true}))
}

fn navigate(tab_id: u64, url: &str, navigation_id: &str, cx: &mut App) -> DispatchResult {
    validate_url(url)?;
    let (handle, index) = find_tab(tab_id, cx)?;
    handle
        .update(cx, |browser, window, cx| {
            if let Some(webview) = browser.tabs[index].content.webview() {
                webview
                    .update(cx, |view, cx| {
                        view.execute(BrowserCommand::Navigate(url.to_owned()), cx)
                    })
                    .map_err(|error| ("navigation_failed", format!("{error:#}")))?;
                Ok::<(), (&'static str, String)>(())
            } else {
                browser.activate_tab(index, true, window, cx);
                browser.navigate_active(url, window, cx);
                Ok::<(), (&'static str, String)>(())
            }
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "navigation_id": navigation_id, "state": "requested"}))
}

fn reload(tab_id: u64, cx: &mut App) -> DispatchResult {
    let (handle, index) = find_tab(tab_id, cx)?;
    handle
        .update(cx, |browser, _window, cx| {
            let Some(webview) = browser.tabs[index].content.webview() else {
                return Err(("tab_unavailable", "tab has no WebView yet".to_owned()));
            };
            webview
                .update(cx, |view, cx| view.execute(BrowserCommand::Reload, cx))
                .map_err(|error| ("navigation_failed", format!("{error:#}")))?;
            Ok::<(), (&'static str, String)>(())
        })
        .map_err(|error| ("window_unavailable", error.to_string()))??;
    Ok(json!({"tab_id": tab_id, "state": "reload_requested"}))
}

fn tab_status(tab_id: u64, cx: &App) -> DispatchResult {
    let (handle, index) = find_tab(tab_id, cx)?;
    let browser = handle
        .read(cx)
        .map_err(|error| ("window_unavailable", error.to_string()))?;
    let tab = &browser.tabs[index];
    Ok(tab_value(
        handle.window_id().as_u64(),
        index,
        tab,
        browser.active_tab == Some(index),
        cx,
    ))
}

fn find_window(
    window_id: u64,
    cx: &App,
) -> Result<WindowHandle<BrowserWindow>, (&'static str, String)> {
    browser_windows(cx)
        .into_iter()
        .find(|handle| handle.window_id().as_u64() == window_id)
        .ok_or_else(|| {
            (
                "window_not_found",
                format!("window {window_id} was not found"),
            )
        })
}

fn find_tab(
    tab_id: u64,
    cx: &App,
) -> Result<(WindowHandle<BrowserWindow>, usize), (&'static str, String)> {
    for handle in browser_windows(cx) {
        let index = handle
            .read(cx)
            .map_err(|error| ("window_unavailable", error.to_string()))?
            .tabs
            .iter()
            .position(|tab| tab.id == tab_id);
        if let Some(index) = index {
            return Ok((handle, index));
        }
    }
    Err(("tab_not_found", format!("tab {tab_id} was not found")))
}

fn validate_url(url: &str) -> Result<(), (&'static str, String)> {
    if url == "about:blank" {
        return Ok(());
    }
    let valid_scheme = url.split_once("://").is_some_and(|(scheme, authority)| {
        matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
            && !authority
                .split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
                .is_empty()
    });
    if !valid_scheme || url.chars().any(char::is_control) || url.chars().any(char::is_whitespace) {
        return Err((
            "invalid_url",
            "automation accepts only about:blank and absolute HTTP(S) URLs".to_owned(),
        ));
    }
    Ok(())
}

fn failure(request_id: Option<String>, code: &'static str, message: String) -> Response {
    Response {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        ok: false,
        result: None,
        error: Some(ProtocolError { code, message }),
    }
}

#[cfg(test)]
mod speedometer_url_tests {
    use super::is_speedometer_url;

    #[test]
    fn speedometer_probe_is_limited_to_official_or_loopback_version_31() {
        assert!(is_speedometer_url(
            "https://browserbench.org/Speedometer3.1/?startAutomatically"
        ));
        assert!(is_speedometer_url(
            "http://127.0.0.1:49152/Speedometer3.1/?startAutomatically"
        ));
        assert!(!is_speedometer_url("https://example.com/Speedometer3.1/"));
        assert!(!is_speedometer_url(
            "http://browserbench.org/Speedometer3.1/"
        ));
        assert!(!is_speedometer_url(
            "https://browserbench.org/Speedometer3/"
        ));
        assert!(!is_speedometer_url(
            "https://user@browserbench.org/Speedometer3.1/"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Command, PROTOCOL_VERSION, constant_time_equal, decode_authenticated_request, failure,
        validate_url,
    };

    #[test]
    fn url_validation_allows_only_blank_and_http_urls() {
        assert!(validate_url("about:blank").is_ok());
        assert!(validate_url("http://127.0.0.1:1234/simple").is_ok());
        assert!(validate_url("https://example.test/path").is_ok());
        assert!(validate_url("file:///tmp/page.html").is_err());
        assert!(validate_url("photon://settings").is_err());
        assert!(validate_url("http://bad host/").is_err());
    }

    #[test]
    fn authentication_comparison_checks_full_secret_without_early_exit() {
        assert!(constant_time_equal(b"same", b"same"));
        assert!(!constant_time_equal(b"same", b"sane"));
        assert!(!constant_time_equal(b"same", b"same-long"));
    }

    #[test]
    fn protocol_decodes_authenticated_commands_and_correlates_request_ids() {
        let request = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": "test-17",
            "authentication": "a".repeat(64),
            "command": "navigate",
            "params": {"tab_id": 42, "url": "http://127.0.0.1/page"}
        });
        let decoded =
            decode_authenticated_request(&serde_json::to_vec(&request).unwrap(), &"a".repeat(64))
                .unwrap();
        assert_eq!(decoded.request_id, "test-17");
        assert!(matches!(
            decoded.command,
            Command::Navigate { tab_id: 42, .. }
        ));

        let response = failure(
            Some(decoded.request_id),
            "test_error",
            "fixture error".to_owned(),
        );
        let response = serde_json::to_value(response).unwrap();
        assert_eq!(response["request_id"], "test-17");
    }

    #[test]
    fn protocol_rejects_malformed_oversized_unauthorized_and_old_requests() {
        assert_eq!(
            decode_authenticated_request(b"{bad json", &"a".repeat(64))
                .err()
                .unwrap()
                .error
                .unwrap()
                .code,
            "malformed_request"
        );
        assert_eq!(
            decode_authenticated_request(
                &vec![b' '; super::MAX_REQUEST_BYTES + 1],
                &"a".repeat(64)
            )
            .err()
            .unwrap()
            .error
            .unwrap()
            .code,
            "request_too_large"
        );

        let make_request = |version, authentication: &str, command| {
            serde_json::json!({
                "protocol_version": version,
                "request_id": "request-1",
                "authentication": authentication,
                "command": command
            })
        };
        let unauthorized = make_request(PROTOCOL_VERSION, &"b".repeat(64), "ping");
        assert_eq!(
            decode_authenticated_request(
                &serde_json::to_vec(&unauthorized).unwrap(),
                &"a".repeat(64)
            )
            .err()
            .unwrap()
            .error
            .unwrap()
            .code,
            "unauthorized"
        );
        let old_protocol = make_request(PROTOCOL_VERSION + 1, &"a".repeat(64), "ping");
        assert_eq!(
            decode_authenticated_request(
                &serde_json::to_vec(&old_protocol).unwrap(),
                &"a".repeat(64)
            )
            .err()
            .unwrap()
            .error
            .unwrap()
            .code,
            "unsupported_protocol"
        );
        let unknown_command = make_request(PROTOCOL_VERSION, &"a".repeat(64), "run_shell");
        assert_eq!(
            decode_authenticated_request(
                &serde_json::to_vec(&unknown_command).unwrap(),
                &"a".repeat(64)
            )
            .err()
            .unwrap()
            .error
            .unwrap()
            .code,
            "malformed_request"
        );
    }
}
