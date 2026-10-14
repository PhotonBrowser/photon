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
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            url.strip_prefix(&format!("http://{}", server.address))
                .unwrap()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("window.__velocityReady=true"));
        assert!(response.contains("Velocity simple fixture"));
        server
            .wait_for_navigation_with_cancel("unit-run", &[0], Duration::from_millis(20), || false)
            .unwrap();
        let mut beacon = TcpStream::connect(server.address).unwrap();
        write!(beacon, "GET /__velocity/ready?run=unit-run&tab=0 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
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
