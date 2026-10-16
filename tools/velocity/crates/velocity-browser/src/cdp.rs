use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::TcpStream,
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// Official Speedometer result fields read directly from its benchmark client.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedometerPageStatus {
    pub url: String,
    pub document_ready: bool,
    pub benchmark_version: Option<String>,
    pub section: Option<String>,
    pub score: Option<f64>,
    pub internal_scores: Vec<f64>,
    pub failed_message: Option<String>,
    pub viewport_warning: bool,
}

/// State published by the fixed JetStream 3.0 page observer.
#[derive(Clone, Debug, PartialEq)]
pub struct JetStreamPageStatus {
    pub url: String,
    pub document_ready: bool,
    pub document_visibility_state: Option<String>,
    pub initialized: bool,
    pub started: bool,
    pub completed: bool,
    pub failed: bool,
    pub status_text: String,
    pub official_score: Option<f64>,
    pub completed_workloads: usize,
    pub total_workloads: usize,
    pub current_workload: Option<String>,
    pub errors: Vec<String>,
    pub subtest_scores: BTreeMap<String, f64>,
}

impl JetStreamPageStatus {
    pub fn ready(&self) -> bool {
        self.document_ready
            && self.document_visibility_state.as_deref() == Some("visible")
            && self.initialized
    }
}

impl SpeedometerPageStatus {
    pub fn started(&self) -> bool {
        self.document_ready && self.section.as_deref() == Some("running")
    }

    pub fn completed(&self) -> bool {
        self.document_ready && self.section.as_deref() == Some("summary")
    }
}

use serde_json::{Value, json};
use tungstenite::{Message, WebSocket, client::IntoClientRequest};

use crate::{AutomationProtocol, BrowserAutomationAdapter};

/// A minimal synchronous Chrome DevTools Protocol client used only for
/// benchmark-owned Chromium sessions with a private profile.
pub struct ChromiumDevTools {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    page_targets: BTreeSet<String>,
    target_sessions: BTreeMap<String, String>,
    page_diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutomationError(pub String);

impl std::fmt::Display for AutomationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AutomationError {}

impl ChromiumDevTools {
    /// Connect through the loopback-only endpoint advertised in the isolated
    /// Chromium profile's DevToolsActivePort file.
    pub fn connect(profile: &Path, timeout: Duration) -> Result<Self, AutomationError> {
        Self::connect_with_cancel(profile, timeout, || false)
    }

    pub fn connect_with_cancel(
        profile: &Path,
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Self, AutomationError> {
        let active_port = profile.join("DevToolsActivePort");
        let deadline = Instant::now() + timeout;
        let (port, path) = loop {
            if cancelled() {
                return Err(AutomationError(
                    "Chromium DevTools connection was cancelled".to_owned(),
                ));
            }
            if let Ok(contents) = fs::read_to_string(&active_port)
                && let Some(endpoint) = parse_active_port(&contents)
            {
                break endpoint;
            }
            if Instant::now() >= deadline {
                return Err(AutomationError(format!(
                    "Chromium did not publish a valid DevToolsActivePort file at {} within {} seconds",
                    active_port.display(),
                    timeout.as_secs()
                )));
            }
            thread::sleep(Duration::from_millis(25));
        };
        let endpoint = format!("ws://127.0.0.1:{port}{path}");
        let request = endpoint
            .into_client_request()
            .map_err(|error| AutomationError(format!("invalid Chromium DevTools URL: {error}")))?;
        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|error| {
            AutomationError(format!(
                "cannot connect to Chromium DevTools on port {port}: {error}"
            ))
        })?;
        let read_timeout = Some(timeout.min(Duration::from_secs(5)));
        stream.set_read_timeout(read_timeout).map_err(|error| {
            AutomationError(format!("cannot configure DevTools timeout: {error}"))
        })?;
        stream.set_write_timeout(read_timeout).map_err(|error| {
            AutomationError(format!("cannot configure DevTools timeout: {error}"))
        })?;
        let (socket, _) = tungstenite::client(request, stream).map_err(|error| {
            AutomationError(format!("cannot connect to Chromium DevTools: {error}"))
        })?;
        Ok(Self {
            socket,
            next_id: 1,
            page_targets: BTreeSet::new(),
            target_sessions: BTreeMap::new(),
            page_diagnostics: Vec::new(),
        })
    }

