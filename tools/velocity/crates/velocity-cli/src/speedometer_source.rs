use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

pub const SPEEDOMETER_VERSION: &str = "3.1";
pub const SPEEDOMETER_COMMIT: &str = "1386415be8fef2f6b6bbdbe1828872471c5d802a";
const ARCHIVE_SHA256: &str = "cfefa818d7789ed2f2b9f3f1bf608cc35e4e8241489eef47049296ce92791313";
const ARCHIVE_URL: &str = "https://codeload.github.com/WebKit/Speedometer/tar.gz/1386415be8fef2f6b6bbdbe1828872471c5d802a";
const MAX_ARCHIVE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkSourceKind {
    Official,
    Local,
}

impl BenchmarkSourceKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "official" => Ok(Self::Official),
            "local" => Ok(Self::Local),
            other => Err(format!(
                "unknown Speedometer source '{other}'; choose official or local"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Official => "official_hosted",
            Self::Local => "official_pinned_local",
        }
    }
}

pub fn official_url() -> String {
    "https://browserbench.org/Speedometer3.1/?startAutomatically".to_owned()
}

pub fn local_cache_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/Caches/Velocity/speedometer")
        .join(format!("{SPEEDOMETER_VERSION}-{SPEEDOMETER_COMMIT}")))
}

