use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use velocity_core::FixtureKind;

const SIMPLE: &str = include_str!("../../../fixtures/simple/index.html");
const COMPLEX: &str = include_str!("../../../fixtures/complex/index.html");
const DYNAMIC: &str = include_str!("../../../fixtures/dynamic/index.html");
const TASK_BACKLOG: &str = include_str!("../../../fixtures/responsiveness/task-backlog.html");
const GC_PRESSURE: &str = include_str!("../../../fixtures/responsiveness/gc-pressure.html");
const DOM_STYLE: &str = include_str!("../../../fixtures/responsiveness/dom-style.html");
const STYLE_INVALIDATION: &str =
    include_str!("../../../fixtures/responsiveness/style-invalidation.html");
const LAYOUT_UPDATES: &str = include_str!("../../../fixtures/responsiveness/layout-updates.html");
const ANIMATION: &str = include_str!("../../../fixtures/responsiveness/animation.html");

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReadyKey {
    token: String,
    tab: usize,
}

#[derive(Default)]
struct FixtureState {
    navigated: BTreeSet<ReadyKey>,
    ready: BTreeSet<ReadyKey>,
}

pub struct FixtureServer {
    address: SocketAddr,
    state: Arc<Mutex<FixtureState>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl FixtureServer {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("cannot start local fixture server: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot configure local fixture server: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("cannot read local fixture server address: {error}"))?;
        let state = Arc::new(Mutex::new(FixtureState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("velocity-fixture-http".to_owned())
            .spawn(move || {
                while !worker_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => handle_request(stream, &worker_state),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(10)),
                    }
                }
            })
            .map_err(|error| format!("cannot start fixture HTTP worker: {error}"))?;
        Ok(Self {
            address,
            state,
            stop,
            worker: Some(worker),
        })
    }

    pub fn url(
        &self,
        fixture: FixtureKind,
        token: &str,
        tab: usize,
        intensity: u32,
        update_interval_ms: u64,
    ) -> String {
        format!(
            "http://{}:{}/{}?run={token}&tab={tab}&intensity={intensity}&update_ms={update_interval_ms}",
            self.address.ip(),
            self.address.port(),
            fixture.name()
        )
    }