    /// Populate exactly `urls.len()` page targets in the fresh private
    /// profile. The one initial new-tab target is reused; further tabs are
    /// created by CDP and verified by querying target ownership.
    pub fn open_tabs(&mut self, urls: &[String]) -> Result<usize, AutomationError> {
        if urls.is_empty() {
            return Err(AutomationError(
                "a tab workload requires at least one URL".to_owned(),
            ));
        }
        let initial_targets = self.page_target_ids()?;
        if initial_targets.len() > 1 {
            return Err(AutomationError(format!(
                "fresh isolated Chromium session unexpectedly contains {} page targets; refusing an ambiguous tab workload",
                initial_targets.len()
            )));
        }
        if let Some(target_id) = initial_targets.first() {
            self.page_targets.insert(target_id.clone());
            let session_id = self.attach_and_navigate(target_id, &urls[0])?;
            self.target_sessions.insert(target_id.clone(), session_id);
        } else {
            BrowserAutomationAdapter::create_tab(self, &urls[0]).map_err(AutomationError)?;
        }
        for url in urls.iter().skip(1) {
            BrowserAutomationAdapter::create_tab(self, url).map_err(AutomationError)?;
        }
        let actual = self.page_target_ids()?;
        if actual.len() != urls.len()
            || self
                .page_targets
                .iter()
                .any(|target| !actual.contains(target))
        {
            return Err(AutomationError(format!(
                "CDP requested {} tabs but verified {} page targets in the managed browser session",
                urls.len(),
                actual.len()
            )));
        }
        Ok(actual.len())
    }

