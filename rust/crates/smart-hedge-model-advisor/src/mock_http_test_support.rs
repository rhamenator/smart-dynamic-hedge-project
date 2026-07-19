//! A tiny, test-only mock HTTP server — see
//! `smart_hedge_data::mock_http_test_support`'s doc comment for the full
//! rationale (duplicated here rather than shared across crates: it's
//! ~30 lines of test-only code, and a cross-crate dependency just for a
//! test double isn't worth the coupling).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Starts a mock server that always returns the same response body for
/// any request (this crate only ever needs to mock one endpoint —
/// OpenAI's Responses API — so no path routing is needed, unlike
/// `smart-hedge-data`'s multi-route Alpaca/FRED/RSS mocks).
pub fn start(status: u16, body: String) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("binding an ephemeral local port should never fail");
    let port = listener.local_addr().expect("a bound listener always has a local address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            handle_one(stream, status, &body);
        }
    });
    port
}

fn handle_one(mut stream: TcpStream, status: u16, body: &str) {
    let mut reader = BufReader::new(stream.try_clone().expect("cloning a TCP stream handle should never fail"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    // Crucial: drain the request body (this crate only ever mocks a POST
    // endpoint) *before* responding and dropping the connection. If the
    // OS still has unread inbound data buffered when the socket closes,
    // it can send a TCP reset instead of a graceful close — which races
    // with the client (`ureq`) possibly still writing that body,
    // occasionally surfacing as a transport error instead of the clean
    // response this mock is supposed to provide. Found via a real,
    // intermittent (~1-in-5) test failure.
    if content_length > 0 {
        let mut buf = vec![0u8; content_length.min(MAX_BODY_BYTES)];
        let _ = reader.read_exact(&mut buf);
    }

    let response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        if status == 200 { "OK" } else { "Error" },
        body.len(),
    );
    let _ = stream.write_all(response.as_bytes());
}
