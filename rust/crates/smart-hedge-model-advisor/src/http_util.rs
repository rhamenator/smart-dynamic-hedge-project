use std::io::Read;

/// Reads a `ureq` response body up to `max_bytes` rather than the
/// unbounded default `.into_string()` read — see
/// `smart_hedge_data::http_util`'s doc comment for the full reasoning
/// (duplicated here rather than shared across crates: it's a handful of
/// lines, and a cross-crate dependency just for this isn't worth the
/// coupling). Python's `openai` SDK client doesn't impose an explicit cap
/// here (it isn't a hand-rolled HTTP call the way `data.py`'s
/// Alpaca/FRED/RSS fetches are), so this bound is a Rust-side hardening
/// choice beyond parity, not a matched behavior — chosen generously
/// (assessment JSON responses are small, but the schema itself allows up
/// to 1000-character summaries and several list fields).
pub fn read_capped_body(response: ureq::Response, max_bytes: usize) -> Result<String, String> {
    let mut buf = Vec::new();
    response.into_reader().take(max_bytes as u64).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}
