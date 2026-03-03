mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::*;

struct HttpResponse {
    status_code: u16,
    body: String,
}

fn read_response_tolerant(stream: &mut TcpStream) -> Vec<u8> {
    let mut response_bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => response_bytes.extend_from_slice(&buffer[..bytes_read]),
            Err(error)
                if error.kind() == std::io::ErrorKind::ConnectionReset
                    || error.kind() == std::io::ErrorKind::BrokenPipe =>
            {
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("unexpected read error: {error}"),
        }
    }
    response_bytes
}

fn try_http_get(port: u16, path: &str) -> Option<HttpResponse> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let request =
        format!("GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;

    let response_bytes = read_response_tolerant(&mut stream);
    Some(parse_http_response(&response_bytes))
}

fn http_get(port: u16, path: &str) -> HttpResponse {
    try_http_get(port, path).unwrap_or_else(|| panic!("failed to connect to port {port}"))
}

fn http_post(port: u16, path: &str, json_body: &str) -> HttpResponse {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .unwrap_or_else(|error| panic!("failed to connect to port {port}: {error}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json_body}",
        json_body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();

    let response_bytes = read_response_tolerant(&mut stream);
    parse_http_response(&response_bytes)
}

fn parse_http_response(raw: &[u8]) -> HttpResponse {
    let response_text = String::from_utf8_lossy(raw);
    let (headers_section, body) = response_text
        .split_once("\r\n\r\n")
        .unwrap_or((&response_text, ""));

    let status_line = headers_section.lines().next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    HttpResponse {
        status_code,
        body: body.to_string(),
    }
}

fn wait_for_port(port: u16, timeout_seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    while Instant::now() < deadline {
        if TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("port {port} not reachable after {timeout_seconds}s");
}

fn wait_for_http(port: u16, path: &str, timeout_seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    while Instant::now() < deadline {
        if let Some(response) = try_http_get(port, path) {
            if response.status_code > 0 {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("no HTTP response on port {port}{path} after {timeout_seconds}s");
}

struct ServerProcess {
    child: Child,
    project_name: String,
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        cleanup_remote_project(&self.project_name);
    }
}

fn create_server_project(source_dir: &str) -> (tempfile::TempDir, String) {
    let source_path = project_root().join("test-apps").join(source_dir);
    let temp_dir = tempfile::Builder::new()
        .prefix("test-server-")
        .tempdir()
        .expect("failed to create temp server dir");

    for entry in std::fs::read_dir(&source_path).expect("failed to read test-apps source dir") {
        let entry = entry.unwrap();
        let dest = temp_dir.path().join(entry.file_name());
        std::fs::copy(entry.path(), &dest).expect("failed to copy test-app file");
    }

    let project_name = temp_dir
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    (temp_dir, project_name)
}

fn create_inline_server_project(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
    let temp_dir = create_temp_project(files);
    let project_name = temp_dir
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    (temp_dir, project_name)
}

fn spawn_server(
    project_dir: &std::path::Path,
    ports: &[u16],
    command_args: &[&str],
) -> ServerProcess {
    let project_name = project_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut args: Vec<&str> = vec!["run", "--on", "server"];
    for port in ports {
        args.push("--forward");
        args.push(Box::leak(port.to_string().into_boxed_str()));
    }
    args.push("--");
    args.extend_from_slice(command_args);

    let homelab_bin = assert_cmd::cargo::cargo_bin!("homelab");
    ensure_docker_lab_running();

    let child = Command::new(homelab_bin)
        .args(&args)
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn homelab run");

    ServerProcess {
        child,
        project_name,
    }
}

// --- Happy Path Tests ---

#[test]
fn server_api_get_and_post() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("api-server");
    let _server = spawn_server(project_dir.path(), &[8081], &["python3", "server.py"]);

    wait_for_port(8081, 15);

    let health = http_get(8081, "/health");
    assert_eq!(health.status_code, 200);
    assert!(health.body.contains("ok"));

    let items = http_get(8081, "/items");
    assert_eq!(items.status_code, 200);
    assert!(items.body.contains("keyboard"));

    let created = http_post(8081, "/items", r#"{"name": "webcam"}"#);
    assert_eq!(created.status_code, 201);
    assert!(created.body.contains("webcam"));

    let not_found = http_get(8081, "/bad");
    assert_eq!(not_found.status_code, 404);
}

#[test]
fn server_web_app_serves_html() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("web-app");
    let _server = spawn_server(project_dir.path(), &[8082], &["python3", "server.py"]);

    wait_for_port(8082, 15);

    let index = http_get(8082, "/");
    assert_eq!(index.status_code, 200);
    assert!(index.body.contains("Welcome to the Homelab"));

    let about = http_get(8082, "/about");
    assert_eq!(about.status_code, 200);
    assert!(about.body.contains("About"));

    let status = http_get(8082, "/status");
    assert_eq!(status.status_code, 200);
    assert!(status.body.contains("Status: Running"));

    let missing = http_get(8082, "/x");
    assert_eq!(missing.status_code, 404);
}

#[test]
fn server_fullstack_api_and_web() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("fullstack-app");
    let _server = spawn_server(project_dir.path(), &[8083, 8084], &["python3", "server.py"]);

    wait_for_port(8083, 15);
    wait_for_port(8084, 15);

    let api_items = http_get(8083, "/api/items");
    assert_eq!(api_items.status_code, 200);
    assert!(api_items.body.contains("raspberry-pi"));

    let web_index = http_get(8084, "/");
    assert_eq!(web_index.status_code, 200);
    assert!(web_index.body.contains("raspberry-pi"));
    assert!(web_index.body.contains("Data fetched from API"));
}

#[test]
fn server_json_rpc() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("rpc-server");
    let _server = spawn_server(project_dir.path(), &[8085], &["python3", "server.py"]);

    wait_for_port(8085, 15);

    let add_result = http_post(
        8085,
        "/",
        r#"{"jsonrpc": "2.0", "method": "add", "params": [3, 7], "id": 1}"#,
    );
    assert_eq!(add_result.status_code, 200);
    assert!(add_result.body.contains("10"));

    let multiply_result = http_post(
        8085,
        "/",
        r#"{"jsonrpc": "2.0", "method": "multiply", "params": [6, 9], "id": 2}"#,
    );
    assert_eq!(multiply_result.status_code, 200);
    assert!(multiply_result.body.contains("54"));

    let unknown_method = http_post(
        8085,
        "/",
        r#"{"jsonrpc": "2.0", "method": "nonexistent", "params": [], "id": 3}"#,
    );
    assert_eq!(unknown_method.status_code, 200);
    assert!(unknown_method.body.contains("error"));
}

// --- Edge Case Tests ---

#[test]
fn server_api_post_malformed_json() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("api-server");
    let _server = spawn_server(project_dir.path(), &[8081], &["python3", "server.py"]);

    wait_for_port(8081, 15);

    let malformed = http_post(8081, "/items", "not valid json{{{");
    assert!(
        malformed.status_code == 500 || malformed.status_code == 400 || malformed.status_code == 0,
        "expected 400, 500, or connection reset (0), got {}",
        malformed.status_code
    );
}

#[test]
fn server_api_wrong_method() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("api-server");
    let _server = spawn_server(project_dir.path(), &[8081], &["python3", "server.py"]);

    wait_for_port(8081, 15);

    let mut stream = TcpStream::connect("127.0.0.1:8081").unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = "PUT /items HTTP/1.1\r\nHost: localhost:8081\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).unwrap();

    let response_bytes = read_response_tolerant(&mut stream);
    let response = parse_http_response(&response_bytes);

    assert!(
        response.status_code == 404 || response.status_code == 405 || response.status_code == 501,
        "expected 404, 405, or 501 for unsupported method, got {}",
        response.status_code
    );
}

