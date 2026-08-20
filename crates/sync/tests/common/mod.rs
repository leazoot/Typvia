//! Shared test support for the WebDAV storage form: an in-process
//! WebDAV-enough server over a real TCP listener, with the ETag and
//! precondition semantics the protocol depends on, plus fault-injection
//! hooks for the malicious-storage red lines. Test-only; never shipped.

#![allow(dead_code)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One stored file: bytes plus a monotonically bumped version that renders
/// as the ETag `"v<n>"`.
#[derive(Clone)]
struct StoredFile {
    bytes: Vec<u8>,
    version: u64,
}

#[derive(Default)]
pub struct WebdavState {
    files: Mutex<HashMap<String, StoredFile>>,
    collections: Mutex<HashMap<String, ()>>,
    /// Fault injection: when set, the server ignores If-None-Match/If-Match
    /// (a precondition-stripping gateway) — the onboarding probe must refuse
    /// such an endpoint.
    pub strip_preconditions: AtomicBool,
    /// Fault injection: answer 500 to every request while set.
    pub fail_all: AtomicBool,
    /// Records the Authorization header of the last request, for credential
    /// wiring assertions (value only — tests use fake credentials).
    pub last_authorization: Mutex<Option<String>>,
    /// Fault injection: paths starting with this prefix answer 404 on GET —
    /// a storage view that temporarily hides a device's stream (network
    /// partition / propagation-lag stand-in for the rival-publish race).
    pub hidden_prefix: Mutex<Option<String>>,
}

impl WebdavState {
    /// Snapshot of all stored files (path -> bytes) for byte-level scans and
    /// rollback injection.
    pub fn snapshot(&self) -> HashMap<String, Vec<u8>> {
        self.files
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.bytes.clone()))
            .collect()
    }

    /// Tamper injection: run `mutate` over one stored file's bytes (the
    /// "storage modified a record" red line).
    pub fn tamper(&self, path: &str, mutate: impl FnOnce(&mut Vec<u8>)) {
        let mut files = self.files.lock().unwrap();
        if let Some(file) = files.get_mut(path) {
            mutate(&mut file.bytes);
            file.version += 1;
        }
    }

    /// Rollback injection: restore a previously captured snapshot, dropping
    /// everything written since (the "storage rolled back" red line).
    pub fn restore(&self, snapshot: HashMap<String, Vec<u8>>) {
        let mut files = self.files.lock().unwrap();
        files.clear();
        for (path, bytes) in snapshot {
            files.insert(path, StoredFile { bytes, version: 1 });
        }
    }
}

pub struct WebdavServer {
    pub url: String,
    pub state: Arc<WebdavState>,
    addr: std::net::SocketAddr,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl WebdavServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind webdav test server");
        let addr = listener.local_addr().expect("local addr");
        let state = Arc::new(WebdavState::default());
        let thread_state = Arc::clone(&state);
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if !handle_connection(stream, &thread_state) {
                    return;
                }
            }
        });
        Self {
            url: format!("http://127.0.0.1:{}", addr.port()),
            state,
            addr,
            handle: Some(handle),
        }
    }
}

impl Drop for WebdavServer {
    fn drop(&mut self) {
        if let Ok(mut stream) = TcpStream::connect(self.addr) {
            let _ = stream.write_all(b"SHUTDOWN / HTTP/1.1\r\n\r\n");
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Handles one connection (one request per connection: the server replies
/// `Connection: close`). Returns false when the shutdown sentinel arrived.
fn handle_connection(mut stream: TcpStream, state: &WebdavState) -> bool {
    let mut request = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let Ok(n) = stream.read(&mut chunk) else {
            return true;
        };
        if n == 0 {
            return true;
        }
        request.extend_from_slice(&chunk[..n]);
        if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if request.len() > 1 << 20 {
            return true;
        }
    };
    let head = String::from_utf8_lossy(&request[..header_end]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_string();
    if method == "SHUTDOWN" {
        return false;
    }
    let path = parts
        .next()
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();

    let mut content_length = 0usize;
    let mut if_none_match_star = false;
    let mut if_match: Option<String> = None;
    let mut authorization: Option<String> = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => content_length = value.parse().unwrap_or(0),
            "if-none-match" if value == "*" => if_none_match_star = true,
            "if-match" => if_match = Some(value.to_string()),
            "authorization" => authorization = Some(value.to_string()),
            _ => {}
        }
    }
    *state.last_authorization.lock().unwrap() = authorization;

    let mut body = request[header_end..].to_vec();
    while body.len() < content_length {
        let Ok(n) = stream.read(&mut chunk) else {
            return true;
        };
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    if state.fail_all.load(Ordering::SeqCst) {
        respond(&mut stream, 500, None, b"");
        return true;
    }
    let strip = state.strip_preconditions.load(Ordering::SeqCst);

    match method.as_str() {
        "GET" => {
            if let Some(prefix) = state.hidden_prefix.lock().unwrap().as_deref()
                && path.starts_with(prefix)
            {
                respond(&mut stream, 404, None, b"");
                return true;
            }
            let files = state.files.lock().unwrap();
            match files.get(&path) {
                Some(file) => {
                    let etag = format!("\"v{}\"", file.version);
                    let bytes = file.bytes.clone();
                    drop(files);
                    respond(&mut stream, 200, Some(&etag), &bytes);
                }
                None => respond(&mut stream, 404, None, b""),
            }
        }
        "PUT" => {
            let mut files = state.files.lock().unwrap();
            let existing = files.get(&path).cloned();
            if !strip {
                if if_none_match_star && existing.is_some() {
                    respond(&mut stream, 412, None, b"");
                    return true;
                }
                if let Some(required) = &if_match {
                    let current = existing.as_ref().map(|f| format!("\"v{}\"", f.version));
                    if current.as_deref() != Some(required.as_str()) {
                        respond(&mut stream, 412, None, b"");
                        return true;
                    }
                }
            }
            let version = existing.map_or(1, |f| f.version + 1);
            files.insert(
                path,
                StoredFile {
                    bytes: body,
                    version,
                },
            );
            let etag = format!("\"v{version}\"");
            respond(&mut stream, 201, Some(&etag), b"");
        }
        "DELETE" => {
            let removed = state.files.lock().unwrap().remove(&path).is_some();
            respond(&mut stream, if removed { 204 } else { 404 }, None, b"");
        }
        "MKCOL" => {
            let created = state.collections.lock().unwrap().insert(path, ()).is_none();
            respond(&mut stream, if created { 201 } else { 405 }, None, b"");
        }
        _ => respond(&mut stream, 405, None, b""),
    }
    true
}

fn respond(stream: &mut TcpStream, status: u16, etag: Option<&str>, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        404 => "Not Found",
        405 => "Method Not Allowed",
        412 => "Precondition Failed",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(etag) = etag {
        head.push_str(&format!("ETag: {etag}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
