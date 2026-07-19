use serde_json::{json, Value};
use smart_hedge_engine::SmartHedgeEngine;

use crate::tools::{self, PriceOptionArgs};

pub const PROTOCOL_VERSION_DEFAULT: &str = "2024-11-05";
pub const SERVER_NAME: &str = "smart-dynamic-hedge";
pub const SERVER_VERSION: &str = "0.2.0";
pub const SERVER_INSTRUCTIONS: &str = "Paper-only hedge research tools. Deterministic pricing and policy output are authoritative. There is intentionally no order-placement tool. Never represent a preview as an executed trade.";

/// Port of `mcp_server.create_server`'s six `@mcp.tool()` registrations,
/// as MCP `tools/list` entries (name, description, JSON Schema input
/// shape). No tool here is named or shaped anything like
/// `place_order`/`submit_order`/`cancel_order` — verifies SDH-LLR-082.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "health",
            "description": "Return service health and prove that no broker-order endpoint is present.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        },
        {
            "name": "get_market_recommendation",
            "description": "Collect evidence and create one replayable paper hedge recommendation.",
            "inputSchema": {
                "type": "object",
                "properties": {"symbol": {"type": "string", "default": "SPY"}},
                "additionalProperties": false
            }
        },
        {
            "name": "price_option",
            "description": "Run deterministic C++ price/Greeks/hedge math without market-data retrieval.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "default": "SPY"},
                    "spot": {"type": "number", "default": 100.0},
                    "strike": {"type": "number", "default": 100.0},
                    "implied_volatility": {"type": "number", "default": 0.20},
                    "days_to_expiry": {"type": "number", "default": 30.0},
                    "option_type": {"type": "string", "default": "put"},
                    "exercise_style": {"type": "string", "default": "american"},
                    "contracts": {"type": "integer", "default": 1},
                    "current_shares": {"type": "number", "default": 0.0}
                },
                "additionalProperties": false
            }
        },
        {
            "name": "replay_decision",
            "description": "Read a stored decision without accessing markets or calling a model.",
            "inputSchema": {
                "type": "object",
                "properties": {"decision_id": {"type": "string"}},
                "required": ["decision_id"],
                "additionalProperties": false
            }
        },
        {
            "name": "list_recent_decisions",
            "description": "List recent paper decisions from the local SQLite audit log.",
            "inputSchema": {
                "type": "object",
                "properties": {"limit": {"type": "integer", "default": 10}, "symbol": {"type": "string", "default": ""}},
                "additionalProperties": false
            }
        },
        {
            "name": "get_policy_snapshot",
            "description": "Show non-model policy limits applied to every recommendation.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
        }
    ])
}

fn arg_str<'a>(args: &'a Value, key: &str, default: &'a str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn arg_f64(args: &Value, key: &str, default: f64) -> f64 {
    args.get(key).and_then(Value::as_f64).unwrap_or(default)
}
fn arg_i64(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(Value::as_i64).unwrap_or(default)
}

/// Dispatches a `tools/call` request's `name`/`arguments` to the matching
/// tool. Returns `Err` for both "unknown tool name" and any failure the
/// tool itself reports — both become an `isError: true` MCP result, not a
/// JSON-RPC protocol-level error (matching how the Python `FastMCP`
/// framework turns an exception raised inside a `@mcp.tool()` function
/// into an error *result*, not a transport-level failure).
pub fn call_tool(engine: &SmartHedgeEngine, name: &str, arguments: &Value) -> Result<String, String> {
    match name {
        "health" => tools::health(engine),
        "get_market_recommendation" => {
            let symbol = arg_str(arguments, "symbol", "SPY").to_uppercase();
            tools::get_market_recommendation(engine, &symbol)
        }
        "price_option" => {
            let args = PriceOptionArgs {
                symbol: arg_str(arguments, "symbol", "SPY").to_uppercase(),
                spot: arg_f64(arguments, "spot", 100.0),
                strike: arg_f64(arguments, "strike", 100.0),
                implied_volatility: arg_f64(arguments, "implied_volatility", 0.20),
                days_to_expiry: arg_f64(arguments, "days_to_expiry", 30.0),
                option_type: arg_str(arguments, "option_type", "put").to_string(),
                exercise_style: arg_str(arguments, "exercise_style", "american").to_string(),
                contracts: arg_i64(arguments, "contracts", 1),
                current_shares: arg_f64(arguments, "current_shares", 0.0),
            };
            tools::price_option(engine, &args)
        }
        "replay_decision" => {
            let decision_id = arg_str(arguments, "decision_id", "");
            if decision_id.is_empty() {
                return Err("decision_id is required".to_string());
            }
            tools::replay_decision(engine, decision_id)
        }
        "list_recent_decisions" => {
            let limit = arg_i64(arguments, "limit", 10);
            let symbol = arg_str(arguments, "symbol", "").to_uppercase();
            tools::list_recent_decisions(engine, limit, &symbol)
        }
        "get_policy_snapshot" => tools::get_policy_snapshot(engine),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn initialize_result(params: &Value) -> Value {
    let protocol_version = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION_DEFAULT);
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
        "instructions": SERVER_INSTRUCTIONS,
    })
}

