use serde_json::{json, Value};
use smart_hedge_engine::{ContractOverrides, EngineError, SmartHedgeEngine};

use crate::cache::Cache;
use crate::html::INDEX_HTML;
use crate::http::ParsedRequest;

pub struct AppState {
    pub engine: SmartHedgeEngine,
    pub cache: Cache,
}

pub struct RenderedResponse {
    pub status: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

fn json_response(status: u16, reason: &'static str, value: &Value) -> RenderedResponse {
    RenderedResponse {
        status,
        reason,
        content_type: "application/json; charset=utf-8",
        body: serde_json::to_vec(value).expect("Value serialization is infallible"),
    }
}

fn detail_error(status: u16, reason: &'static str, detail: impl std::fmt::Display) -> RenderedResponse {
    json_response(status, reason, &json!({"detail": detail.to_string()}))
}

fn html_response(body: &str) -> RenderedResponse {
    RenderedResponse { status: 200, reason: "OK", content_type: "text/html; charset=utf-8", body: body.as_bytes().to_vec() }
}

/// Port of `Query(..., min_length=1, max_length=12,
/// pattern=r"^[A-Za-z0-9._-]+$")` — the same validation FastAPI applied to
/// the `symbol` query parameter in both `/api/recommendation` and
/// `/api/history`.
fn valid_symbol(s: &str) -> bool {
    (1..=12).contains(&s.chars().count()) && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn parse_bool_flag(value: Option<&String>) -> bool {
    matches!(value.map(String::as_str), Some("true" | "1" | "yes" | "True" | "TRUE"))
}

fn parse_positive_i64(value: Option<&String>, default: i64) -> i64 {
    value.and_then(|v| v.parse::<i64>().ok()).unwrap_or(default)
}

fn route_health(state: &AppState) -> RenderedResponse {
    json_response(200, "OK", &state.engine.health())
}

fn route_recommendation(state: &AppState, req: &ParsedRequest) -> RenderedResponse {
    let raw_symbol = req.query.get("symbol").map(String::as_str).unwrap_or("SPY");
    if !valid_symbol(raw_symbol) {
        return detail_error(422, "Unprocessable Entity", "symbol must be 1-12 characters of [A-Za-z0-9._-]");
    }
    let symbol = raw_symbol.to_uppercase();
    let fresh = parse_bool_flag(req.query.get("fresh"));

    if !fresh
        && let Some(cached) = state.cache.get(&symbol)
    {
        return json_response(200, "OK", &cached);
    }
    match state.engine.recommendation(&symbol, &ContractOverrides::default()) {
        Ok(value) => {
            state.cache.put(&symbol, value.clone());
            json_response(200, "OK", &value)
        }
        Err(e) => detail_error(400, "Bad Request", format!("{e}")),
    }
}

fn route_history(state: &AppState, req: &ParsedRequest) -> RenderedResponse {
    let limit = parse_positive_i64(req.query.get("limit"), 20);
    let symbol = req.query.get("symbol").cloned();
    if let Some(s) = &symbol
        && !s.is_empty()
        && !valid_symbol(s)
    {
        return detail_error(422, "Unprocessable Entity", "symbol must be 1-12 characters of [A-Za-z0-9._-]");
    }
    let symbol_ref = symbol.as_deref().filter(|s| !s.is_empty());
    match state.engine.recent(limit, symbol_ref) {
        Ok(values) => json_response(200, "OK", &Value::Array(values)),
        Err(e) => detail_error(500, "Internal Server Error", format!("{e}")),
    }
}

fn route_replay(state: &AppState, decision_id: &str) -> RenderedResponse {
    match state.engine.replay(decision_id) {
        Ok(value) => json_response(200, "OK", &value),
        Err(EngineError::DecisionNotFound(id)) => detail_error(404, "Not Found", format!("decision not found: {id}")),
        Err(e) => detail_error(500, "Internal Server Error", format!("{e}")),
    }
}

/// Routes one parsed request to a handler, matching `dashboard.create_app`'s
/// route table. Returns `405` for a non-`GET` method (this dashboard never
/// mutates state, so nothing else is ever allowed) and `404` for an
/// unrecognized path.
pub fn handle(state: &AppState, req: &ParsedRequest) -> RenderedResponse {
    if req.method != "GET" {
        return detail_error(405, "Method Not Allowed", format!("{} is not supported", req.method));
    }
    if req.path == "/" {
        return html_response(INDEX_HTML);
    }
    if req.path == "/api/health" {
        return route_health(state);
    }
    if req.path == "/api/recommendation" {
        return route_recommendation(state, req);
    }
    if req.path == "/api/history" {
        return route_history(state, req);
    }
    if let Some(decision_id) = req.path.strip_prefix("/api/replay/")
        && !decision_id.is_empty()
        && !decision_id.contains('/')
    {
        return route_replay(state, decision_id);
    }
    detail_error(404, "Not Found", format!("no route for {}", req.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_symbol_accepts_typical_tickers() {
        assert!(valid_symbol("SPY"));
        assert!(valid_symbol("BRK.B"));
        assert!(valid_symbol("a"));
    }

    #[test]
    fn valid_symbol_rejects_empty_and_overlong_and_bad_characters() {
        assert!(!valid_symbol(""));
        assert!(!valid_symbol(&"x".repeat(13)));
        assert!(!valid_symbol("SP Y"));
        assert!(!valid_symbol("SPY;DROP"));
    }

    #[test]
    fn parse_bool_flag_recognizes_common_true_spellings() {
        assert!(parse_bool_flag(Some(&"true".to_string())));
        assert!(parse_bool_flag(Some(&"1".to_string())));
        assert!(!parse_bool_flag(Some(&"false".to_string())));
        assert!(!parse_bool_flag(None));
    }

    #[test]
    fn parse_positive_i64_falls_back_to_default_on_garbage() {
        assert_eq!(parse_positive_i64(Some(&"not-a-number".to_string()), 20), 20);
        assert_eq!(parse_positive_i64(Some(&"5".to_string()), 20), 5);
        assert_eq!(parse_positive_i64(None, 20), 20);
    }
}
