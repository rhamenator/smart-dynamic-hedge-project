//! A tiny, test-only mock HTTP server: binds an ephemeral local port,
//! serves one canned response body per exact path (query string ignored),
//! and answers `404` for anything else. Not a general mock framework —
//! just enough to let the Alpaca/FRED/RSS provider tests make a *real*
//! HTTP round trip (real TCP, real `ureq` client code, real JSON/XML
//! parsing) against known fixture responses instead of either skipping the
//! network path entirely or depending on live third-party credentials.
//! Response bodies are checked against the exact JSON/XML shapes those
//! real APIs return, cross-referenced from `python/smart_hedge/data.py`'s
//! own field access (`quote_payload["quote"]["bp"]`, etc.) — the Python
//! source is the spec for the wire format here, not a dependency.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

/// Starts the mock server on a background thread and returns its port.
/// The thread is never explicitly stopped — it lives for the rest of the
/// test process, same as the dashboard's own integration-test servers.
pub fn start(routes: Vec<(&'static str, (u16, &'static str, String))>) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("binding an ephemeral local port should never fail");
    let port = listener.local_addr().expect("a bound listener always has a local address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            handle_one(stream, &routes);
        }
    });
    port
}

fn handle_one(mut stream: TcpStream, routes: &[(&'static str, (u16, &'static str, String))]) {
    let mut reader = BufReader::new(stream.try_clone().expect("cloning a TCP stream handle should never fail"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");

    let (status, content_type, body) = routes
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, response)| response.clone())
        .unwrap_or((404, "text/plain", "not found".to_string()));

    let response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        if status == 200 { "OK" } else { "Not Found" },
        body.len(),
    );
    let _ = stream.write_all(response.as_bytes());
}