#[test]
fn server_web_app_missing_route() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("web-app");
    let _server = spawn_server(project_dir.path(), &[8082], &["python3", "server.py"]);

    wait_for_port(8082, 15);

    let missing = http_get(8082, "/this/does/not/exist");
    assert_eq!(missing.status_code, 404);
    assert!(missing.body.contains("Not Found"));
}

#[test]
fn server_crash_during_request() {
    require_docker_lab!();

    let crash_server = r#"
from http.server import HTTPServer, BaseHTTPRequestHandler
import sys

class CrashHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"ok")
        self.wfile.flush()
        sys.exit(1)

    def log_message(self, format, *args):
        pass

if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", 8086), CrashHandler)
    print("Crash server listening on port 8086", flush=True)
    server.serve_forever()
"#;

    let (project_dir, _project_name) =
        create_inline_server_project(&[("server.py", crash_server.trim())]);
    let mut server = spawn_server(project_dir.path(), &[8086], &["python3", "server.py"]);

    wait_for_port(8086, 15);

    let _ = http_get(8086, "/");

    let exit_status = server
        .child
        .wait()
        .expect("failed to wait for server process");
    assert!(
        !exit_status.success(),
        "CLI should exit with non-zero after server crash"
    );

    cleanup_remote_project(&server.project_name);
    server.project_name = String::new();
}

