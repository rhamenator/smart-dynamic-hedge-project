//! End-to-end tests that bind a real `TcpListener`, run the real accept
//! loop in a background thread, and make real TCP requests against it —
//! the dashboard equivalent of `smart-hedge-cli`'s subprocess integration
//! tests. Skips (passes trivially) when no prebuilt C++ core binary is
//! available, same reasoning as `smart-hedge-engine`/`smart-hedge-cli`'s
//! own integration tests.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use smart_hedge_config::EnvOverrides;

use crate::server::{build_state, run};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

fn prebuilt_core_binary(root: &Path) -> Option<PathBuf> {
    let direct = root.join("build").join(if cfg!(windows) { "smart_dynamic_hedge.exe" } else { "smart_dynamic_hedge" });
    if direct.is_file() {
        return Some(direct);
    }
    let windows_fallback = root.join("build").join("Release").join("smart_dynamic_hedge.exe");
    if windows_fallback.is_file() { Some(windows_fallback) } else { None }
}

/// Binds an ephemeral local port, starts the real server on it in a
/// background thread, and returns the port. The background thread is
/// intentionally never joined — it lives for the rest of the test
/// process, same as any other "serve forever" loop under test.
fn start_test_server(name: &str) -> Option<u16> {
    let root = repo_root();
    let core_binary = prebuilt_core_binary(&root)?;
    let cpp_source = root.join("cpp").join("smart_dynamic_hedge.cpp");

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("smart-hedge-dashboard-itest-{name}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    let config_path = dir.join("config.json");
    let sqlite_path = dir.join("decisions.sqlite3");
    std::fs::write(
        &config_path,
        format!(
            r#"{{"storage": {{"sqlite_path": "{}"}}}}"#,
            sqlite_path.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();

    let env = EnvOverrides { core_binary: Some(core_binary.to_string_lossy().into_owned()), ..EnvOverrides::default() };
    let loaded = smart_hedge_config::load_config(Some(&config_path), &env, &root).unwrap();

    let state = build_state(loaded, root, cpp_source).expect("engine construction should succeed");
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let _ = run(listener, state);
    });
    Some(port)
}

fn raw_request(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn health_endpoint_returns_200_and_reports_no_order_endpoint() {
    let Some(port) = start_test_server("health") else { return };
    let response = raw_request(port, "GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 200"), "response was: {response}");
    assert!(response.contains("\"broker_order_endpoint_present\":false"), "response was: {response}");
}

#[test]
fn index_page_returns_the_html_console() {
    let Some(port) = start_test_server("index") else { return };
    let response = raw_request(port, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 200"), "response was: {response}");
    assert!(response.contains("Content-Type: text/html"), "response was: {response}");
    assert!(response.contains("Smart Dynamic Hedge"), "response was: {response}");
}

#[test]
fn recommendation_endpoint_returns_a_paper_only_decision() {
    let Some(port) = start_test_server("recommendation") else { return };
    let response = raw_request(
        port,
        "GET /api/recommendation?symbol=SPY&fresh=true HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(response.starts_with("HTTP/1.1 200"), "response was: {response}");
    assert!(response.contains("\"mode\":\"paper\""), "response was: {response}");
    assert!(response.contains("\"live_execution_allowed\":false"), "response was: {response}");
}

#[test]
fn an_invalid_symbol_is_rejected_with_422() {
    let Some(port) = start_test_server("badsymbol") else { return };
    let response = raw_request(port, "GET /api/recommendation?symbol=bad%20symbol HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 422"), "response was: {response}");
}

#[test]
fn replay_of_an_unknown_decision_returns_404() {
    let Some(port) = start_test_server("replay404") else { return };
    let response = raw_request(port, "GET /api/replay/does-not-exist HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 404"), "response was: {response}");
}

#[test]
fn an_unknown_route_returns_404() {
    let Some(port) = start_test_server("unknownroute") else { return };
    let response = raw_request(port, "GET /nope HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 404"), "response was: {response}");
}

#[test]
fn a_non_get_method_returns_405() {
    let Some(port) = start_test_server("wrongmethod") else { return };
    let response = raw_request(port, "POST /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 405"), "response was: {response}");
}

#[test]
fn history_endpoint_returns_a_json_array() {
    let Some(port) = start_test_server("history") else { return };
    let response = raw_request(port, "GET /api/history?limit=5 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(response.starts_with("HTTP/1.1 200"), "response was: {response}");
    let body_start = response.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    assert!(response[body_start..].trim_start().starts_with('['), "response was: {response}");
}

#[test]
fn recommendation_then_history_shows_the_new_decision() {
    let Some(port) = start_test_server("recthenhist") else { return };
    let rec = raw_request(port, "GET /api/recommendation?symbol=SPY&fresh=true HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(rec.starts_with("HTTP/1.1 200"), "response was: {rec}");
    let hist = raw_request(port, "GET /api/history?symbol=SPY HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert!(hist.starts_with("HTTP/1.1 200"), "response was: {hist}");
    assert!(hist.contains("\"symbol\":\"SPY\""), "response was: {hist}");
}