    fn command(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, AutomationError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| AutomationError("DevTools command identifier exhausted".to_owned()))?;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            message["sessionId"] = Value::String(session_id.to_owned());
        }
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(|error| AutomationError(format!("cannot send CDP {method}: {error}")))?;
        loop {
            let response = self.socket.read().map_err(|error| {
                AutomationError(format!("cannot read CDP {method} response: {error}"))
            })?;
            let Message::Text(response) = response else {
                continue;
            };
            let response: Value = serde_json::from_str(&response)
                .map_err(|error| AutomationError(format!("invalid CDP response: {error}")))?;
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                self.capture_page_diagnostic(&response);
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(AutomationError(format!("CDP {method} failed: {error}")));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Returns a bounded set of page errors observed on the managed target.
    /// Request query strings and bodies are never captured.
    pub fn page_diagnostics(&self) -> &[String] {
        &self.page_diagnostics
    }

    fn capture_page_diagnostic(&mut self, event: &Value) {
        const MAX_EVENTS: usize = 16;
        const MAX_MESSAGE_CHARS: usize = 384;
        if self.page_diagnostics.len() >= MAX_EVENTS {
            return;
        }
        let Some(method) = event.get("method").and_then(Value::as_str) else {
            return;
        };
        let params = event.get("params").unwrap_or(&Value::Null);
        let message = match method {
            "Runtime.exceptionThrown" => {
                let details = params.get("exceptionDetails").unwrap_or(&Value::Null);
                let text = details
                    .get("exception")
                    .and_then(|exception| exception.get("description"))
                    .and_then(Value::as_str)
                    .or_else(|| details.get("text").and_then(Value::as_str))
                    .unwrap_or("uncaught JavaScript exception");
                Some(format!("JavaScript exception: {text}"))
            }
            "Runtime.consoleAPICalled"
                if params.get("type").and_then(Value::as_str) == Some("error") =>
            {
                let text = params
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|args| {
                        args.iter()
                            .filter_map(|arg| {
                                arg.get("value")
                                    .and_then(Value::as_str)
                                    .or_else(|| arg.get("description").and_then(Value::as_str))
                            })
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                (!text.is_empty()).then(|| format!("JavaScript console error: {text}"))
            }
            "Log.entryAdded"
                if params
                    .get("entry")
                    .and_then(|entry| entry.get("level"))
                    .and_then(Value::as_str)
                    == Some("error") =>
            {
                params
                    .get("entry")
                    .and_then(|entry| entry.get("text"))
                    .and_then(Value::as_str)
                    .map(|text| format!("Browser log error: {text}"))
            }
            "Network.loadingFailed" => params
                .get("errorText")
                .and_then(Value::as_str)
                .map(|text| format!("Resource load failed: {text}")),
            "Network.responseReceived" => {
                let response = params.get("response").unwrap_or(&Value::Null);
                let status = response.get("status").and_then(Value::as_u64).unwrap_or(0);
                (status >= 400).then(|| {
                    let url = response
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown resource")
                        .split(['?', '#'])
                        .next()
                        .unwrap_or("unknown resource");
                    format!("Resource returned HTTP {status}: {url}")
                })
            }
            _ => None,
        };
        if let Some(message) = message {
            let bounded = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
            if !self.page_diagnostics.contains(&bounded) {
                self.page_diagnostics.push(bounded);
            }
        }
    }

    fn page_target_ids(&mut self) -> Result<Vec<String>, AutomationError> {
        let targets = self.command("Target.getTargets", json!({}), None)?;
        let targets = targets
            .get("targetInfos")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                AutomationError("CDP omitted Target.getTargets.targetInfos".to_owned())
            })?;
        Ok(targets
            .iter()
            .filter(|target| target.get("type").and_then(Value::as_str) == Some("page"))
            .filter_map(|target| {
                target
                    .get("targetId")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect())
    }

    fn attach_and_navigate(
        &mut self,
        target_id: &str,
        url: &str,
    ) -> Result<String, AutomationError> {
        let attached = self.command(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
            None,
        )?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| AutomationError("CDP did not return a target session ID".to_owned()))?
            .to_owned();
        self.command("Runtime.enable", json!({}), Some(&session_id))?;
        self.command("Log.enable", json!({}), Some(&session_id))?;
        self.command("Network.enable", json!({}), Some(&session_id))?;
        self.command("Page.enable", json!({}), Some(&session_id))?;
        self.command("Page.navigate", json!({ "url": url }), Some(&session_id))?;
        Ok(session_id)
    }

    /// Polls the fixture's fixed readiness property in one verified CDP page
    /// target. The expression is constant and does not accept caller code.
    pub fn wait_for_fixture_ready(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(), AutomationError> {
        let session_id = self
            .target_sessions
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                AutomationError(format!(
                    "CDP has no verified target session for fixture tab {target_id}"
                ))
            })?;
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            if cancelled() {
                return Err(AutomationError(
                    "Chromium fixture readiness wait was cancelled".to_owned(),
                ));
            }
            match self.command(
                "Runtime.evaluate",
                json!({
                    "expression": "({ url: window.location.href, ready: window.__velocityReady === true })",
                    "returnByValue": true
                }),
                Some(&session_id),
            ) {
                Ok(result) => {
                    let value = result
                        .get("result")
                        .and_then(|remote| remote.get("value"));
                    if value
                        .and_then(|value| value.get("url"))
                        .and_then(Value::as_str)
                        == Some(expected_url)
                        && value
                            .and_then(|value| value.get("ready"))
                            .and_then(Value::as_bool)
                            == Some(true)
                    {
                        return Ok(());
                    }
                }
                Err(error) => last_error = Some(error.to_string()),
            }
            thread::sleep(Duration::from_millis(20));
        }
        let detail = last_error.map_or_else(
            || "the tab did not expose the fixture ready signal".to_owned(),
            |error| format!("last CDP error: {error}"),
        );
        Err(AutomationError(format!(
            "Chromium fixture tab {target_id} did not become ready at the assigned URL within {} seconds ({detail})",
            timeout.as_secs_f64()
        )))
    }

    /// Reads only the official Speedometer client state and published score.
    /// This expression is fixed and intentionally does not accept browser-side
    /// code from CLI callers.
    pub fn speedometer_status(
        &mut self,
        target_id: &str,
    ) -> Result<SpeedometerPageStatus, AutomationError> {
        let session_id = self
            .target_sessions
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                AutomationError(format!(
                    "CDP has no verified target session for Speedometer page {target_id}"
                ))
            })?;
        let result = self.command(
            "Runtime.evaluate",
            json!({
                "expression": SPEEDOMETER_STATUS_EXPRESSION,
                "returnByValue": true,
                "awaitPromise": false
            }),
            Some(&session_id),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(AutomationError(format!(
                "Speedometer status evaluation failed: {exception}"
            )));
        }
        let value = result
            .get("result")
            .and_then(|remote| remote.get("value"))
            .ok_or_else(|| {
                AutomationError("Speedometer status response had no by-value result".to_owned())
            })?;
        parse_speedometer_status(value)
    }

    pub fn wait_for_speedometer_started(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<SpeedometerPageStatus, AutomationError> {
        self.wait_for_speedometer_condition(
            target_id,
            expected_url,
            timeout,
            "start",
            &mut cancelled,
            |s| s.started() || s.completed(),
        )
    }

    pub fn wait_for_speedometer_completed(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<SpeedometerPageStatus, AutomationError> {
        self.wait_for_speedometer_condition(
            target_id,
            expected_url,
            timeout,
            "completion",
            &mut cancelled,
            |s| s.completed(),
        )
    }

    /// Reads the fixed, read-only JetStream 3.0 status expression. Callers
    /// cannot supply JavaScript through this API.
    pub fn jetstream_status(
        &mut self,
        target_id: &str,
    ) -> Result<JetStreamPageStatus, AutomationError> {
        let session_id = self
            .target_sessions
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                AutomationError(format!(
                    "CDP has no verified target session for JetStream page {target_id}"
                ))
            })?;
        let result = self.command(
            "Runtime.evaluate",
            json!({
                "expression": JETSTREAM_STATUS_EXPRESSION,
                "returnByValue": true,
                "awaitPromise": false
            }),
            Some(&session_id),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(AutomationError(format!(
                "JetStream status evaluation failed: {exception}"
            )));
        }
        let value = result
            .get("result")
            .and_then(|remote| remote.get("value"))
            .ok_or_else(|| {
                AutomationError("JetStream status response had no by-value result".to_owned())
            })?;
        parse_jetstream_status(value)
    }

    /// Brings the verified benchmark page to the foreground before execution.
    pub fn bring_page_to_front(&mut self, target_id: &str) -> Result<(), AutomationError> {
        let session_id = self
            .target_sessions
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                AutomationError(format!(
                    "CDP has no verified target session for JetStream page {target_id}"
                ))
            })?;
        self.command("Page.bringToFront", json!({}), Some(&session_id))?;
        Ok(())
    }

    /// Activates JetStream's own Start Test control on the verified benchmark
    /// document without accepting JavaScript from the CLI caller.
    pub fn start_jetstream(&mut self, target_id: &str) -> Result<(), AutomationError> {
        let session_id = self
            .target_sessions
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                AutomationError(format!(
                    "CDP has no verified target session for JetStream page {target_id}"
                ))
            })?;
        let result = self.command(
            "Runtime.evaluate",
            json!({
                "expression": JETSTREAM_START_EXPRESSION,
                "returnByValue": true,
                "awaitPromise": false
            }),
            Some(&session_id),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(AutomationError(format!(
                "JetStream start request failed: {exception}"
            )));
        }
        let value = result
            .get("result")
            .and_then(|remote| remote.get("value"))
            .and_then(Value::as_object)
            .ok_or_else(|| {
                AutomationError("JetStream start response had no structured result".to_owned())
            })?;
        if value.get("started").and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        let reason = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("the official Start Test control was unavailable");
        Err(AutomationError(format!(
            "JetStream did not start through its official control: {reason}"
        )))
    }

    pub fn wait_for_jetstream_ready(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<JetStreamPageStatus, AutomationError> {
        self.wait_for_jetstream_condition(
            target_id,
            expected_url,
            timeout,
            "initialization",
            cancelled,
            |status| status.ready(),
        )
    }

    pub fn wait_for_jetstream_started(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<JetStreamPageStatus, AutomationError> {
        self.wait_for_jetstream_condition(
            target_id,
            expected_url,
            timeout,
            "start",
            cancelled,
            |status| status.started || status.completed,
        )
    }

    pub fn wait_for_jetstream_completed(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<JetStreamPageStatus, AutomationError> {
        self.wait_for_jetstream_condition(
            target_id,
            expected_url,
            timeout,
            "completion",
            cancelled,
            |status| status.completed,
        )
    }

    fn wait_for_jetstream_condition(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        phase: &str,
        mut cancelled: impl FnMut() -> bool,
        condition: impl Fn(&JetStreamPageStatus) -> bool,
    ) -> Result<JetStreamPageStatus, AutomationError> {
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            if cancelled() {
                return Err(AutomationError(format!(
                    "JetStream automation was cancelled during {phase}"
                )));
            }
            match self.jetstream_status(target_id) {
                Ok(status) => {
                    if !same_document_url(&status.url, expected_url) {
                        last_error = Some(format!(
                            "controlled page navigated away from expected JetStream document: {}",
                            status.url
                        ));
                    } else if status.failed {
                        let detail = status
                            .errors
                            .first()
                            .map(String::as_str)
                            .unwrap_or(status.status_text.as_str());
                        return Err(AutomationError(format!(
                            "JetStream reported a page or workload failure during {phase}: {detail}"
                        )));
                    } else if condition(&status) {
                        return Ok(status);
                    } else {
                        last_error = Some(format!(
                            "state={}, initialized={}, running={}, completed={}/{}, current={}",
                            status.status_text,
                            status.initialized,
                            status.started,
                            status.completed_workloads,
                            status.total_workloads,
                            status.current_workload.as_deref().unwrap_or("none")
                        ));
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if is_transport_disconnect(&message) {
                        return Err(AutomationError(format!(
                            "JetStream automation connection was lost during {phase}: {message}"
                        )));
                    }
                    last_error = Some(message);
                }
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err(AutomationError(format!(
            "JetStream did not reach verified {phase} state within {:.1} seconds ({})",
            timeout.as_secs_f64(),
            last_error.unwrap_or_else(|| "page remained in its loading state".to_owned())
        )))
    }

    fn wait_for_speedometer_condition(
        &mut self,
        target_id: &str,
        expected_url: &str,
        timeout: Duration,
        phase: &str,
        cancelled: &mut impl FnMut() -> bool,
        condition: impl Fn(&SpeedometerPageStatus) -> bool,
    ) -> Result<SpeedometerPageStatus, AutomationError> {
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            if cancelled() {
                return Err(AutomationError(
                    "Speedometer automation was cancelled".to_owned(),
                ));
            }
            match self.speedometer_status(target_id) {
                Ok(status) => {
                    if status.url != expected_url
                        && !status.url.starts_with(&format!("{expected_url}#"))
                    {
                        last_error = Some(format!(
                            "controlled page navigated away from expected URL: {}",
                            status.url
                        ));
                    } else if condition(&status) {
                        return Ok(status);
                    } else if status.completed() && status.failed_message.is_some() {
                        return Err(AutomationError(format!(
                            "Speedometer completed with an invalid result: {}",
                            status
                                .failed_message
                                .as_deref()
                                .unwrap_or("unknown benchmark error")
                        )));
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if is_transport_disconnect(&message) {
                        return Err(AutomationError(format!(
                            "Speedometer automation connection was lost during {phase}: {message}"
                        )));
                    }
                    last_error = Some(message);
                }
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err(AutomationError(format!(
            "Speedometer did not reach verified {phase} state within {:.1} seconds ({})",
            timeout.as_secs_f64(),
            last_error.unwrap_or_else(|| "page remained in its loading state".to_owned())
        )))
    }
}

fn is_transport_disconnect(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "connection reset",
        "connection refused",
        "connection aborted",
        "broken pipe",
        "connection closed",
        "unexpected eof",
        "early eof",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

const SPEEDOMETER_STATUS_EXPRESSION: &str = r##"(() => {
    const root = document.documentElement;
    const client = globalThis.benchmarkClient;
    const metrics = client && client.metrics;
    const scoreMetric = metrics && metrics.Score;
    const summary = document.querySelector("#summary");
    const invalid = summary && summary.classList.contains("invalid");
    const warning = document.querySelector("#screen-size-warning");
    return {
        url: window.location.href,
        document_ready: document.readyState === "complete",
        benchmark_version: document.querySelector(".version")?.textContent?.trim() || null,
        section: root?.getAttribute("data-visible-section") || null,
        score: scoreMetric && Number.isFinite(scoreMetric.mean) ? scoreMetric.mean : null,
        internal_scores: scoreMetric && Array.isArray(scoreMetric.values) ? scoreMetric.values : [],
        failed_message: invalid ? document.querySelector("#invalid-score-text")?.innerText?.trim() || "Speedometer reported an invalid score" : null,
        viewport_warning: !!warning && getComputedStyle(warning).display !== "none"
    };
})()"##;

const JETSTREAM_STATUS_EXPRESSION: &str = r##"(() => {
    const driver = globalThis.JetStream;
    const status = document.querySelector("#status");
    const summary = document.querySelector("#result-summary");
    const scoreText = summary?.querySelector(".score")?.textContent?.trim() || "";
    const score = Number(scoreText);
    const workloads = Array.isArray(driver?.benchmarks) ? driver.benchmarks : [];
    const completed = workloads.filter(benchmark =>
        document.getElementById(`benchmark-${benchmark.name}`)?.classList.contains("benchmark-done")
    ).length;
    const errors = Array.isArray(driver?.errors)
        ? driver.errors.slice(0, 8).map(error => String(error?.error || error).slice(0, 192))
        : [];
    const runningNode = workloads.find(benchmark =>
        document.getElementById(`benchmark-${benchmark.name}`)?.classList.contains("benchmark-running")
    );
    const failed = globalThis.allIsGood === false
        || status?.classList.contains("error") === true
        || document.querySelector(".benchmark-error") !== null
        || errors.length > 0;
    const finished = driver?.isDone === true
        && summary?.classList.contains("done") === true
        && workloads.length > 0
        && completed === workloads.length
        && Number.isFinite(score)
        && score > 0;
    const statusText = String(status?.innerText || status?.textContent || "").trim().slice(0, 512);
    const subtestScores = {};
    for (const benchmark of workloads) {
        const value = benchmark?.score;
        if (typeof benchmark?.name === "string" && Number.isFinite(value) && value > 0)
            subtestScores[benchmark.name] = value;
    }
    return {
        url: location.href,
        document_ready: document.readyState === "complete",
        document_visibility_state: document.visibilityState,
        initialized: driver?.isReady === true && globalThis.allIsGood !== false,
        started: statusText.includes("Running") || !!runningNode || finished,
        completed: finished,
        failed,
        status_text: statusText,
        official_score: Number.isFinite(score) && score > 0 ? score : null,
        completed_workloads: completed,
        total_workloads: workloads.length,
        current_workload: runningNode?.name || null,
        errors,
        subtest_scores: subtestScores,
    };
})()"##;

const JETSTREAM_START_EXPRESSION: &str = r##"(() => {
    const path = location.pathname;
    const officialHost = location.protocol === "https:" && location.hostname === "browserbench.org";
    const loopbackHost = location.protocol === "http:" && location.hostname === "127.0.0.1";
    const validPath = path === "/JetStream3.0" || path === "/JetStream3.0/" || path === "/JetStream3.0/index.html";
    if (!(officialHost || loopbackHost) || !validPath)
        return { started: false, error: "page is outside the official JetStream 3.0 route" };
    if (location.search.length)
        return { started: false, error: "JetStream URL contains non-default parameters" };
    const driver = globalThis.JetStream;
    const status = document.querySelector("#status");
    if (driver?.isReady !== true)
        return { started: false, error: "JetStream has not completed initialization" };
    if (driver.isDone)
        return { started: false, error: "JetStream has already completed" };
    if (!status || typeof status.onclick !== "function" || !status.querySelector("a.button"))
        return { started: false, error: "official Start Test control is unavailable" };
    status.click();
    return { started: true };
})()"##;

fn parse_speedometer_status(value: &Value) -> Result<SpeedometerPageStatus, AutomationError> {
    let url = value
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| AutomationError("Speedometer status omitted the page URL".to_owned()))?
        .to_owned();
    let internal_scores = value
        .get("internal_scores")
        .and_then(Value::as_array)
        .map(|scores| scores.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    Ok(SpeedometerPageStatus {
        url,
        document_ready: value
            .get("document_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        benchmark_version: value
            .get("benchmark_version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        section: value
            .get("section")
            .and_then(Value::as_str)
            .map(str::to_owned),
        score: value.get("score").and_then(Value::as_f64),
        internal_scores,
        failed_message: value
            .get("failed_message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        viewport_warning: value
            .get("viewport_warning")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_jetstream_status(value: &Value) -> Result<JetStreamPageStatus, AutomationError> {
    let url = value
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| AutomationError("JetStream status omitted the page URL".to_owned()))?
        .to_owned();
    let errors: Vec<String> = value
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .take(8)
                .map(|error| error.chars().take(192).collect())
                .collect()
        })
        .unwrap_or_default();
    let subtest_scores = value
        .get("subtest_scores")
        .and_then(Value::as_object)
        .map(|scores| {
            scores
                .iter()
                .filter_map(|(name, score)| {
                    score
                        .as_f64()
                        .filter(|score| score.is_finite() && *score > 0.0)
                        .map(|score| (name.clone(), score))
                })
                .collect()
        })
        .unwrap_or_default();
    let official_score = value
        .get("official_score")
        .and_then(Value::as_f64)
        .filter(|score| score.is_finite() && *score > 0.0);
    let completed_workloads = value
        .get("completed_workloads")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let total_workloads = value
        .get("total_workloads")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let failed = value
        .get("failed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || !errors.is_empty();
    let completed = value
        .get("completed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && total_workloads > 0
        && completed_workloads == total_workloads
        && official_score.is_some()
        && !failed;
    Ok(JetStreamPageStatus {
        url,
        document_ready: value
            .get("document_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        document_visibility_state: value
            .get("document_visibility_state")
            .and_then(Value::as_str)
            .map(str::to_owned),
        initialized: value
            .get("initialized")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        started: value
            .get("started")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        completed,
        failed,
        status_text: value
            .get("status_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(512)
            .collect(),
        official_score,
        completed_workloads,
        total_workloads,
        current_workload: value
            .get("current_workload")
            .and_then(Value::as_str)
            .map(str::to_owned),
        errors,
        subtest_scores,
    })
}

fn same_document_url(actual: &str, expected: &str) -> bool {
    let actual = actual.split(['?', '#']).next().unwrap_or_default();
    let expected = expected.split(['?', '#']).next().unwrap_or_default();
    actual.trim_end_matches('/') == expected.trim_end_matches('/')
}

impl BrowserAutomationAdapter for ChromiumDevTools {
    fn protocol(&self) -> AutomationProtocol {
        AutomationProtocol::ChromeDevToolsProtocol
    }

    fn navigate_initial_page(&mut self, url: &str) -> Result<String, String> {
        let targets = self.page_target_ids().map_err(|error| error.to_string())?;
        let target = targets
            .first()
            .ok_or_else(|| "Chromium has no initial page target".to_owned())?;
        self.page_targets.insert(target.clone());
        let session_id = self
            .attach_and_navigate(target, url)
            .map_err(|error| error.to_string())?;
        self.target_sessions.insert(target.clone(), session_id);
        Ok(target.clone())
    }

    fn create_tab(&mut self, url: &str) -> Result<String, String> {
        let created = self
            .command(
                "Target.createTarget",
                json!({ "url": "about:blank", "newWindow": false }),
                None,
            )
            .map_err(|error| error.to_string())?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "CDP did not return a new target ID".to_owned())?
            .to_owned();
        self.page_targets.insert(target_id.clone());
        let session_id = self
            .attach_and_navigate(&target_id, url)
            .map_err(|error| error.to_string())?;
        self.target_sessions.insert(target_id.clone(), session_id);
        let actual = self.page_target_ids().map_err(|error| error.to_string())?;
        if !actual.contains(&target_id) {
            return Err(
                "CDP created a target that is not present in the managed browser".to_owned(),
            );
        }
        Ok(target_id)
    }

    fn page_count(&mut self) -> Result<usize, String> {
        self.page_target_ids()
            .map(|targets| targets.len())
            .map_err(|error| error.to_string())
    }
}

fn parse_active_port(contents: &str) -> Option<(u16, String)> {
    let mut lines = contents.lines();
    let port = lines.next()?.parse::<u16>().ok()?;
    let path = lines.next()?;
    if port == 0 || !path.starts_with("/devtools/browser/") || path.contains("..") {
        return None;
    }
    Some((port, path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_official_speedometer_completion_without_recomputing_score() {
        let status = parse_speedometer_status(&json!({
            "url": "https://browserbench.org/Speedometer3.1/#summary",
            "document_ready": true,
            "benchmark_version": "3.1",
            "section": "summary",
            "score": 18.75,
            "internal_scores": [18.0, 19.5],
            "viewport_warning": false,
        }))
        .unwrap();

        assert!(status.completed());
        assert_eq!(status.score, Some(18.75));
        assert_eq!(status.internal_scores, vec![18.0, 19.5]);
        assert!(!status.viewport_warning);
    }

    #[test]
    fn rejects_speedometer_status_without_a_page_url() {
        let error = parse_speedometer_status(&json!({"section": "summary"})).unwrap_err();
        assert!(error.to_string().contains("omitted the page URL"));
    }

    #[test]
    fn parses_jetstream_completion_and_official_subtest_scores() {
        let status = parse_jetstream_status(&json!({
            "url": "http://127.0.0.1:43123/JetStream3.0/index.html?startAutomatically",
            "document_ready": true,
            "document_visibility_state": "visible",
            "initialized": true,
            "started": true,
            "completed": true,
            "failed": false,
            "status_text": "",
            "official_score": 71.23,
            "completed_workloads": 77,
            "total_workloads": 77,
            "current_workload": null,
            "errors": [],
            "subtest_scores": {"acorn-wtb": 12.5, "bad": null},
        }))
        .unwrap();

        assert!(status.ready());
        assert!(status.started);
        assert!(status.completed);
        assert!(!status.failed);
        assert_eq!(status.official_score, Some(71.23));
        assert_eq!(status.completed_workloads, 77);
        assert_eq!(status.total_workloads, 77);
        assert_eq!(status.subtest_scores.get("acorn-wtb"), Some(&12.5));
    }

    #[test]
    fn jetstream_readiness_requires_a_visible_document() {
        let hidden = parse_jetstream_status(&json!({
            "url": "http://127.0.0.1:43123/JetStream3.0/index.html",
            "document_ready": true,
            "document_visibility_state": "hidden",
            "initialized": true,
        }))
        .unwrap();

        assert!(!hidden.ready());
        assert_eq!(hidden.document_visibility_state.as_deref(), Some("hidden"));
    }

    #[test]
    fn jetstream_status_bounds_errors_and_rejects_unusable_score_values() {
        let status = parse_jetstream_status(&json!({
            "url": "https://browserbench.org/JetStream3.0/index.html",
            "initialized": false,
            "failed": true,
            "errors": ["x".repeat(500)],
            "subtest_scores": {"invalid": -1, "nan": null},
        }))
        .unwrap();
        assert!(status.failed);
        assert_eq!(status.errors[0].len(), 192);
        assert!(status.subtest_scores.is_empty());
        assert_eq!(status.official_score, None);
    }

    #[test]
    fn jetstream_completion_requires_the_complete_official_workload_set() {
        let incomplete = parse_jetstream_status(&json!({
            "url": "https://browserbench.org/JetStream3.0/index.html",
            "completed": true,
            "failed": false,
            "official_score": 71.23,
            "completed_workloads": 76,
            "total_workloads": 77,
        }))
        .unwrap();
        assert!(!incomplete.completed);

        assert!(same_document_url(
            "https://browserbench.org/JetStream3.0/index.html?startAutomatically",
            "https://browserbench.org/JetStream3.0/index.html"
        ));
        assert!(!same_document_url(
            "https://browserbench.org/JetStream3.0/index.html",
            "https://example.com/JetStream3.0/index.html"
        ));
    }

    #[test]
    fn distinguishes_a_closed_devtools_connection_from_a_timed_out_read() {
        assert!(is_transport_disconnect("connection reset by peer"));
        assert!(is_transport_disconnect("WebSocket: Broken pipe"));
        assert!(!is_transport_disconnect("Resource temporarily unavailable"));
    }

    #[test]
    fn active_port_requires_loopback_browser_endpoint_path() {
        assert_eq!(
            parse_active_port("9222\n/devtools/browser/session\n"),
            Some((9222, "/devtools/browser/session".to_owned()))
        );
        assert!(parse_active_port("0\n/devtools/browser/session").is_none());
        assert!(parse_active_port("9222\n/ws/foreign").is_none());
        assert!(parse_active_port("9222\n/devtools/browser/../other").is_none());
    }
}
