//! A minimal, narrowly-scoped HTTP/1.1 server-side request parser and
//! response writer — **not** a general-purpose HTTP implementation. This
//! server only ever accepts `GET` requests with no body against a fixed
//! set of routes on `127.0.0.1` (or an operator-configured host) by
//! default; it always responds `Connection: close` (no keep-alive/pipelining
//! to get wrong) and always sends the full response body in one write (no
//! chunked transfer-encoding to implement). That narrowness is what makes
//! hand-rolling this safe to do at all — see `docs/ROADMAP.md` "Language
//! and dependency policy" for the broader hand-roll-vs-depend reasoning;
//! unlike the *client* side (`smart-hedge-data`'s `ureq`/`rustls`), there is
//! no TLS here (matches Python's `uvicorn` dashboard default, which is also
//! plain HTTP on localhost) and no adversarial third-party response to parse.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 100;
const MAX_BODY_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum HttpError {
    Io(io::Error),
    MalformedRequestLine,
    HeadersTooLarge,
    TooManyHeaders,
}

impl From<io::Error> for HttpError {
    fn from(e: io::Error) -> Self {
        HttpError::Io(e)
    }
}

/// Reads and parses one HTTP/1.1 request from `reader`: the request line
/// (method, path, query string — the HTTP version token is read but
/// ignored), then headers up to the blank line, then discards any request
/// body per `Content-Length` (bounded — this server never expects a body
/// for the `GET`-only routes it serves, this is just defensive draining).
/// Bounded throughout so a slow or malicious client can't grow memory
/// unboundedly; exceeding a bound is reported as an error, never a panic.
pub fn read_request(stream: impl Read) -> Result<ParsedRequest, HttpError> {
    let mut reader = BufReader::new(stream);
    let mut total = 0usize;

    let mut request_line = String::new();
    let n = reader.read_line(&mut request_line)?;
    if n == 0 {
        return Err(HttpError::MalformedRequestLine);
    }
    total += n;
    let request_line = request_line.trim_end();
    let mut parts = request_line.splitn(3, ' ');
    let method = parts.next().filter(|s| !s.is_empty()).ok_or(HttpError::MalformedRequestLine)?.to_string();
    let target = parts.next().filter(|s| !s.is_empty()).ok_or(HttpError::MalformedRequestLine)?;

    let (path, query_str) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    let query = parse_query(&query_str);

    let mut content_length: usize = 0;
    let mut header_count = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        total += n;
        if total > MAX_HEADER_BYTES {
            return Err(HttpError::HeadersTooLarge);
        }
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        header_count += 1;
        if header_count > MAX_HEADERS {
            return Err(HttpError::TooManyHeaders);
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    if content_length > 0 {
        let mut buf = vec![0u8; content_length.min(MAX_BODY_BYTES)];
        let _ = reader.read_exact(&mut buf);
    }

    Ok(ParsedRequest { method, path, query })
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decodes purely on bytes (never slices the `&str` at an
/// arbitrary offset) so a stray `%` next to a multi-byte UTF-8 character
/// can never panic on a non-char-boundary index; also decodes `+` as a
/// space, the conventional `application/x-www-form-urlencoded` query-string
/// behavior most web frameworks (including the FastAPI original this ports)
/// apply to query strings.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(raw: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(percent_decode(k), percent_decode(v));
    }
    out
}

/// Writes a complete HTTP/1.1 response: status line, `Content-Type`,
/// `Content-Length`, `Connection: close`, a blank line, then the body.
pub fn write_response(mut writer: impl Write, status: u16, reason: &str, content_type: &str, body: &[u8]) -> io::Result<()> {
    write!(writer, "HTTP/1.1 {status} {reason}\r\n")?;
    write!(writer, "Content-Type: {content_type}\r\n")?;
    write!(writer, "Content-Length: {}\r\n", body.len())?;
    write!(writer, "Connection: close\r\n")?;
    write!(writer, "\r\n")?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_a_simple_get_request_with_no_query_string() {
        let raw = "GET /api/health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/health");
        assert!(req.query.is_empty());
    }

    #[test]
    fn parses_a_query_string_into_key_value_pairs() {
        let raw = "GET /api/recommendation?symbol=SPY&fresh=true HTTP/1.1\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.path, "/api/recommendation");
        assert_eq!(req.query.get("symbol"), Some(&"SPY".to_string()));
        assert_eq!(req.query.get("fresh"), Some(&"true".to_string()));
    }

    #[test]
    fn percent_decodes_query_values() {
        let raw = "GET /x?a=hello%20world&b=%2Fslash HTTP/1.1\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.query.get("a"), Some(&"hello world".to_string()));
        assert_eq!(req.query.get("b"), Some(&"/slash".to_string()));
    }

    #[test]
    fn plus_decodes_as_space_in_query_strings() {
        let raw = "GET /x?a=hello+world HTTP/1.1\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.query.get("a"), Some(&"hello world".to_string()));
    }

    #[test]
    fn a_query_key_with_no_equals_sign_becomes_an_empty_value() {
        let raw = "GET /x?flag HTTP/1.1\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.query.get("flag"), Some(&"".to_string()));
    }

    #[test]
    fn a_trailing_percent_with_no_hex_digits_is_left_literal_not_panicking() {
        let raw = "GET /x?a=100%25off HTTP/1.1\r\n\r\n";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.query.get("a"), Some(&"100%off".to_string()));
        // Also verify the truncated-at-end case never panics.
        let raw2 = "GET /x?a=trunc% HTTP/1.1\r\n\r\n";
        let req2 = read_request(Cursor::new(raw2)).unwrap();
        assert_eq!(req2.query.get("a"), Some(&"trunc%".to_string()));
    }

    #[test]
    fn a_stray_percent_next_to_a_multibyte_character_never_panics() {
        let raw = "GET /x?a=%e2%9c%93text HTTP/1.1\r\n\r\n"; // decodes to a checkmark + "text"
        let req = read_request(Cursor::new(raw)).unwrap();
        assert!(req.query.contains_key("a"));
    }

    #[test]
    fn empty_input_is_a_malformed_request_line_not_a_panic() {
        let result = read_request(Cursor::new(""));
        assert!(matches!(result, Err(HttpError::MalformedRequestLine)));
    }

    #[test]
    fn a_request_line_with_no_target_is_malformed() {
        let result = read_request(Cursor::new("GET\r\n\r\n"));
        assert!(matches!(result, Err(HttpError::MalformedRequestLine)));
    }

    #[test]
    fn headers_exceeding_the_total_byte_bound_are_rejected() {
        // Few headers, each individually huge, so this trips the byte-size
        // bound specifically rather than the header-count bound (which a
        // large number of small headers would hit first).
        let mut raw = "GET / HTTP/1.1\r\n".to_string();
        for i in 0..10 {
            raw.push_str(&format!("X-Header-{i}: {}\r\n", "x".repeat(3000)));
        }
        raw.push_str("\r\n");
        let result = read_request(Cursor::new(raw));
        assert!(matches!(result, Err(HttpError::HeadersTooLarge)));
    }

    #[test]
    fn a_large_number_of_small_headers_is_rejected_by_the_count_bound() {
        let mut raw = "GET / HTTP/1.1\r\n".to_string();
        for i in 0..2000 {
            raw.push_str(&format!("X-Header-{i}: x\r\n"));
        }
        raw.push_str("\r\n");
        let result = read_request(Cursor::new(raw));
        assert!(matches!(result, Err(HttpError::TooManyHeaders) | Err(HttpError::HeadersTooLarge)));
    }

    #[test]
    fn a_request_with_a_body_and_content_length_is_drained_not_left_dangling() {
        let raw = "GET /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
        let req = read_request(Cursor::new(raw)).unwrap();
        assert_eq!(req.path, "/x");
    }

    #[test]
    fn write_response_produces_well_formed_headers_and_body() {
        let mut out = Vec::new();
        write_response(&mut out, 200, "OK", "application/json", b"{}").unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Type: application/json\r\n"));
        assert!(text.contains("Content-Length: 2\r\n"));
        assert!(text.ends_with("{}"));
    }
}