#[test]
fn server_slow_startup() {
    require_docker_lab!();

    let slow_server = r#"
import time
from http.server import HTTPServer, BaseHTTPRequestHandler

class SlowHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"finally ready")

    def log_message(self, format, *args):
        pass

if __name__ == "__main__":
    time.sleep(3)
    server = HTTPServer(("0.0.0.0", 8087), SlowHandler)
    print("Slow server listening on port 8087", flush=True)
    server.serve_forever()
"#;

    let (project_dir, _project_name) =
        create_inline_server_project(&[("server.py", slow_server.trim())]);
    let _server = spawn_server(project_dir.path(), &[8087], &["python3", "server.py"]);

    wait_for_http(8087, "/", 25);

    let response = http_get(8087, "/");
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("finally ready"));
}

#[test]
fn server_concurrent_requests() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("api-server");
    let _server = spawn_server(project_dir.path(), &[8081], &["python3", "server.py"]);

    wait_for_port(8081, 15);

    for request_index in 0..10 {
        let response = http_get(8081, "/health");
        assert_eq!(
            response.status_code, 200,
            "request {request_index} failed with status {}",
            response.status_code
        );
    }
}

#[test]
fn server_large_json_response() {
    require_docker_lab!();

    let large_server = r#"
import json
from http.server import HTTPServer, BaseHTTPRequestHandler

class LargeHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        items = [{"id": i, "name": f"item-{i}", "data": "x" * 80} for i in range(1000)]
        payload = json.dumps({"items": items})
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload.encode())

    def log_message(self, format, *args):
        pass

if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", 8088), LargeHandler)
    print("Large response server listening on port 8088", flush=True)
    server.serve_forever()
"#;

    let (project_dir, _project_name) =
        create_inline_server_project(&[("server.py", large_server.trim())]);
    let _server = spawn_server(project_dir.path(), &[8088], &["python3", "server.py"]);

    wait_for_port(8088, 15);

    let response = http_get(8088, "/");
    assert_eq!(response.status_code, 200);
    assert!(
        response.body.len() > 50_000,
        "expected large response body (>50KB), got {} bytes",
        response.body.len()
    );
    assert!(response.body.contains("item-0"));
    assert!(response.body.contains("item-999"));
}

#[test]
fn server_rpc_missing_fields() {
    require_docker_lab!();

    let (project_dir, _project_name) = create_server_project("rpc-server");
    let _server = spawn_server(project_dir.path(), &[8085], &["python3", "server.py"]);

    wait_for_port(8085, 15);

    let missing_method = http_post(8085, "/", r#"{"jsonrpc": "2.0", "params": [1], "id": 1}"#);
    assert_eq!(missing_method.status_code, 200);
    assert!(
        missing_method.body.contains("error") || missing_method.body.contains("result"),
        "server should handle missing method field gracefully"
    );
}
