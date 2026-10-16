use std::{
    fs::{self, File},
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

pub const JETSTREAM_VERSION: &str = "3.0";
pub const JETSTREAM_COMMIT: &str = "06785cf861ac44855f168cbbe829278c2802e6de";
pub const JETSTREAM_ARCHIVE_SHA256: &str =
    "0361851eb2ddeb003e06be74239c3f7ce437c64e774a29ba446faaed9ab16fa9";
const ARCHIVE_URL: &str =
    "https://codeload.github.com/WebKit/JetStream/tar.gz/06785cf861ac44855f168cbbe829278c2802e6de";
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const SERVER_WORKERS: usize = 8;
const MOUNT_PREFIX: &str = "/JetStream3.0/";

pub fn official_url() -> String {
    "https://browserbench.org/JetStream3.0/index.html".to_owned()
}

pub fn local_cache_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/Caches/Velocity/jetstream")
        .join(format!("{JETSTREAM_VERSION}-{JETSTREAM_COMMIT}")))
}

pub fn setup_jetstream_source() -> Result<PathBuf, String> {
    let destination = local_cache_path()?;
    if is_prepared_source(&destination) {
        return Ok(destination);
    }
    let cache_root = destination
        .parent()
        .ok_or_else(|| "JetStream cache path has no parent".to_owned())?;
    fs::create_dir_all(cache_root).map_err(|error| {
        format!(
            "cannot create JetStream cache {}: {error}",
            cache_root.display()
        )
    })?;
    set_private_directory_permissions(cache_root)?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = cache_root.join(format!(".setup-{}-{nonce}", std::process::id()));
    fs::create_dir(&temporary).map_err(|error| {
        format!(
            "cannot create temporary JetStream setup directory {}: {error}",
            temporary.display()
        )
    })?;
    let _temporary_cleanup = RemoveTemporaryDirectory(temporary.clone());
    set_private_directory_permissions(&temporary)?;
    let archive = temporary.join("jetstream.tar.gz");
    let max_filesize = MAX_ARCHIVE_BYTES.to_string();
    let download = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-filesize",
            &max_filesize,
            "--output",
        ])
        .arg(&archive)
        .arg(ARCHIVE_URL)
        .status()
        .map_err(|error| format!("cannot start curl to fetch official JetStream: {error}"))?;
    if !download.success() {
        return Err("could not download the pinned official JetStream source archive".to_owned());
    }
    if fs::metadata(&archive)
        .map_err(|error| format!("cannot inspect JetStream archive: {error}"))?
        .len()
        > MAX_ARCHIVE_BYTES
    {
        return Err("JetStream source archive exceeds the 256 MiB size limit".to_owned());
    }
    let actual_digest = sha256_file(&archive)?;
    if actual_digest != JETSTREAM_ARCHIVE_SHA256 {
        return Err(format!(
            "official JetStream archive checksum mismatch: expected {JETSTREAM_ARCHIVE_SHA256}, received {actual_digest}"
        ));
    }

    let extracted = temporary.join("source");
    fs::create_dir(&extracted).map_err(|error| {
        format!(
            "cannot create JetStream extraction directory {}: {error}",
            extracted.display()
        )
    })?;
    let extraction = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&extracted)
        .args([
            "--strip-components=1",
            "--no-same-owner",
            "--no-same-permissions",
        ])
        .status()
        .map_err(|error| format!("cannot start tar to unpack JetStream: {error}"))?;
    if !extraction.success() {
        return Err("could not unpack the verified JetStream source archive".to_owned());
    }
    if !has_jetstream_entry(&extracted) {
        return Err("pinned JetStream archive is missing required browser assets".to_owned());
    }

    let marker = serde_json::json!({
        "version": JETSTREAM_VERSION,
        "commit": JETSTREAM_COMMIT,
        "archive_sha256": JETSTREAM_ARCHIVE_SHA256,
    });
    fs::write(
        extracted.join(".velocity-jetstream.json"),
        serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("cannot write JetStream cache metadata: {error}"))?;
    set_private_directory_permissions(&extracted)?;

    if destination.exists() {
        return Err(format!(
            "JetStream cache path already exists but did not validate: {}",
            destination.display()
        ));
    }
    fs::rename(&extracted, &destination).map_err(|error| {
        format!(
            "cannot install JetStream cache at {}: {error}",
            destination.display()
        )
    })?;
    Ok(destination)
}

pub fn require_prepared_source() -> Result<PathBuf, String> {
    let path = local_cache_path()?;
    if is_prepared_source(&path) {
        Ok(path)
    } else {
        Err(format!(
            "the pinned local JetStream source is not installed; run 'velocity setup jetstream' (expected {})",
            path.display()
        ))
    }
}