pub fn setup_speedometer_source() -> Result<PathBuf, String> {
    let destination = local_cache_path()?;
    if is_prepared_source(&destination) {
        return Ok(destination);
    }
    let cache_root = destination
        .parent()
        .ok_or_else(|| "Speedometer cache path has no parent".to_owned())?;
    fs::create_dir_all(cache_root).map_err(|error| {
        format!(
            "cannot create Speedometer cache {}: {error}",
            cache_root.display()
        )
    })?;
    set_private_directory_permissions(cache_root)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = cache_root.join(format!(".setup-{}-{now}", std::process::id()));
    fs::create_dir(&temporary).map_err(|error| {
        format!(
            "cannot create temporary Speedometer setup directory {}: {error}",
            temporary.display()
        )
    })?;
    let _temporary_cleanup = RemoveTemporaryDirectory(temporary.clone());
    set_private_directory_permissions(&temporary)?;
    let archive = temporary.join("speedometer.tar.gz");

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
            &MAX_ARCHIVE_BYTES.to_string(),
            "--output",
        ])
        .arg(&archive)
        .arg(ARCHIVE_URL)
        .status()
        .map_err(|error| format!("cannot start curl to fetch official Speedometer: {error}"))?;
    if !download.success() {
        let _ = fs::remove_dir_all(&temporary);
        return Err("could not download the pinned official Speedometer source archive".to_owned());
    }
    if fs::metadata(&archive)
        .map_err(|error| format!("cannot inspect Speedometer archive: {error}"))?
        .len()
        > MAX_ARCHIVE_BYTES
    {
        let _ = fs::remove_dir_all(&temporary);
        return Err("Speedometer source archive exceeds the 50 MiB size limit".to_owned());
    }
    let actual_digest = sha256_file(&archive)?;
    if actual_digest != ARCHIVE_SHA256 {
        let _ = fs::remove_dir_all(&temporary);
        return Err(format!(
            "official Speedometer archive checksum mismatch: expected {ARCHIVE_SHA256}, received {actual_digest}"
        ));
    }

    let extracted = temporary.join("source");
    fs::create_dir(&extracted).map_err(|error| {
        format!(
            "cannot create Speedometer extraction directory {}: {error}",
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
        .map_err(|error| format!("cannot start tar to unpack Speedometer: {error}"))?;
    if !extraction.success() {
        let _ = fs::remove_dir_all(&temporary);
        return Err("could not unpack the verified Speedometer source archive".to_owned());
    }
    if !has_speedometer_entry(&extracted) {
        let _ = fs::remove_dir_all(&temporary);
        return Err(
            "pinned Speedometer archive does not contain its expected entry point".to_owned(),
        );
    }

    let marker = serde_json::json!({
        "version": SPEEDOMETER_VERSION,
        "commit": SPEEDOMETER_COMMIT,
        "archive_sha256": ARCHIVE_SHA256,
    });
    fs::write(
        extracted.join(".velocity-speedometer.json"),
        serde_json::to_vec_pretty(&marker).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("cannot write Speedometer cache metadata: {error}"))?;
    set_private_directory_permissions(&extracted)?;

    if destination.exists() {
        // Never replace a directory whose identity or contents Velocity did not create.
        let _ = fs::remove_dir_all(&temporary);
        return Err(format!(
            "Speedometer cache path already exists but did not validate: {}",
            destination.display()
        ));
    }
    fs::rename(&extracted, &destination).map_err(|error| {
        format!(
            "cannot install Speedometer cache at {}: {error}",
            destination.display()
        )
    })?;
    let _ = fs::remove_dir_all(&temporary);
    Ok(destination)
}

struct RemoveTemporaryDirectory(PathBuf);

impl Drop for RemoveTemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn require_prepared_source() -> Result<PathBuf, String> {
    let path = local_cache_path()?;
    if is_prepared_source(&path) {
        Ok(path)
    } else {
        Err(format!(
            "the pinned local Speedometer source is not installed; run 'velocity setup speedometer' (expected {})",
            path.display()
        ))
    }
}

fn is_prepared_source(path: &Path) -> bool {
    if !has_speedometer_entry(path) {
        return false;
    }
    let marker = path.join(".velocity-speedometer.json");
    fs::read(marker)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|value| {
            value.get("version").and_then(serde_json::Value::as_str) == Some(SPEEDOMETER_VERSION)
                && value.get("commit").and_then(serde_json::Value::as_str)
                    == Some(SPEEDOMETER_COMMIT)
                && value
                    .get("archive_sha256")
                    .and_then(serde_json::Value::as_str)
                    == Some(ARCHIVE_SHA256)
        })
}

fn has_speedometer_entry(path: &Path) -> bool {
    path.join("index.html").is_file()
        && path.join("resources/main.mjs").is_file()
        && path.join("resources/tests.mjs").is_file()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open Speedometer archive for verification: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read Speedometer archive: {error}"))?;
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
                "cannot restrict Speedometer cache permissions at {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

pub struct LocalSpeedometerServer {
    url: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LocalSpeedometerServer {
    pub fn start(root: PathBuf) -> Result<Self, String> {
        let root = fs::canonicalize(&root)
            .map_err(|error| format!("cannot resolve local Speedometer source: {error}"))?;
        if !is_prepared_source(&root) {
            return Err(format!(
                "local Speedometer source at {} is missing or does not match the pinned release",
                root.display()
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("cannot start local Speedometer server: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("cannot configure local Speedometer server: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("cannot inspect local Speedometer server: {error}"))?
            .port();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_root = root.clone();
        let worker = thread::Builder::new()
            .name("velocity-speedometer-http".to_owned())
            .spawn(move || {
                while !worker_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => serve_request(stream, &worker_root),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(25)),
                    }
                }
            })
            .map_err(|error| format!("cannot start local Speedometer worker: {error}"))?;
        Ok(Self {
            url: format!("http://127.0.0.1:{port}/Speedometer3.1/?startAutomatically"),
            stop,
            worker: Some(worker),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for LocalSpeedometerServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(port) = self.url.split(':').nth(2).and_then(|port_and_path| {
            port_and_path
                .split('/')
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

fn serve_request(mut stream: TcpStream, root: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
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
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let Some(relative) = path.strip_prefix("/Speedometer3.1/") else {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    };
    let relative = if relative.is_empty() {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(relative)
    };
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
        || relative.to_string_lossy().contains('%')
    {
        let _ = write_response(&mut stream, 400, "text/plain", b"invalid path");
        return;
    }
    let path = root.join(relative);
    let Ok(path) = fs::canonicalize(path) else {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    };
    if !path.starts_with(root) || !path.is_file() {
        let _ = write_response(&mut stream, 404, "text/plain", b"not found");
        return;
    }
    match fs::read(&path) {
        Ok(contents) => {
            let content_type = content_type(&path);
            let body = if method == "HEAD" { &[][..] } else { &contents };
            let _ = write_response(&mut stream, 200, content_type, body);
        }
        Err(_) => {
            let _ = write_response(&mut stream, 500, "text/plain", b"cannot read asset");
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("mjs" | "js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("woff" | "woff2") => "font/woff2",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
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
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_url_starts_the_unmodified_benchmark_automatically() {
        assert_eq!(
            official_url(),
            "https://browserbench.org/Speedometer3.1/?startAutomatically"
        );
        assert_eq!(SPEEDOMETER_VERSION, "3.1");
        assert_eq!(SPEEDOMETER_COMMIT.len(), 40);
    }

    #[test]
    fn local_source_requires_the_pinned_manifest_and_entry_points() {
        let root = std::env::temp_dir().join(format!(
            "velocity-speedometer-source-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("index.html"), "fixture").unwrap();
        fs::write(root.join("resources/main.mjs"), "fixture").unwrap();
        fs::write(root.join("resources/tests.mjs"), "fixture").unwrap();
        fs::write(
            root.join(".velocity-speedometer.json"),
            serde_json::json!({
                "version": SPEEDOMETER_VERSION,
                "commit": SPEEDOMETER_COMMIT,
                "archive_sha256": ARCHIVE_SHA256,
            })
            .to_string(),
        )
        .unwrap();
        assert!(is_prepared_source(&root));
        fs::write(
            root.join(".velocity-speedometer.json"),
            serde_json::json!({
                "version": SPEEDOMETER_VERSION,
                "commit": SPEEDOMETER_COMMIT,
                "archive_sha256": "wrong",
            })
            .to_string(),
        )
        .unwrap();
        assert!(!is_prepared_source(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_server_serves_only_the_pinned_benchmark_tree_and_shuts_down() {
        let root = std::env::temp_dir().join(format!(
            "velocity-speedometer-server-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("index.html"), "official entry").unwrap();
        fs::write(root.join("resources/main.mjs"), "main").unwrap();
        fs::write(root.join("resources/tests.mjs"), "tests").unwrap();
        fs::write(
            root.join(".velocity-speedometer.json"),
            serde_json::json!({
                "version": SPEEDOMETER_VERSION,
                "commit": SPEEDOMETER_COMMIT,
                "archive_sha256": ARCHIVE_SHA256,
            })
            .to_string(),
        )
        .unwrap();
        let server = LocalSpeedometerServer::start(root.clone()).unwrap();
        let response = TcpStream::connect(
            server
                .url()
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap(),
        )
        .and_then(|mut stream| {
            stream.write_all(b"GET /Speedometer3.1/ HTTP/1.1\r\nHost: localhost\r\n\r\n")?;
            let mut response = String::new();
            stream.read_to_string(&mut response)?;
            Ok(response)
        })
        .unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("official entry"));
        drop(server);
        fs::remove_dir_all(root).unwrap();
    }
}
