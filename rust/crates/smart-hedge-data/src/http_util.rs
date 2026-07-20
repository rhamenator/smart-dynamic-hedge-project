use std::io::Read;

/// Reads a `ureq` response body up to `max_bytes`, matching Python's own
/// defensive `response.read(N)` caps in `data.py` (2,000,000 bytes for
/// Alpaca/RSS, 1,000,000 for FRED). An unbounded `.into_string()` read
/// would let a misbehaving or actively malicious endpoint — especially an
/// arbitrary operator-configured RSS feed URL, the most adversarial input
/// surface of the three — exhaust memory with an oversized response.
/// Truncating mid-body (rather than erroring outright) matches Python's
/// behavior exactly: `read(N)` silently returns at most `N` bytes, which
/// then typically fails to parse as valid JSON/XML and is handled the
/// same way any other malformed response already is.
pub fn read_capped_body(response: ureq::Response, max_bytes: usize) -> Result<String, String> {
    let mut buf = Vec::new();
    response.into_reader().take(max_bytes as u64).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_response_within_the_cap_is_read_in_full() {
        let port = crate::mock_http_test_support::start(vec![("/x", (200, "text/plain", "hello".to_string()))]);
        let response = ureq::get(&format!("http://127.0.0.1:{port}/x")).call().unwrap();
        let body = read_capped_body(response, 1_000_000).unwrap();
        assert_eq!(body, "hello");
    }

    #[test]
    fn a_response_larger_than_the_cap_is_truncated_not_unbounded() {
        let large_body = "x".repeat(10_000);
        let port = crate::mock_http_test_support::start(vec![("/x", (200, "text/plain", large_body))]);
        let response = ureq::get(&format!("http://127.0.0.1:{port}/x")).call().unwrap();
        let body = read_capped_body(response, 100).unwrap();
        assert_eq!(body.len(), 100, "expected the read to stop at the cap, not read the full 10,000 bytes");
    }
}