fn success_envelope(id: Value, result: Value) -> String {
    serde_json::to_string(&json!({"jsonrpc": "2.0", "id": id, "result": result})).expect("Value serialization is infallible")
}

fn error_envelope(id: Value, code: i32, message: &str) -> String {
    serde_json::to_string(&json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
        .expect("Value serialization is infallible")
}

/// Handles one line of the MCP stdio transport (newline-delimited JSON-RPC
/// 2.0 messages — see this crate's module doc comment). Returns `None` for
/// a blank line or a notification (no `id`: per JSON-RPC 2.0, notifications
/// never get a response, even an error one, and even for an unrecognized
/// method — a client is free to send notifications the server doesn't
/// implement). Never panics on malformed input.
pub fn handle_line(engine: &SmartHedgeEngine, line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Some(error_envelope(Value::Null, -32700, "Parse error")),
    };

    let id = parsed.get("id").cloned()?;
    let method = parsed.get("method").and_then(Value::as_str).unwrap_or("").to_string();
    let params = parsed.get("params").cloned().unwrap_or(Value::Null);

    let outcome: Result<Value, (i32, String)> = match method.as_str() {
        "initialize" => Ok(initialize_result(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match call_tool(engine, name, &arguments) {
                Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}]})),
                Err(message) => Ok(json!({"content": [{"type": "text", "text": message}], "isError": true})),
            }
        }
        other => Err((-32601, format!("Method not found: {other}"))),
    };

    Some(match outcome {
        Ok(result) => success_envelope(id, result),
        Err((code, message)) => error_envelope(id, code, &message),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn test_engine() -> SmartHedgeEngine {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-mcp-protocol-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{}").unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        SmartHedgeEngine::new(loaded, dir, std::env::temp_dir().join("nonexistent.cpp")).unwrap()
    }

    #[test]
    fn a_blank_line_produces_no_response() {
        let engine = test_engine();
        assert_eq!(handle_line(&engine, ""), None);
        assert_eq!(handle_line(&engine, "   \n"), None);
    }

    #[test]
    fn malformed_json_returns_a_parse_error_with_null_id() {
        let engine = test_engine();
        let response = handle_line(&engine, "{not valid json").unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["error"]["code"], -32700);
        assert_eq!(value["id"], Value::Null);
    }

    #[test]
    fn a_notification_with_no_id_produces_no_response_even_if_recognized() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert_eq!(response, None);
    }

    #[test]
    fn a_notification_for_an_unrecognized_method_still_produces_no_response() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","method":"totally/unknown"}"#);
        assert_eq!(response, None);
    }

    #[test]
    fn initialize_echoes_the_requested_protocol_version() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-01-01"}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"]["protocolVersion"], "2025-01-01");
        assert_eq!(value["result"]["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn initialize_defaults_the_protocol_version_when_absent() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"]["protocolVersion"], PROTOCOL_VERSION_DEFAULT);
    }

    #[test]
    fn tools_list_returns_exactly_the_six_expected_tools_and_no_order_tool() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        let names: Vec<&str> = value["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec!["health", "get_market_recommendation", "price_option", "replay_decision", "list_recent_decisions", "get_policy_snapshot"]
        );
        for forbidden in ["place_order", "submit_order", "cancel_order"] {
            assert!(!names.contains(&forbidden));
        }
    }

    #[test]
    fn an_unknown_top_level_method_is_a_jsonrpc_protocol_error() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":7,"method":"bogus/method"}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["id"], 7);
    }

    #[test]
    fn tools_call_health_returns_content_with_no_error_flag() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"health","arguments":{}}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert!(value["result"]["content"][0]["text"].as_str().unwrap().contains("broker_order_endpoint_present"));
        assert!(value["result"].get("isError").is_none());
    }

    #[test]
    fn tools_call_with_an_unknown_tool_name_is_an_error_result_not_a_protocol_error() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"place_order","arguments":{}}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert!(value.get("error").is_none(), "should be a tool-level error, not a JSON-RPC error: {value}");
        assert_eq!(value["result"]["isError"], true);
    }

    #[test]
    fn tools_call_replay_decision_without_a_decision_id_is_a_tool_error() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"replay_decision","arguments":{}}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"]["isError"], true);
    }

    #[test]
    fn tools_call_replay_decision_for_an_unknown_id_is_a_tool_error() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"replay_decision","arguments":{"decision_id":"nope"}}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"]["isError"], true);
    }

    #[test]
    fn tools_call_get_policy_snapshot_reports_paper_only() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"get_policy_snapshot","arguments":{}}}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        let text = value["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"live_execution_allowed\": false"));
    }

    #[test]
    fn ping_is_answered() {
        let engine = test_engine();
        let response = handle_line(&engine, r#"{"jsonrpc":"2.0","id":8,"method":"ping"}"#).unwrap();
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"], json!({}));
    }
}
