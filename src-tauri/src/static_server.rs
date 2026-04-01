use std::collections::HashMap;
use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use tiny_http::{Header, Response, Server, StatusCode};

pub struct StaticFileServer {
    port: u16,
    _handle: thread::JoinHandle<()>,
}

impl StaticFileServer {
    pub fn start(root_dir: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let server = Server::http(format!("127.0.0.1:{}", port))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let root = Arc::new(root_dir);

        let handle = thread::spawn(move || {
            serve_loop(server, root);
        });

        Ok(Self {
            port,
            _handle: handle,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn serve_loop(server: Server, root: Arc<PathBuf>) {
    let mime_types = build_mime_map();

    for request in server.incoming_requests() {
        let url_path = request.url().split('?').next().unwrap_or("/");
        let decoded = percent_decode(url_path);
        let clean = decoded.trim_start_matches('/');

        let file_path = if clean.is_empty() {
            root.join("index.html")
        } else {
            root.join(clean)
        };

        let (file_path, status) = if file_path.is_file() {
            (file_path, 200)
        } else if file_path.is_dir() {
            let index = file_path.join("index.html");
            if index.is_file() {
                (index, 200)
            } else {
                (root.join("index.html"), 200)
            }
        } else {
            // SPA fallback: serve index.html for routes that don't match a file
            (root.join("index.html"), 200)
        };

        if !file_path.is_file() {
            let resp = Response::from_string("Not Found")
                .with_status_code(StatusCode(404));
            let _ = request.respond(resp);
            continue;
        }

        // Security: ensure the resolved path is within root
        match file_path.canonicalize() {
            Ok(canonical) => {
                if let Ok(root_canonical) = root.canonicalize() {
                    if !canonical.starts_with(&root_canonical) {
                        let resp = Response::from_string("Forbidden")
                            .with_status_code(StatusCode(403));
                        let _ = request.respond(resp);
                        continue;
                    }
                }
            }
            Err(_) => {
                let resp = Response::from_string("Not Found")
                    .with_status_code(StatusCode(404));
                let _ = request.respond(resp);
                continue;
            }
        }

        let mut file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(_) => {
                let resp = Response::from_string("Internal Server Error")
                    .with_status_code(StatusCode(500));
                let _ = request.respond(resp);
                continue;
            }
        };

        let mut data = Vec::new();
        if file.read_to_end(&mut data).is_err() {
            let resp = Response::from_string("Internal Server Error")
                .with_status_code(StatusCode(500));
            let _ = request.respond(resp);
            continue;
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let content_type = mime_types
            .get(ext)
            .copied()
            .unwrap_or("application/octet-stream");

        let mut resp = Response::from_data(data).with_status_code(StatusCode(status as u16));

        let ct_header = Header::from_bytes("Content-Type", content_type).unwrap();
        resp = resp.with_header(ct_header);

        let cache_val = if is_immutable_asset(ext, url_path) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        let cache_header = Header::from_bytes("Cache-Control", cache_val).unwrap();
        resp = resp.with_header(cache_header);

        let cors_header = Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap();
        resp = resp.with_header(cors_header);

        let _ = request.respond(resp);
    }
}

fn is_immutable_asset(ext: &str, path: &str) -> bool {
    // Hashed assets in /assets/ are immutable
    if path.starts_with("/assets/") {
        return true;
    }
    matches!(ext, "woff" | "woff2" | "ttf" | "otf")
}

fn build_mime_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("html", "text/html; charset=utf-8");
    m.insert("htm", "text/html; charset=utf-8");
    m.insert("css", "text/css; charset=utf-8");
    m.insert("js", "application/javascript; charset=utf-8");
    m.insert("mjs", "application/javascript; charset=utf-8");
    m.insert("json", "application/json; charset=utf-8");
    m.insert("wasm", "application/wasm");
    m.insert("svg", "image/svg+xml");
    m.insert("png", "image/png");
    m.insert("jpg", "image/jpeg");
    m.insert("jpeg", "image/jpeg");
    m.insert("gif", "image/gif");
    m.insert("ico", "image/x-icon");
    m.insert("webp", "image/webp");
    m.insert("ttf", "font/ttf");
    m.insert("otf", "font/otf");
    m.insert("woff", "font/woff");
    m.insert("woff2", "font/woff2");
    m.insert("map", "application/json");
    m.insert("txt", "text/plain; charset=utf-8");
    m.insert("xml", "application/xml; charset=utf-8");
    m
}

fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            if let (Some(h), Some(l)) = (hex_val(hi), hex_val(lo)) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
                result.push(hi as char);
                result.push(lo as char);
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