    pub fn wait_for_ready_with_cancel(
        &self,
        token: &str,
        tabs: &[usize],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(), String> {
        self.wait_for_signal(
            token,
            tabs,
            timeout,
            "readiness",
            |state, key| state.ready.contains(key),
            cancelled,
        )
    }

    pub fn wait_for_navigation_with_cancel(
        &self,
        token: &str,
        tabs: &[usize],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(), String> {
        self.wait_for_signal(
            token,
            tabs,
            timeout,
            "navigation",
            |state, key| state.navigated.contains(key),
            cancelled,
        )
    }

    fn wait_for_signal(
        &self,
        token: &str,
        tabs: &[usize],
        timeout: Duration,
        signal: &str,
        contains: impl Fn(&FixtureState, &ReadyKey) -> bool,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let state = self
                .state
                .lock()
                .map_err(|_| "fixture state lock was poisoned".to_owned())?;
            if tabs.iter().all(|tab| {
                contains(
                    &state,
                    &ReadyKey {
                        token: token.to_owned(),
                        tab: *tab,
                    },
                )
            }) {
                return Ok(());
            }
            drop(state);
            if cancelled() {
                return Err(format!("fixture {signal} wait was cancelled"));
            }
            if Instant::now() >= deadline {
                let missing = tabs
                    .iter()
                    .filter(|tab| {
                        self.state.lock().is_ok_and(|state| {
                            !contains(
                                &state,
                                &ReadyKey {
                                    token: token.to_owned(),
                                    tab: **tab,
                                },
                            )
                        })
                    })
                    .map(usize::to_string)
                    .collect::<Vec<_>>();
                return Err(format!(
                    "fixture {signal} timed out for tab(s) {} within {} seconds",
                    missing.join(", "),
                    timeout.as_secs_f64()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn handle_request(mut stream: TcpStream, state: &Mutex<FixtureState>) {
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    while request.len() < 16_384 && !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(count) => request.extend_from_slice(&buffer[..count]),
        }
    }
    let request_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let Some(path) = request_line.split_whitespace().nth(1) else {
        respond(&mut stream, 400, "text/plain", "bad request");
        return;
    };
    if let Some(query) = path.strip_prefix("/__velocity/ready?") {
        if let Some((token, tab)) = parse_ready_query(query)
            && let Ok(mut state) = state.lock()
        {
            state.ready.insert(ReadyKey { token, tab });
            respond(&mut stream, 204, "text/plain", "");
            return;
        }
        respond(&mut stream, 400, "text/plain", "invalid readiness token");
        return;
    }
    let Some((route, query)) = path.trim_start_matches('/').split_once('?') else {
        respond(&mut stream, 404, "text/plain", "fixture not found");
        return;
    };
    let query = parse_query(query);
    let Some(token) = query.get("run") else {
        respond(&mut stream, 400, "text/plain", "missing run token");
        return;
    };
    let Some(tab) = query
        .get("tab")
        .and_then(|value| value.parse::<usize>().ok())
    else {
        respond(&mut stream, 400, "text/plain", "missing tab identifier");
        return;
    };
    let Some(fixture) = fixture_kind(route.trim_end_matches('/')) else {
        respond(&mut stream, 404, "text/plain", "fixture not found");
        return;
    };
    if let Ok(mut state) = state.lock() {
        state.navigated.insert(ReadyKey {
            token: token.clone(),
            tab,
        });
    }
    let body = render_fixture(fixture, token, tab, &query);
    respond(&mut stream, 200, "text/html; charset=utf-8", &body);
}

fn fixture_kind(path: &str) -> Option<FixtureKind> {
    match path {
        "simple" => Some(FixtureKind::Simple),
        "complex" => Some(FixtureKind::Complex),
        "dynamic" => Some(FixtureKind::Dynamic),
        "task-backlog" => Some(FixtureKind::TaskBacklog),
        "gc-pressure" => Some(FixtureKind::GcPressure),
        "dom-style" => Some(FixtureKind::DomStyle),
        "style-localized" => Some(FixtureKind::StyleLocalized),
        "style-descendant" => Some(FixtureKind::StyleDescendant),
        "style-sibling" => Some(FixtureKind::StyleSibling),
        "style-relational" => Some(FixtureKind::StyleRelational),
        "layout-updates" => Some(FixtureKind::LayoutUpdates),
        "animation" => Some(FixtureKind::Animation),
        _ => None,
    }
}

fn render_fixture(
    fixture: FixtureKind,
    token: &str,
    tab: usize,
    query: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut html = match fixture {
        FixtureKind::Simple => SIMPLE.to_owned(),
        FixtureKind::Complex => {
            let cards = (0..128)
                .map(|index| {
                    format!(
                        "<article><h2>Card {index}</h2><p>Deterministic fixture content {index:03}.</p><div class=\"bar\" style=\"width:{}%\"></div><small>row {}</small></article>",
                        25 + (index * 37 % 75),
                        index % 16
                    )
                })
                .collect::<String>();
            COMPLEX.replace("<!-- VELOCITY_COMPLEX_CARDS -->", &cards)
        }
        FixtureKind::Dynamic => DYNAMIC.to_owned(),
        FixtureKind::TaskBacklog => TASK_BACKLOG.to_owned(),
        FixtureKind::GcPressure => GC_PRESSURE.to_owned(),
        FixtureKind::DomStyle => DOM_STYLE.to_owned(),
        FixtureKind::StyleLocalized => {
            STYLE_INVALIDATION.replace("__VELOCITY_STYLE_SCENARIO__", "localized")
        }
        FixtureKind::StyleDescendant => {
            STYLE_INVALIDATION.replace("__VELOCITY_STYLE_SCENARIO__", "descendant")
        }
        FixtureKind::StyleSibling => {
            STYLE_INVALIDATION.replace("__VELOCITY_STYLE_SCENARIO__", "sibling")
        }
        FixtureKind::StyleRelational => {
            STYLE_INVALIDATION.replace("__VELOCITY_STYLE_SCENARIO__", "relational")
        }
        FixtureKind::LayoutUpdates => LAYOUT_UPDATES.to_owned(),
        FixtureKind::Animation => ANIMATION.replace(
            "__VELOCITY_ANIMATION_PREROLL_MS__",
            &velocity_core::ANIMATION_PRE_ROLL_MS.to_string(),
        ),
    };
    if fixture == FixtureKind::Dynamic {
        let intensity = query.get("intensity").map(String::as_str).unwrap_or("25");
        let update_ms = query.get("update_ms").map(String::as_str).unwrap_or("100");
        html = html.replace(
            "const intensity = Math.max(1, Math.min(10000, Number(query.get('intensity')) || 25));",
            &format!(
                "const intensity = Math.max(1, Math.min(10000, Number('{intensity}') || 25));"
            ),
        );
        html = html.replace(
            "const updateInterval = Math.max(10, Number(query.get('update_ms')) || 100);",
            &format!("const updateInterval = Math.max(10, Number('{update_ms}') || 100);"),
        );
    }
    let beacon = format!(
        "<script>window.__velocityReady=false;const velocityReady=()=>{{window.__velocityReady=true;fetch('/__velocity/ready?run={token}&tab={tab}',{{cache:'no-store'}}).catch(()=>{{}});}};if(document.readyState==='complete'){{velocityReady();}}else{{window.addEventListener('load',velocityReady,{{once:true}});}}</script>"
    );
    html.replace("</body>", &format!("{beacon}</body>"))
}

fn parse_query(query: &str) -> std::collections::BTreeMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn parse_ready_query(query: &str) -> Option<(String, usize)> {
    let query = parse_query(query);
    let token = query.get("run")?.clone();
    let tab = query.get("tab")?.parse().ok()?;
    (!token.is_empty()
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    .then_some((token, tab))
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_fixture_with_readiness_signal_and_stops_worker() {
        let server = FixtureServer::start().unwrap();
        let url = server.url(FixtureKind::Simple, "unit-run", 0, 25, 100);
        let mut stream = TcpStream::connect(server.address).unwrap();
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            url.strip_prefix(&format!("http://{}", server.address))
                .unwrap()
        );
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "unexpected fixture response: {response:?}"
        );
        assert!(response.contains("window.__velocityReady=true"));
        assert!(response.contains("Velocity simple fixture"));
        server
            .wait_for_navigation_with_cancel("unit-run", &[0], Duration::from_millis(20), || false)
            .unwrap();
        let mut beacon = TcpStream::connect(server.address).unwrap();
        beacon
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        beacon
            .write_all(b"GET /__velocity/ready?run=unit-run&tab=0 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        beacon.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 204"));
        server
            .wait_for_ready_with_cancel("unit-run", &[0], Duration::from_millis(20), || false)
            .unwrap();
        drop(server);
    }

    #[test]
    fn complex_fixture_has_fixed_card_count_and_dynamic_parameters_are_injected() {
        let complex = render_fixture(FixtureKind::Complex, "run", 1, &parse_query(""));
        assert_eq!(complex.matches("<article>").count(), 128);
        let dynamic = render_fixture(
            FixtureKind::Dynamic,
            "run",
            1,
            &parse_query("intensity=50&update_ms=250"),
        );
        assert!(dynamic.contains("Number('50')"));
        assert!(dynamic.contains("Number('250')"));
    }

    #[test]
    fn task_backlog_fixture_has_a_fixed_task_count_and_correctness_check() {
        let fixture = render_fixture(FixtureKind::TaskBacklog, "fixture-run", 2, &parse_query(""));
        assert!(fixture.contains("const taskCount = 512"));
        assert!(fixture.contains("setTimeout(() =>"));
        assert!(fixture.contains("data-processed"));
        assert!(
            fixture.contains("root.querySelectorAll(\".item.changed\").length === taskCount / 2")
        );
        assert!(fixture.contains("VelocityTaskBacklog:${completed}:${durationMs}:${correct}"));
        assert!(fixture.contains("encodeURIComponent(query.get(\"run\") || \"\")"));
        assert!(fixture.contains("/__velocity/ready?run=${encodeURIComponent"));
    }

    #[test]
    fn gc_pressure_fixture_has_fixed_allocation_counts_and_releases_retained_graphs() {
        let fixture = render_fixture(FixtureKind::GcPressure, "gc-run", 3, &parse_query(""));
        assert!(fixture.contains("const cycles = 32"));
        assert!(fixture.contains("const recordsPerCycle = 25_000"));
        assert!(fixture.contains("const expectedRecords = cycles * recordsPerCycle"));
        assert!(fixture.contains("retainedGraph.length = 0"));
        assert!(fixture.contains("setTimeout(() => {"));
        assert!(fixture.contains("VelocityGCPressure:"));
        assert!(!fixture.contains("gc()"));
        assert!(fixture.contains("run=gc-run&tab=3"));
    }

    #[test]
    fn dom_style_fixture_has_fixed_tree_and_exercises_selector_kinds() {
        let fixture = render_fixture(FixtureKind::DomStyle, "style-run", 4, &parse_query(""));
        assert!(fixture.contains("const branchCount = 16"));
        assert!(fixture.contains("const itemsPerBranch = 32"));
        assert!(fixture.contains("const mutationCount = 1024"));
        assert!(fixture.contains(".branch:has(input:checked)"));
        assert!(fixture.contains(".item.changed + .item"));
        assert!(fixture.contains("input:checked ~ .item.changed"));
        assert!(fixture.contains("VelocityDOMStyle:"));
        assert!(fixture.contains("run=style-run&tab=4"));
    }

    #[test]
    fn targeted_style_fixtures_select_one_dependency_family_and_keep_correctness_checks() {
        for (fixture_kind, scenario, selector) in [
            (FixtureKind::StyleLocalized, "localized", ".isolated.active"),
            (
                FixtureKind::StyleDescendant,
                "descendant",
                ".branch.active .item .leaf",
            ),
            (
                FixtureKind::StyleSibling,
                "sibling",
                ".item.sibling-active + .item",
            ),
            (
                FixtureKind::StyleRelational,
                "relational",
                ".branch:has(.selected) .target",
            ),
        ] {
            let fixture = render_fixture(fixture_kind, "style-run", 7, &parse_query(""));
            assert!(fixture.contains(&format!("const scenario = \"{scenario}\"")));
            assert!(fixture.contains(selector));
            assert!(fixture.contains("const mutationCount = 1024"));
            assert!(fixture.contains("const styleReadInterval = 65"));
            assert!(fixture.contains("(operation + 1) % styleReadInterval === 0"));
            assert!(fixture.contains("VelocityStyleInvalidation:"));
            assert!(!fixture.contains("__VELOCITY_STYLE_SCENARIO__"));
        }
    }

    #[test]
    fn layout_updates_fixture_has_fixed_flow_and_absolute_geometry_work() {
        let fixture = render_fixture(
            FixtureKind::LayoutUpdates,
            "layout-run",
            5,
            &parse_query(""),
        );
        assert!(fixture.contains("const normalFlowCount = 256"));
        assert!(fixture.contains("const absoluteCount = 128"));
        assert!(fixture.contains("const updateCount = 512"));
        assert!(fixture.contains("getBoundingClientRect()"));
        assert!(fixture.contains("VelocityLayoutUpdates:"));
        assert!(fixture.contains("run=layout-run&tab=5"));
    }

    #[test]
    fn animation_fixture_has_five_deterministic_phases_and_completion_metrics() {
        let fixture = render_fixture(FixtureKind::Animation, "animation-run", 6, &parse_query(""));
        assert!(fixture.contains("transform-run"));
        assert!(fixture.contains("opacity-run"));
        assert!(fixture.contains("requestAnimationFrame(frame)"));
        assert!(fixture.contains("const PHASE_COUNT = 5"));
        assert!(fixture.contains("const PRE_ROLL_MS = 900"));
        assert!(fixture.contains(
            "allocationChecksum += temporary[callbackCount % temporary.length].values[1]"
        ));
        assert!(fixture.contains("box.style.transform = `translateX("));
        assert!(!fixture.contains("__VELOCITY_ANIMATION_PREROLL_MS__"));
        assert!(fixture.contains("VelocityAnimation"));
        assert!(fixture.contains("run=animation-run&tab=6"));
    }

    #[test]
    fn readiness_events_are_bound_to_run_and_tab_identifiers() {
        assert_eq!(
            parse_ready_query("run=abc-123&tab=5"),
            Some(("abc-123".to_owned(), 5))
        );
        assert!(parse_ready_query("run=../other&tab=0").is_none());
        assert!(parse_ready_query("tab=0").is_none());
    }

    #[test]
    fn navigation_wait_honours_cancellation_and_the_fixture_server_drops() {
        let server = FixtureServer::start().unwrap();
        let error = server
            .wait_for_navigation_with_cancel("cancelled-run", &[0], Duration::from_secs(30), || {
                true
            })
            .unwrap_err();
        assert_eq!(error, "fixture navigation wait was cancelled");
        drop(server);
    }
}
