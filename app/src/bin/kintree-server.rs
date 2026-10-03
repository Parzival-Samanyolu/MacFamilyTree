//! Local HTTP bridge: `POST /api/<command>` with a JSON body, plus static files for the web build.
//! Used for development, end-to-end tests and "web mode". The desktop app calls the same `dispatch` via Tauri.
//!
//! Usage: kintree-server [--port 8787] [--static ui/dist]

use kintree_app::{dispatch, Session};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tiny_http::{Header, Method, Response, Server, StatusCode};

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn main() {
    let mut port = 8787u16;
    let mut static_dir = PathBuf::from("ui/dist");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port),
            "--static" => static_dir = PathBuf::from(args.next().unwrap_or_default()),
            _ => {}
        }
    }
    // Bind to loopback only: the API has no authentication.
    let server = Server::http(("127.0.0.1", port)).expect("cannot bind");
    eprintln!("kintree-server listening on http://127.0.0.1:{port}");
    let session = Mutex::new(Session::new());
    for mut req in server.incoming_requests() {
        let url = req.url().split('?').next().unwrap_or("/").to_string();
        let method = req.method().clone();
        let json: Header = "Content-Type: application/json".parse().unwrap();
        if method == Method::Post && url.starts_with("/api/") {
            let cmd = url.trim_start_matches("/api/").to_string();
            let mut body = String::new();
            let _ = req
                .as_reader()
                .take(512 * 1024 * 1024)
                .read_to_string(&mut body);
            let args: serde_json::Value = if body.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&body).unwrap_or(serde_json::json!({}))
            };
            let result = {
                let mut s = session.lock().unwrap();
                dispatch(&mut s, &cmd, args)
            };
            let (code, text) = match result {
                Ok(v) => (200, v.to_string()),
                Err(e) => (400, serde_json::to_string(&e).unwrap()),
            };
            let _ = req.respond(
                Response::from_string(text)
                    .with_status_code(StatusCode(code))
                    .with_header(json),
            );
            continue;
        }
        // static files (no path traversal)
        let rel = url.trim_start_matches('/');
        let rel = if rel.is_empty() { "index.html" } else { rel };
        let mut path = static_dir.join(rel);
        if rel.split('/').any(|c| c == "..") || !path.is_file() {
            path = static_dir.join("index.html"); // SPA fallback
        }
        match std::fs::read(&path) {
            Ok(bytes) => {
                let ct: Header = format!("Content-Type: {}", content_type(&path))
                    .parse()
                    .unwrap();
                let _ = req.respond(Response::from_data(bytes).with_header(ct));
            }
            Err(_) => {
                let _ = req
                    .respond(Response::from_string("not found").with_status_code(StatusCode(404)));
            }
        }
    }
}
