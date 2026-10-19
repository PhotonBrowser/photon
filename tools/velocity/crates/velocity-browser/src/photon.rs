//! Client for Photon Automation Protocol v1 over a private Unix socket.

use crate::{AutomationProtocol, BrowserAutomationAdapter};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use velocity_metrics::ProcessStartTime;

const PROTOCOL_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Deserialize)]
pub struct PhotonWindow {
    pub window_id: u64,
    pub tab_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PhotonTab {
    pub tab_id: u64,
    pub window_id: u64,
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub active: bool,
    pub loading: Option<bool>,
    #[serde(default)]
    pub navigation_state: Option<String>,
    #[serde(default)]
    pub error_state: Option<String>,
    pub document_ready: Option<bool>,
    #[serde(default)]
    pub engine_load_finished: Option<bool>,
    #[serde(default)]
    pub document_generation: Option<u64>,
    #[serde(default)]
    pub window_count: Option<usize>,
    #[serde(default)]
    pub tab_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PhotonObserverError {
    pub name: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PhotonSpeedometerResult {
    pub state: String,
    pub version: Option<String>,
    pub score: Option<f64>,
    #[serde(default)]
    pub internal_scores: Vec<f64>,
    #[serde(default)]
    pub current_suite: Option<String>,
    #[serde(default)]
    pub current_test: Option<String>,
    #[serde(default)]
    pub current_iteration: Option<usize>,
    #[serde(default)]
    pub completed_tests: Option<usize>,
    #[serde(default)]
    pub total_tests: Option<usize>,
    #[serde(default)]
    pub runner_is_running: Option<bool>,
    #[serde(default)]
    pub runner_finished_test_count: Option<usize>,
    #[serde(default)]
    pub runner_marks: Vec<String>,
    #[serde(default)]
    pub document_visibility_state: Option<String>,
    #[serde(default)]
    pub test_frame_src: Option<String>,
    #[serde(default)]
    pub test_frame_location: Option<String>,
    #[serde(default)]
    pub test_frame_ready_state: Option<String>,
    #[serde(default)]
    pub test_frame_visibility_state: Option<String>,
    #[serde(default)]
    pub test_frame_has_todo_app: Option<bool>,
    #[serde(default)]
    pub test_frame_has_new_todo: Option<bool>,
    #[serde(default)]
    pub test_frame_boot_diagnostics: Option<String>,
    #[serde(default)]
    pub test_frame_onload_handler: Option<bool>,
    #[serde(default)]
    pub test_frame_error: Option<String>,
    #[serde(default)]
    pub benchmark_running: Option<bool>,
    #[serde(default)]
    pub document_generation: Option<u64>,
    #[serde(default)]
    pub document_url: Option<String>,
    #[serde(default)]
    pub document_ready_state: Option<String>,
    #[serde(default)]
    pub last_progress_at_ms: Option<f64>,
    #[serde(default)]
    pub observer_errors: Vec<PhotonObserverError>,
    #[serde(default)]
    pub viewport_warning: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PhotonJetStreamResult {
    pub state: String,
    #[serde(default)]
    pub initialized: bool,
    #[serde(default)]
    pub started: bool,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub failed: bool,
    pub official_score: Option<f64>,
    #[serde(default)]
    pub completed_workloads: usize,
    #[serde(default)]
    pub total_workloads: usize,
    pub current_workload: Option<String>,
    #[serde(default)]
    pub status_text: String,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub subtest_scores: BTreeMap<String, f64>,
    pub document_generation: Option<u64>,
    pub document_url: Option<String>,
}

pub fn parse_speedometer_result_title(
    title: Option<&str>,
) -> Result<Option<PhotonSpeedometerResult>, PhotonAutomationError> {
    let Some(payload) = title.and_then(|title| title.strip_prefix("VelocitySpeedometer3.1:"))
    else {
        return Ok(None);
    };
    serde_json::from_str(payload).map(Some).map_err(|error| {
        PhotonAutomationError(format!("invalid Photon Speedometer status: {error}"))
    })
}

pub fn parse_jetstream_result_title(
    title: Option<&str>,
) -> Result<Option<PhotonJetStreamResult>, PhotonAutomationError> {
    let Some(payload) = title.and_then(|title| title.strip_prefix("VelocityJetStream3.0:")) else {
        return Ok(None);
    };
    serde_json::from_str(payload)
        .map(Some)
        .map_err(|error| PhotonAutomationError(format!("invalid Photon JetStream status: {error}")))
}

#[derive(Clone, Debug, Deserialize)]
pub struct PhotonRegisteredProcess {
    pub pid: u32,
    pub role: String,
    pub state: String,
    pub start_time: Option<ProcessStartTime>,
    pub launch_generation: Option<String>,
    pub parent_identity: Option<PhotonParentIdentity>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PhotonParentIdentity {
    pub pid: u32,
    pub start_time: Option<ProcessStartTime>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PhotonProcessRegistry {
    pub scope: String,
    pub lifecycle_source: String,
    pub complete: bool,
    pub processes: Vec<PhotonRegisteredProcess>,
}

pub struct PhotonAutomationClient {
    socket_path: PathBuf,
    token: String,
    request_id: u64,
    window_id: u64,
    application_version: String,
    capabilities: BTreeSet<String>,
}

impl std::fmt::Debug for PhotonAutomationClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhotonAutomationClient")
            .field("socket_path", &self.socket_path)
            .field("request_id", &self.request_id)
            .field("window_id", &self.window_id)
            .field("application_version", &self.application_version)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct PhotonAutomationError(pub String);

impl std::fmt::Display for PhotonAutomationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PhotonAutomationError {}

#[derive(Deserialize)]
struct AutomationResponse {
    protocol_version: u32,
    request_id: Option<String>,
    ok: bool,
    result: Option<Value>,
    error: Option<AutomationResponseError>,
}

#[derive(Deserialize)]
struct AutomationResponseError {
    code: String,
    message: String,
}

impl PhotonAutomationClient {
    pub fn connect(
        socket_path: impl Into<PathBuf>,
        token_path: impl AsRef<Path>,
        timeout: Duration,
    ) -> Result<Self, PhotonAutomationError> {
        let socket_path = socket_path.into();
        validate_private_socket(&socket_path)?;
        let token = read_private_token(token_path.as_ref())?;
        let mut client = Self {
            socket_path,
            token,
            request_id: 0,
            window_id: 0,
            application_version: String::new(),
            capabilities: BTreeSet::new(),
        };
        let version = client.call("get_version", None, timeout)?;
        let protocol_version = version
            .get("protocol_version")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                PhotonAutomationError("Photon omitted its protocol version".to_owned())
            })?;
        if protocol_version != u64::from(PROTOCOL_VERSION) {
            return Err(PhotonAutomationError(format!(
                "Photon protocol version {protocol_version} is incompatible with Velocity protocol version {PROTOCOL_VERSION}"
            )));
        }
        client.application_version = version
            .get("application_version")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        client.capabilities = version
            .get("capabilities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let windows = client.list_windows(timeout)?;
        match windows.as_slice() {
            [window] => client.window_id = window.window_id,
            [] => {
                return Err(PhotonAutomationError(
                    "Photon automation connected, but no controlled window is available".to_owned(),
                ));
            }
            _ => {
                return Err(PhotonAutomationError(format!(
                    "isolated Photon session exposed {} windows; refusing ambiguous automation",
                    windows.len()
                )));
            }
        }
        Ok(client)
    }

    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn connect_with_cancel(
        socket_path: impl Into<PathBuf>,
        token_path: impl AsRef<Path>,
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Self, PhotonAutomationError> {
        let socket_path = socket_path.into();
        let token_path = token_path.as_ref().to_path_buf();
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            if cancelled() {
                return Err(PhotonAutomationError(
                    "Photon automation connection was cancelled".to_owned(),
                ));
            }
            match Self::connect(&socket_path, &token_path, Duration::from_secs(2)) {
                Ok(client) => return Ok(client),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(Duration::from_millis(40));
        }
        Err(PhotonAutomationError(format!(
            "timed out connecting to Photon automation socket {}: {}",
            socket_path.display(),
            last_error.map_or_else(
                || "server did not respond".to_owned(),
                |error| error.to_string()
            )
        )))
    }

    pub fn window_id(&self) -> u64 {
        self.window_id
    }

    pub fn application_version(&self) -> &str {
        &self.application_version
    }

    pub fn list_windows(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<PhotonWindow>, PhotonAutomationError> {
        let result = self.call("list_windows", None, timeout)?;
        serde_json::from_value(result.get("windows").cloned().unwrap_or(Value::Null))
            .map_err(|error| PhotonAutomationError(format!("invalid Photon window list: {error}")))
    }

    pub fn list_processes(
        &mut self,
        timeout: Duration,
    ) -> Result<PhotonProcessRegistry, PhotonAutomationError> {
        let result = self.call("list_processes", None, timeout)?;
        serde_json::from_value(result).map_err(|error| {
            PhotonAutomationError(format!("invalid Photon process registry response: {error}"))
        })
    }

    pub fn list_tabs(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<PhotonTab>, PhotonAutomationError> {
        let result = self.call(
            "list_tabs",
            Some(json!({"window_id": self.window_id})),
            timeout,
        )?;
        serde_json::from_value(result.get("tabs").cloned().unwrap_or(Value::Null))
            .map_err(|error| PhotonAutomationError(format!("invalid Photon tab list: {error}")))
    }

    pub fn new_tab(
        &mut self,
        url: &str,
        timeout: Duration,
    ) -> Result<PhotonTab, PhotonAutomationError> {
        let result = self.call(
            "new_tab",
            Some(json!({"window_id": self.window_id, "url": url})),
            timeout,
        )?;
        let tab_id = result
            .get("tab_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                PhotonAutomationError("Photon did not return a tab identifier".to_owned())
            })?;
        self.list_tabs(timeout)?
            .into_iter()
            .find(|tab| tab.tab_id == tab_id)
            .ok_or_else(|| {
                PhotonAutomationError(format!(
                    "Photon acknowledged new tab {tab_id}, but did not expose it in list_tabs"
                ))
            })
    }

    pub fn navigate(
        &mut self,
        tab_id: u64,
        url: &str,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        self.call(
            "navigate",
            Some(json!({"tab_id": tab_id, "url": url})),
            timeout,
        )?;
        Ok(())
    }

    pub fn close_tab(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        self.call("close_tab", Some(json!({"tab_id": tab_id})), timeout)?;
        Ok(())
    }

    pub fn activate_tab(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        self.call("activate_tab", Some(json!({"tab_id": tab_id})), timeout)?;
        Ok(())
    }

    pub fn tab_status(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<PhotonTab, PhotonAutomationError> {
        let result = self.call("get_tab_status", Some(json!({"tab_id": tab_id})), timeout)?;
        serde_json::from_value(result)
            .map_err(|error| PhotonAutomationError(format!("invalid Photon tab status: {error}")))
    }

    pub fn install_speedometer_result_observer(
        &mut self,
        tab_id: u64,
        document_generation: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        if !self.supports("probe_speedometer_result") {
            return Err(PhotonAutomationError(
                "Photon does not advertise its restricted Speedometer 3.1 result observer"
                    .to_owned(),
            ));
        }
        let result = self.call(
            "probe_speedometer_result",
            Some(json!({
                "tab_id": tab_id,
                "document_generation": document_generation
            })),
            timeout,
        )?;
        if result.get("document_generation").and_then(Value::as_u64) != Some(document_generation) {
            return Err(PhotonAutomationError(
                "Photon installed the Speedometer observer for a different document generation"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn install_jetstream_result_observer(
        &mut self,
        tab_id: u64,
        document_generation: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        if !self.supports("probe_jetstream_result") {
            return Err(PhotonAutomationError(
                "Photon does not advertise its restricted JetStream 3.0 result observer".to_owned(),
            ));
        }
        let result = self.call(
            "probe_jetstream_result",
            Some(json!({
                "tab_id": tab_id,
                "document_generation": document_generation
            })),
            timeout,
        )?;
        if result.get("document_generation").and_then(Value::as_u64) != Some(document_generation) {
            return Err(PhotonAutomationError(
                "Photon installed the JetStream observer for a different document generation"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn request_jetstream_start(
        &mut self,
        tab_id: u64,
        document_generation: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        if !self.supports("request_jetstream_start") {
            return Err(PhotonAutomationError(
                "Photon does not advertise the restricted JetStream start control".to_owned(),
            ));
        }
        let result = self.call(
            "request_jetstream_start",
            Some(json!({
                "tab_id": tab_id,
                "document_generation": document_generation
            })),
            timeout,
        )?;
        if result.get("requested").and_then(Value::as_bool) != Some(true)
            || result.get("document_generation").and_then(Value::as_u64)
                != Some(document_generation)
        {
            return Err(PhotonAutomationError(
                "Photon did not confirm the JetStream start request for the current document"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn performance_start(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<(), PhotonAutomationError> {
        if !self.supports("performance_diagnostics_v1") {
            return Err(PhotonAutomationError(
                "Photon does not advertise performance diagnostics schema v1".to_owned(),
            ));
        }
        let response = self.call(
            "performance_start",
            Some(json!({"tab_id": tab_id})),
            timeout,
        )?;
        validate_task_diagnostics_envelope(&response, tab_id, Some("started"), false)?;
        Ok(())
    }

    pub fn performance_snapshot(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<Value, PhotonAutomationError> {
        if !self.supports("performance_diagnostics_v1") {
            return Err(PhotonAutomationError(
                "Photon does not advertise performance diagnostics schema v1".to_owned(),
            ));
        }
        let response = self.call(
            "performance_snapshot",
            Some(json!({"tab_id": tab_id})),
            timeout,
        )?;
        validate_task_diagnostics_envelope(&response, tab_id, None, true)?;
        Ok(response)
    }

    pub fn performance_frame_snapshot(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<Value, PhotonAutomationError> {
        if !self.supports("performance_frame_timing_v1") {
            return Err(PhotonAutomationError(
                "Photon does not advertise Metal drawable presentation timing schema v1".to_owned(),
            ));
        }
        let response = self.call(
            "performance_frame_snapshot",
            Some(json!({"tab_id": tab_id})),
            timeout,
        )?;
        validate_frame_diagnostics_envelope(&response, tab_id)?;
        Ok(response)
    }

    pub fn performance_stop(
        &mut self,
        tab_id: u64,
        timeout: Duration,
    ) -> Result<Value, PhotonAutomationError> {
        if !self.supports("performance_diagnostics_v1") {
            return Err(PhotonAutomationError(
                "Photon does not advertise performance diagnostics schema v1".to_owned(),
            ));
        }
        let response = self.call("performance_stop", Some(json!({"tab_id": tab_id})), timeout)?;
        validate_task_diagnostics_envelope(&response, tab_id, Some("stopped"), true)?;
        Ok(response)
    }

    fn call(
        &mut self,
        command: &str,
        parameters: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, PhotonAutomationError> {
        self.request_id = self.request_id.saturating_add(1);
        let request_id = self.request_id.to_string();
        let mut request = json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": request_id,
            "authentication": self.token,
            "command": command
        });
        if let Some(parameters) = parameters {
            request["params"] = parameters;
        }
        let bytes = serde_json::to_vec(&request).map_err(|error| {
            PhotonAutomationError(format!("cannot encode protocol request: {error}"))
        })?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(PhotonAutomationError(
                "protocol request exceeds 64 KiB".to_owned(),
            ));
        }
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|error| {
            PhotonAutomationError(format!(
                "cannot connect to Photon automation socket: {error}"
            ))
        })?;
        validate_bound_socket(&self.socket_path)?;
        stream
            .set_read_timeout(Some(timeout))
            .and_then(|()| stream.set_write_timeout(Some(timeout)))
            .map_err(|error| {
                PhotonAutomationError(format!("cannot set protocol timeout: {error}"))
            })?;
        stream
            .write_all(&bytes)
            .and_then(|()| stream.write_all(b"\n"))
            .map_err(|error| {
                PhotonAutomationError(format!("cannot send protocol request: {error}"))
            })?;
        let mut response_bytes = Vec::with_capacity(1024);
        let read = BufReader::new(stream)
            .take((MAX_MESSAGE_BYTES + 1) as u64)
            .read_until(b'\n', &mut response_bytes)
            .map_err(|error| {
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) {
                    PhotonAutomationError(format!(
                        "timed out after {:.1}s waiting for Photon automation response to '{command}'",
                        timeout.as_secs_f64()
                    ))
                } else {
                    PhotonAutomationError(format!("cannot read protocol response: {error}"))
                }
            })?;
        if read == 0 || read > MAX_MESSAGE_BYTES {
            return Err(PhotonAutomationError(
                "Photon returned an empty or oversized protocol response".to_owned(),
            ));
        }
        let response: AutomationResponse =
            serde_json::from_slice(&response_bytes).map_err(|error| {
                PhotonAutomationError(format!("Photon returned malformed JSON: {error}"))
            })?;
        if response.protocol_version != PROTOCOL_VERSION {
            return Err(PhotonAutomationError(format!(
                "Photon response used protocol version {}, expected {PROTOCOL_VERSION}",
                response.protocol_version
            )));
        }
        if response.request_id.as_deref() != Some(&request_id) {
            let error_detail = response.error.as_ref().map_or_else(String::new, |error| {
                let message = error.message.chars().take(256).collect::<String>();
                format!("; uncorrelated Photon error {}: {message}", error.code)
            });
            return Err(PhotonAutomationError(format!(
                "Photon response request ID did not match the request (expected {request_id}, received {}){error_detail}",
                response.request_id.as_deref().unwrap_or("<missing>"),
            )));
        }
        if !response.ok {
            let error = response.error.ok_or_else(|| {
                PhotonAutomationError(
                    "Photon rejected a request without an error object".to_owned(),
                )
            })?;
            return Err(PhotonAutomationError(format!(
                "{}: {}",
                error.code, error.message
            )));
        }
        response
            .result
            .ok_or_else(|| PhotonAutomationError("Photon response omitted its result".to_owned()))
    }
}

fn validate_task_diagnostics_envelope(
    response: &Value,
    tab_id: u64,
    expected_state: Option<&str>,
    require_metrics: bool,
) -> Result<(), PhotonAutomationError> {
    if response.get("schema_version").and_then(Value::as_u64) != Some(1)
        || response.get("tab_id").and_then(Value::as_u64) != Some(tab_id)
    {
        return Err(PhotonAutomationError(
            "Photon returned an invalid task diagnostics envelope".to_owned(),
        ));
    }
    if let Some(expected_state) = expected_state
        && response.get("state").and_then(Value::as_str) != Some(expected_state)
    {
        return Err(PhotonAutomationError(format!(
            "Photon did not confirm task diagnostics state '{expected_state}'"
        )));
    }
    if require_metrics {
        let metrics = response.get("metrics").ok_or_else(|| {
            PhotonAutomationError("Photon omitted task diagnostics metrics".to_owned())
        })?;
        if metrics.get("schema_version").and_then(Value::as_u64) != Some(1)
            || metrics.get("scope").and_then(Value::as_str) != Some("webcontent_processes")
            || !metrics.get("processes").is_some_and(Value::is_array)
        {
            return Err(PhotonAutomationError(
                "Photon returned an unsupported task diagnostics metrics schema".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_frame_diagnostics_envelope(
    response: &Value,
    tab_id: u64,
) -> Result<(), PhotonAutomationError> {
    let metrics = response
        .get("metrics")
        .ok_or_else(|| PhotonAutomationError("Photon omitted frame timing metrics".to_owned()))?;
    if response.get("schema_version").and_then(Value::as_u64) != Some(1)
        || response.get("tab_id").and_then(Value::as_u64) != Some(tab_id)
        || metrics.get("schema_version").and_then(Value::as_u64) != Some(1)
        || metrics.get("scope").and_then(Value::as_str) != Some("photon_tab_engine_surface_frames")
        || !metrics.get("frames").is_some_and(Value::is_array)
        || metrics
            .get("frame_sample_fields")
            .and_then(Value::as_array)
            .is_none()
    {
        return Err(PhotonAutomationError(
            "Photon returned an unsupported frame timing diagnostics schema".to_owned(),
        ));
    }
    Ok(())
}

impl BrowserAutomationAdapter for PhotonAutomationClient {
    fn protocol(&self) -> AutomationProtocol {
        AutomationProtocol::PhotonAutomationProtocol
    }

    fn navigate_initial_page(&mut self, url: &str) -> Result<String, String> {
        let timeout = Duration::from_secs(10);
        let tabs = self.list_tabs(timeout).map_err(|error| error.to_string())?;
        if let Some(tab) = tabs.first() {
            self.navigate(tab.tab_id, url, timeout)
                .map(|_| tab.tab_id.to_string())
                .map_err(|error| error.to_string())
        } else {
            self.new_tab(url, timeout)
                .map(|tab| tab.tab_id.to_string())
                .map_err(|error| error.to_string())
        }
    }

    fn create_tab(&mut self, url: &str) -> Result<String, String> {
        self.new_tab(url, Duration::from_secs(10))
            .map(|tab| tab.tab_id.to_string())
            .map_err(|error| error.to_string())
    }

    fn page_count(&mut self) -> Result<usize, String> {
        self.list_tabs(Duration::from_secs(10))
            .map(|tabs| tabs.len())
            .map_err(|error| error.to_string())
    }
}

fn validate_private_socket(path: &Path) -> Result<(), PhotonAutomationError> {
    let parent = path
        .parent()
        .ok_or_else(|| PhotonAutomationError("socket path has no directory".to_owned()))?;
    let directory = fs::symlink_metadata(parent).map_err(|error| {
        PhotonAutomationError(format!("cannot inspect socket directory: {error}"))
    })?;
    if !directory.is_dir()
        || directory.uid() != unsafe { libc::geteuid() }
        || directory.mode() & 0o077 != 0
    {
        return Err(PhotonAutomationError(
            "Photon socket directory is not private to the current user".to_owned(),
        ));
    }
    Ok(())
}

fn validate_bound_socket(path: &Path) -> Result<(), PhotonAutomationError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PhotonAutomationError(format!("cannot inspect Photon socket: {error}")))?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(PhotonAutomationError(
            "Photon socket is not a private current-user-owned Unix socket".to_owned(),
        ));
    }
    Ok(())
}

fn read_private_token(path: &Path) -> Result<String, PhotonAutomationError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PhotonAutomationError(format!("cannot inspect Photon credential file: {error}"))
    })?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 128
    {
        return Err(PhotonAutomationError(
            "Photon credential file must be a private current-user-owned regular file".to_owned(),
        ));
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            PhotonAutomationError(format!("cannot open Photon credential: {error}"))
        })?;
    let mut token = String::new();
    file.read_to_string(&mut token).map_err(|error| {
        PhotonAutomationError(format!("cannot read Photon credential: {error}"))
    })?;
    let token = token.trim_end_matches(['\r', '\n']);
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PhotonAutomationError(
            "Photon credential is not a 256-bit hexadecimal token".to_owned(),
        ));
    }
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        PhotonAutomationClient, PhotonTab, ProcessStartTime, parse_jetstream_result_title,
        parse_speedometer_result_title,
    };
    use serde_json::{Value, json};
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        os::unix::{fs::PermissionsExt, net::UnixListener},
        path::PathBuf,
        thread,
        time::Duration,
    };

    static NEXT_DIRECTORY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    fn private_temp_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "velocity-photon-test-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn reply(
        stream: &mut std::os::unix::net::UnixStream,
        request: &Value,
        result: Value,
        mismatch_id: bool,
    ) {
        let request_id = request["request_id"].as_str().unwrap();
        let response = json!({
            "protocol_version": 1,
            "request_id": if mismatch_id { "another-request" } else { request_id },
            "ok": true,
            "result": result
        });
        serde_json::to_writer(&mut *stream, &response).unwrap();
        stream.write_all(b"\n").unwrap();
    }

    #[test]
    fn negotiates_protocol_and_verifies_response_correlation() {
        let directory = private_temp_directory();
        let socket_path = directory.join("automation.sock");
        let token_path = directory.join("token");
        fs::write(&token_path, format!("{}\n", "c".repeat(64))).unwrap();
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = UnixListener::bind(&socket_path).unwrap();
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).unwrap();
        let worker = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["authentication"], "c".repeat(64));
                match request["command"].as_str().unwrap() {
                    "get_version" => reply(
                        &mut stream,
                        &request,
                        json!({
                            "protocol_version": 1,
                            "application_version": "test",
                            "capabilities": ["list_tabs", "list_processes", "new_tab", "navigate"]
                        }),
                        false,
                    ),
                    "list_windows" => reply(
                        &mut stream,
                        &request,
                        json!({"windows": [{"window_id": 81, "tab_count": 1}]}),
                        false,
                    ),
                    "list_processes" => reply(
                        &mut stream,
                        &request,
                        json!({
                            "scope": "active processes registered by WebView::ProcessManager",
                            "lifecycle_source": "WebView::ProcessManager",
                            "complete": true,
                            "processes": [{
                                "pid": 42,
                                "role": "application",
                                "state": "running",
                                "start_time": {"seconds": 1, "microseconds": 2},
                                "launch_generation": "1000002",
                                "parent_identity": {
                                    "pid": 41,
                                    "start_time": {"seconds": 1, "microseconds": 1}
                                }
                            }]
                        }),
                        false,
                    ),
                    command => panic!("unexpected command {command}"),
                }
            }
        });
        let mut client =
            PhotonAutomationClient::connect(&socket_path, &token_path, Duration::from_secs(1))
                .unwrap();
        assert_eq!(client.window_id(), 81);
        assert_eq!(client.application_version(), "test");
        assert!(client.supports("new_tab"));
        assert!(client.supports("list_processes"));
        assert!(!client.supports("close_window"));
        let registry = client.list_processes(Duration::from_secs(1)).unwrap();
        assert!(registry.complete);
        assert_eq!(registry.processes[0].pid, 42);
        assert_eq!(registry.processes[0].role, "application");
        assert_eq!(
            registry.processes[0].start_time,
            Some(ProcessStartTime {
                seconds: 1,
                microseconds: 2,
            })
        );
        let parent = registry.processes[0].parent_identity.as_ref().unwrap();
        assert_eq!(parent.pid, 41);
        assert_eq!(
            parent.start_time,
            Some(ProcessStartTime {
                seconds: 1,
                microseconds: 1,
            })
        );
        worker.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn negotiates_and_calls_task_queue_diagnostics_with_the_fixed_schema() {
        let directory = private_temp_directory();
        let socket_path = directory.join("automation.sock");
        let token_path = directory.join("token");
        fs::write(&token_path, format!("{}\n", "f".repeat(64))).unwrap();
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = UnixListener::bind(&socket_path).unwrap();
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).unwrap();
        let worker = thread::spawn(move || {
            for expected_command in [
                "get_version",
                "list_windows",
                "performance_start",
                "performance_snapshot",
                "performance_stop",
                "performance_frame_snapshot",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["authentication"], "f".repeat(64));
                assert_eq!(request["command"], expected_command);
                let result = match expected_command {
                    "get_version" => json!({
                        "protocol_version": 1,
                        "application_version": "test",
                        "capabilities": ["performance_diagnostics_v1", "performance_frame_timing_v1", "performance_drawable_presentation_events_v1"]
                    }),
                    "list_windows" => json!({"windows": [{"window_id": 1, "tab_count": 1}]}),
                    "performance_start" => json!({
                        "schema_version": 1,
                        "tab_id": 9,
                        "state": "started",
                        "metrics_scope": "webcontent_process"
                    }),
                    "performance_snapshot" | "performance_stop" => json!({
                        "schema_version": 1,
                        "tab_id": 9,
                        "state": if expected_command == "performance_stop" { "stopped" } else { "snapshot" },
                        "metrics": {
                            "schema_version": 1,
                            "scope": "webcontent_processes",
                            "processes": [{
                                "process_id": 17,
                                "scope": "webcontent_process",
                                "shared_with_other_views": false,
                                "available": true,
                                "diagnostics": {"schema_version": 1, "sample_count": 1}
                            }]
                        }
                    }),
                    "performance_frame_snapshot" => json!({
                        "schema_version": 1,
                        "tab_id": 9,
                        "metrics": {
                            "schema_version": 1,
                            "scope": "photon_tab_engine_surface_frames",
                            "frame_sample_fields": ["generation", "engine_frame_id"],
                            "frames": []
                        }
                    }),
                    _ => unreachable!(),
                };
                reply(&mut stream, &request, result, false);
            }
        });
        let mut client =
            PhotonAutomationClient::connect(&socket_path, &token_path, Duration::from_secs(1))
                .unwrap();
        assert!(client.supports("performance_diagnostics_v1"));
        assert!(client.supports("performance_drawable_presentation_events_v1"));
        client.performance_start(9, Duration::from_secs(1)).unwrap();
        let snapshot = client
            .performance_snapshot(9, Duration::from_secs(1))
            .unwrap();
        assert_eq!(snapshot["metrics"]["schema_version"], 1);
        assert_eq!(snapshot["metrics"]["processes"][0]["process_id"], 17);
        let stopped = client.performance_stop(9, Duration::from_secs(1)).unwrap();
        assert_eq!(stopped["metrics"]["processes"][0]["available"], true);
        let frames = client
            .performance_frame_snapshot(9, Duration::from_secs(1))
            .unwrap();
        assert_eq!(frames["metrics"]["frames"].as_array().unwrap().len(), 0);
        worker.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_response_with_unmatched_request_id() {
        let directory = private_temp_directory();
        let socket_path = directory.join("automation.sock");
        let token_path = directory.join("token");
        fs::write(&token_path, format!("{}\n", "d".repeat(64))).unwrap();
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = UnixListener::bind(&socket_path).unwrap();
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            reply(&mut stream, &request, json!({}), true);
        });
        let error = PhotonAutomationClient {
            socket_path,
            token: "d".repeat(64),
            request_id: 0,
            window_id: 0,
            application_version: String::new(),
            capabilities: Default::default(),
        }
        .call("ping", None, Duration::from_secs(1))
        .unwrap_err();
        worker.join().unwrap();
        assert!(error.to_string().contains("request ID did not match"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn preserves_bounded_protocol_error_for_uncorrelated_rejection() {
        let directory = private_temp_directory();
        let socket_path = directory.join("automation.sock");
        let token_path = directory.join("token");
        fs::write(&token_path, format!("{}\n", "e".repeat(64))).unwrap();
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = UnixListener::bind(&socket_path).unwrap();
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)).unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let response = json!({
                "protocol_version": 1,
                "request_id": null,
                "ok": false,
                "error": { "code": "malformed_request", "message": "could not read request" }
            });
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        });
        let error = PhotonAutomationClient {
            socket_path,
            token: "e".repeat(64),
            request_id: 0,
            window_id: 0,
            application_version: String::new(),
            capabilities: Default::default(),
        }
        .call("ping", None, Duration::from_secs(1))
        .unwrap_err();
        worker.join().unwrap();
        assert!(error.to_string().contains("request ID did not match"));
        assert!(error.to_string().contains("malformed_request"));
        assert!(error.to_string().contains("could not read request"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn speedometer_title_parser_accepts_only_structured_observer_payloads() {
        assert!(parse_speedometer_result_title(None).unwrap().is_none());
        let status = parse_speedometer_result_title(Some(
            "VelocitySpeedometer3.1:{\"state\":\"complete\",\"version\":\"3.1\",\"score\":12.5,\"internal_scores\":[12.0,13.0],\"viewport_warning\":true}",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(status.state, "complete");
        assert_eq!(status.score, Some(12.5));
        assert_eq!(status.internal_scores, vec![12.0, 13.0]);
        assert!(status.viewport_warning);
        let running = parse_speedometer_result_title(Some(
            "VelocitySpeedometer3.1:{\"state\":\"running\",\"version\":\"3.1\",\"document_generation\":4,\"current_suite\":\"TodoMVC-JavaScript-ES5\",\"current_test\":\"Adding100Items\",\"current_iteration\":10,\"completed_tests\":522,\"total_tests\":580,\"benchmark_running\":true,\"document_visibility_state\":\"hidden\"}",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            running.current_suite.as_deref(),
            Some("TodoMVC-JavaScript-ES5")
        );
        assert_eq!(running.document_generation, Some(4));
        assert_eq!(running.current_test.as_deref(), Some("Adding100Items"));
        assert_eq!(running.current_iteration, Some(10));
        assert_eq!(running.completed_tests, Some(522));
        assert_eq!(running.benchmark_running, Some(true));
        assert_eq!(running.document_visibility_state.as_deref(), Some("hidden"));
        let frame = parse_speedometer_result_title(Some(concat!(
            "VelocitySpeedometer3.1:{\"state\":\"running\",\"version\":\"3.1\",",
            "\"runner_is_running\":true,\"runner_finished_test_count\":3,",
            "\"runner_marks\":[\"suite-TodoMVC-JavaScript-ES5-prepare-start\"],",
            "\"document_visibility_state\":\"visible\",",
            "\"test_frame_src\":\"resources/todomvc/web-components/index.html\",",
            "\"test_frame_location\":\"http://127.0.0.1:8080/resources/todomvc/web-components/index.html\",",
            "\"test_frame_ready_state\":\"complete\",\"test_frame_visibility_state\":\"visible\",",
            "\"test_frame_has_todo_app\":true,\"test_frame_has_new_todo\":false,",
            "\"test_frame_boot_diagnostics\":\"body=1;inputs=1;class=0;selector=false;scripts=app.bundle.js:loaded=true\",",
            "\"test_frame_onload_handler\":true}"
        )))
        .unwrap()
        .unwrap();
        assert_eq!(frame.test_frame_ready_state.as_deref(), Some("complete"));
        assert_eq!(frame.runner_finished_test_count, Some(3));
        assert_eq!(frame.runner_marks.len(), 1);
        assert_eq!(frame.test_frame_has_todo_app, Some(true));
        assert_eq!(
            frame.test_frame_boot_diagnostics.as_deref(),
            Some("body=1;inputs=1;class=0;selector=false;scripts=app.bundle.js:loaded=true")
        );
        assert_eq!(frame.test_frame_onload_handler, Some(true));
        assert!(parse_speedometer_result_title(Some("VelocitySpeedometer3.1:not-json")).is_err());
    }

    #[test]
    fn jetstream_title_parser_accepts_bounded_structured_state() {
        assert!(parse_jetstream_result_title(None).unwrap().is_none());
        let status = parse_jetstream_result_title(Some(concat!(
            "VelocityJetStream3.0:{\"state\":\"complete\",\"initialized\":true,",
            "\"started\":true,\"completed\":true,\"failed\":false,",
            "\"official_score\":71.23,\"completed_workloads\":77,\"total_workloads\":77,",
            "\"subtest_scores\":{\"acorn-wtb\":12.5},\"errors\":[],",
            "\"document_generation\":9,\"document_url\":\"http://127.0.0.1:1234/JetStream3.0/index.html\"}"
        )))
        .unwrap()
        .unwrap();
        assert!(status.completed);
        assert_eq!(status.official_score, Some(71.23));
        assert_eq!(status.total_workloads, 77);
        assert_eq!(status.document_generation, Some(9));
        assert_eq!(status.subtest_scores.get("acorn-wtb"), Some(&12.5));
        assert!(parse_jetstream_result_title(Some("VelocityJetStream3.0:bad-json")).is_err());
    }

    #[test]
    fn tab_status_reads_engine_load_state_and_accepts_older_responses() {
        let loaded: PhotonTab = serde_json::from_value(json!({
            "tab_id": 7,
            "window_id": 3,
            "url": "https://browserbench.org/Speedometer3.1/",
            "title": "Speedometer 3.1",
            "active": true,
            "loading": false,
            "navigation_state": "idle",
            "error_state": null,
            "document_ready": null,
            "engine_load_finished": true,
            "document_generation": 4,
            "window_count": 1,
            "tab_count": 1
        }))
        .unwrap();
        assert_eq!(loaded.engine_load_finished, Some(true));
        assert_eq!(loaded.document_generation, Some(4));
        assert_eq!(loaded.window_count, Some(1));
        assert_eq!(loaded.tab_count, Some(1));
        assert_eq!(loaded.navigation_state.as_deref(), Some("idle"));
        assert_eq!(loaded.error_state, None);

        let older: PhotonTab = serde_json::from_value(json!({
            "tab_id": 7,
            "window_id": 3,
            "active": true,
            "loading": false,
            "document_ready": null
        }))
        .unwrap();
        assert_eq!(older.engine_load_finished, None);
        assert_eq!(older.document_generation, None);
    }
}