fn is_prepared_source(path: &Path) -> bool {
    if !has_jetstream_entry(path) {
        return false;
    }
    fs::read(path.join(".velocity-jetstream.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|value| {
            value.get("version").and_then(serde_json::Value::as_str) == Some(JETSTREAM_VERSION)
                && value.get("commit").and_then(serde_json::Value::as_str) == Some(JETSTREAM_COMMIT)
                && value
                    .get("archive_sha256")
                    .and_then(serde_json::Value::as_str)
                    == Some(JETSTREAM_ARCHIVE_SHA256)
        })
}

fn has_jetstream_entry(path: &Path) -> bool {
    path.join("index.html").is_file()
        && path.join("JetStreamDriver.js").is_file()
        && path.join("utils/params.js").is_file()
        && path.join("resources/JetStream.css").is_file()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open JetStream archive for verification: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read JetStream archive: {error}"))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            format!(
                "cannot restrict JetStream cache permissions at {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

struct RemoveTemporaryDirectory(PathBuf);

impl Drop for RemoveTemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct LocalJetStreamServer {
    url: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LocalJetStreamServer {
    pub fn start(root: PathBuf) -> Result<Self, String> {
        let root = fs::canonicalize(&root)
            .map_err(|error| format!("cannot resolve local JetStream source: {error}"))?;
        if !is_prepared_source(&root) {
            return Err(format!(
                "local JetStream source at {} is missing or does not match the pinned release",
                root.display()
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("cannot start local JetStream server: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot configure local JetStream server: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("cannot inspect local JetStream server: {error}"))?
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_root = root.clone();
        let worker = thread::Builder::new()
            .name("velocity-jetstream-http".to_owned())
            .spawn(move || {
                let (sender, receiver) = mpsc::sync_channel::<TcpStream>(32);
                let receiver = Arc::new(Mutex::new(receiver));
                let workers = (0..SERVER_WORKERS)
                    .filter_map(|index| {
                        let receiver = Arc::clone(&receiver);
                        let root = worker_root.clone();
                        thread::Builder::new()
                            .name(format!("velocity-jetstream-asset-{index}"))
                            .spawn(move || serve_worker(receiver, root))
                            .ok()
                    })
                    .collect::<Vec<_>>();
                while !worker_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if sender.send(stream).is_err() {
                                break;
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(10)),
                    }
                }
                drop(sender);
                for worker in workers {
                    let _ = worker.join();
                }
            })
            .map_err(|error| format!("cannot start local JetStream worker: {error}"))?;
        Ok(Self {
            url: format!("http://127.0.0.1:{port}{MOUNT_PREFIX}index.html"),
            stop,
            worker: Some(worker),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for LocalJetStreamServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(port) = self.url.split(':').nth(2).and_then(|part| {
            part.split('/')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
        }) {
            let _ = TcpStream::connect(("127.0.0.1", port));
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve_worker(receiver: Arc<Mutex<Receiver<TcpStream>>>, root: PathBuf) {
    loop {
        let stream = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        match stream {
            Ok(stream) => serve_request(stream, &root),
            Err(_) => return,
        }
    }
}

fn serve_request(mut stream: TcpStream, root: &Path) {
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    let mut request = Vec::with_capacity(1024);
    let mut byte = [0_u8; 1];
    while request.len() < MAX_REQUEST_BYTES {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                request.push(byte[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    if request.len() >= MAX_REQUEST_BYTES {
        let _ = write_response(&mut stream, 431, "text/plain", b"request too large");
        return;
    }
    let request = String::from_utf8_lossy(&request);
    let Some(line) = request.lines().next() else {
        return;
    };
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else {
        return;
    };
    let Some(target) = parts.next() else {
        return;
    };
    if !matches!(method, "GET" | "HEAD") {
        let _ = write_response(&mut stream, 405, "text/plain", b"method not allowed");
        return;
    }
    let request_path = target.split(['?', '#']).next().unwrap_or_default();
    let Some(relative_path) = request_path.strip_prefix(MOUNT_PREFIX) else {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    };
    let Some(relative_path) = decode_path(relative_path) else {
        let _ = write_response(&mut stream, 400, "text/plain", b"invalid path");
        return;
    };
    let relative = if relative_path.is_empty() {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(relative_path)
    };
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        let _ = write_response(&mut stream, 400, "text/plain", b"invalid path");
        return;
    }
    let Ok(mut path) = fs::canonicalize(root.join(relative)) else {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    };
    if path.is_dir() {
        path.push("index.html");
        let Ok(index_path) = fs::canonicalize(path) else {
            let _ = write_response(&mut stream, 404, "text/plain", b"not found");
            return;
        };
        path = index_path;
    }
    if !path.starts_with(root) || !path.is_file() {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    }
    let content_type = content_type(&path);
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => {
            let _ = write_response(&mut stream, 500, "text/plain", b"cannot read asset");
            return;
        }
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        let _ = write_response(&mut stream, 500, "text/plain", b"cannot read asset");
        return;
    };
    let _ = write_header(&mut stream, 200, content_type, length);
    if method == "GET" {
        let _ = io::copy(&mut file, &mut stream);
    }
}

fn decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            decoded.push((hex_value(high)? << 4) | hex_value(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("mjs" | "js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write_header(stream, status, content_type, body.len() as u64)?;
    stream.write_all(body)
}

fn write_header(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    length: u64,
) -> io::Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {length}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "velocity-jetstream-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn prepare_fixture(root: &Path) {
        fs::create_dir_all(root.join("utils")).unwrap();
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("index.html"), "<!doctype html>").unwrap();
        fs::write(root.join("JetStreamDriver.js"), "window.JetStream = {};").unwrap();
        fs::write(root.join("utils/params.js"), "const JetStreamParams = {};").unwrap();
        fs::write(root.join("resources/JetStream.css"), "body {}").unwrap();
        fs::write(
            root.join(".velocity-jetstream.json"),
            serde_json::json!({
                "version": JETSTREAM_VERSION,
                "commit": JETSTREAM_COMMIT,
                "archive_sha256": JETSTREAM_ARCHIVE_SHA256,
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn pinned_source_manifest_and_entry_points_are_required() {
        let root = fixture_root("manifest");
        prepare_fixture(&root);
        assert!(is_prepared_source(&root));
        fs::write(
            root.join(".velocity-jetstream.json"),
            serde_json::json!({
                "version": JETSTREAM_VERSION,
                "commit": JETSTREAM_COMMIT,
                "archive_sha256": "wrong",
            })
            .to_string(),
        )
        .unwrap();
        assert!(!is_prepared_source(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serves_large_assets_and_blocks_path_traversal() {
        let root = fixture_root("server");
        prepare_fixture(&root);
        let body = vec![b'j'; 2_000_000];
        fs::write(root.join("large.wasm"), &body).unwrap();
        let server = LocalJetStreamServer::start(root.clone()).unwrap();
        let address = server
            .url()
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /JetStream3.0/large.wasm HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let headers_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let headers = String::from_utf8_lossy(&response[..headers_end]);
        assert!(headers.starts_with("HTTP/1.1 200 OK"));
        assert!(headers.contains("Content-Type: application/wasm"));
        assert!(headers.contains("Content-Length: 2000000"));
        assert_eq!(response.len() - headers_end, body.len());
        assert!(response[headers_end..].iter().all(|byte| *byte == b'j'));

        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /JetStream3.0/ HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with("<!doctype html>"));

        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /JetStream3.0/%2e%2e/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        drop(server);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serves_bursty_concurrent_asset_requests() {
        const CLIENTS: usize = 96;
        let root = fixture_root("concurrent-server");
        prepare_fixture(&root);
        let server = LocalJetStreamServer::start(root.clone()).unwrap();
        let address = server
            .url()
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap()
            .to_owned();
        let barrier = Arc::new(std::sync::Barrier::new(CLIENTS));
        let results = thread::scope(|scope| {
            let clients = (0..CLIENTS)
                .map(|_| {
                    let address = address.clone();
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        let mut stream = TcpStream::connect(address).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(10)))
                            .unwrap();
                        stream
                            .write_all(
                                b"GET /JetStream3.0/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n",
                            )
                            .unwrap();
                        barrier.wait();
                        let mut response = Vec::new();
                        stream.read_to_end(&mut response).unwrap();
                        response.starts_with(b"HTTP/1.1 200 OK")
                            && response.ends_with(b"<!doctype html>")
                    })
                })
                .collect::<Vec<_>>();
            clients
                .into_iter()
                .map(|client| client.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(results.iter().all(|served| *served));
        drop(server);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn official_jetstream_url_uses_default_parameters() {
        assert_eq!(
            official_url(),
            "https://browserbench.org/JetStream3.0/index.html"
        );
        assert_eq!(JETSTREAM_VERSION, "3.0");
        assert_eq!(JETSTREAM_COMMIT.len(), 40);
    }
}
