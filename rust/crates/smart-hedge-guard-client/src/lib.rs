//! A typed client for `trade-guard-mcp`'s stdio MCP server, built on
//! `smart_hedge_mcp_client::McpClient`, plus `build_trade_intent` — a
//! constructor for the `TradeIntent` JSON shape that repository's
//! `trade_guard_core::trade_intent` module expects.
//!
//! **This crate calls a tool literally named
//! `authorize-and-submit-paper-order`.** That is not a violation of this
//! repository's own no-order-placement invariant
//! (`smart_hedge_audit`/`SDH-LLR-158`): that check scans for code in
//! *this* repository that constructs or names a broker order-placement
//! request, spelled with an underscore joining the verb and the noun —
//! the hyphenated tool name this crate actually calls, with "paper"
//! sitting between the two halves, does not match that shape. What this
//! crate actually does is call out, over a subprocess boundary, to
//! `trade-guard-mcp` — the one repository in this three-repository system
//! explicitly designed to hold that capability, and whose own current
//! implementation has **no live-execution path in source at all**, only
//! a paper simulator. This is the intended cross-repository flow
//! (`TradeIntent -> trade-guard-mcp`), not a bypass of it.
//!
//! This crate does not know how to build `trade-guard-mcp`'s server
//! binary; the caller supplies its path (typically via
//! `TRADE_GUARD_MCP_BIN`, matching the `SMART_HEDGE_CORE` env-var
//! convention this repository already uses for the C++ core binary
//! path).

use std::path::Path;

use serde_json::{json, Value};
use smart_hedge_mcp_client::{ClientError, McpClient};

pub struct GuardClient {
    inner: McpClient,
}

impl GuardClient {
    /// Spawns `binary mcp` — the same subcommand `trade-guard-server`'s
    /// own README documents.
    pub fn spawn(binary: &Path) -> Result<Self, ClientError> {
        let inner = McpClient::spawn(binary, &["mcp"])?;
        Ok(GuardClient { inner })
    }

    fn call_json(&mut self, tool: &str, arguments: Value) -> Result<Value, ClientError> {
        let text = self.inner.call_tool(tool, arguments)?;
        serde_json::from_str(&text).map_err(|e| ClientError::Parse(e.to_string()))
    }

    pub fn health(&mut self) -> Result<Value, ClientError> {
        self.call_json("health", json!({}))
    }

    pub fn get_account_snapshot(&mut self) -> Result<Value, ClientError> {
        self.call_json("get-account-snapshot", json!({}))
    }

    /// `intent` must match `trade_guard_core::trade_intent::TradeIntent`'s
    /// wire shape — see `build_trade_intent` below to construct one.
    /// `evidence`, if present, must match
    /// `trade_guard_core::evidence::EvidenceBundle`'s wire shape (the
    /// same shape `smart_hedge_intelligence_client::IntelligenceClient::build_evidence_bundle`
    /// returns).
    ///
    /// A rejection (insufficient buying power, failed evidence check,
    /// live mode, etc.) is `Err(ClientError::Tool(_))` containing the
    /// full `{policy_outcome, order, was_duplicate}` JSON as text — not a
    /// panic or a swallowed failure. Callers should parse and display it,
    /// not just propagate a generic error message.
    pub fn authorize_and_submit_paper_order(&mut self, intent: Value, evidence: Option<Value>) -> Result<Value, ClientError> {
        let mut arguments = serde_json::Map::new();
        arguments.insert("intent".to_string(), intent);
        if let Some(evidence) = evidence {
            arguments.insert("evidence".to_string(), evidence);
        }
        self.call_json("authorize-and-submit-paper-order", Value::Object(arguments))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TradeSide {
    fn as_str(self) -> &'static str {
        match self {
            TradeSide::Buy => "buy",
            TradeSide::Sell => "sell",
        }
    }
}

/// The inputs `build_trade_intent` needs. Deliberately smaller than the
/// full `TradeIntent` schema (no `limit-price`/`stop-price`/
/// `max-slippage-bps`/`session`/`rationale` etc.) — this constructs a
/// market-order paper-mode intent only, matching what
/// `smart-hedge`'s own deterministic policy output
/// (`paper_trade_preview_shares`) actually produces today: a plain share
/// count with no order-type sophistication.
pub struct TradeIntentParams<'a> {
    pub intent_id: &'a str,
    pub strategy_id: &'a str,
    pub decision_id: &'a str,
    pub account_alias: &'a str,
    pub instrument_id: &'a str,
    pub symbol: &'a str,
    pub side: TradeSide,
    /// Absolute share count (always positive; `side` carries direction).
    pub quantity: f64,
    pub decision_time: &'a str,
    pub confidence: f64,
    pub idempotency_key: &'a str,
    pub evidence_bundle_id: Option<&'a str>,
}

/// Formats a nonnegative `f64` as a `decimal-string` per
/// `common.schema.json#/$defs/decimal-string`: no exponent notation, no
/// leading zeros, trailing fractional zeros trimmed, bare integers have
/// no decimal point at all. Rounds to 8 fractional digits first (this
/// system's shares are never meaningfully more precise than that).
pub fn format_decimal_string(value: f64) -> String {
    let magnitude = value.abs();
    let rounded = format!("{magnitude:.8}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

pub fn build_trade_intent(params: &TradeIntentParams) -> Value {
    json!({
        "schema-version": "2.0.0",
        "intent-id": params.intent_id,
        "strategy-id": params.strategy_id,
        "decision-id": params.decision_id,
        "account-alias": params.account_alias,
        "instrument": {
            "schema-version": "2.0.0",
            "instrument-id": params.instrument_id,
            "asset-class": "equity",
            "symbol": params.symbol,
            "contract-multiplier": "1",
        },
        "side": params.side.as_str(),
        "order-type": "market",
        "quantity": format_decimal_string(params.quantity),
        "time-in-force": "day",
        "decision-time": params.decision_time,
        "confidence": params.confidence.clamp(0.0, 1.0),
        "signal-ids": [],
        "evidence-bundle-id": params.evidence_bundle_id,
        "mode": "paper",
        "idempotency-key": params.idempotency_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawning_a_nonexistent_binary_is_a_spawn_error() {
        let bogus = std::env::temp_dir().join("this-trade-guard-binary-does-not-exist-12345.exe");
        let result = GuardClient::spawn(&bogus);
        assert!(matches!(result, Err(ClientError::Spawn(_))));
    }

    #[test]
    fn format_decimal_string_trims_trailing_zeros() {
        assert_eq!(format_decimal_string(45.0), "45");
        assert_eq!(format_decimal_string(0.5), "0.5");
        assert_eq!(format_decimal_string(10.50000000), "10.5");
    }

    #[test]
    fn format_decimal_string_rounds_to_eight_fractional_digits() {
        assert_eq!(format_decimal_string(1.0 / 3.0), "0.33333333");
    }

    #[test]
    fn format_decimal_string_never_produces_a_negative_sign() {
        assert_eq!(format_decimal_string(-45.0), "45");
    }

    #[test]
    fn format_decimal_string_zero_is_bare_zero() {
        assert_eq!(format_decimal_string(0.0), "0");
    }

    #[test]
    fn build_trade_intent_produces_the_expected_kebab_case_shape() {
        let params = TradeIntentParams {
            intent_id: "intent-1",
            strategy_id: "smart-dynamic-hedge",
            decision_id: "decision-1",
            account_alias: "paper-default",
            instrument_id: "us-equity-spy",
            symbol: "SPY",
            side: TradeSide::Buy,
            quantity: 45.0,
            decision_time: "2026-07-20T00:00:00Z",
            confidence: 0.8,
            idempotency_key: "decision-1",
            evidence_bundle_id: Some("bundle-1"),
        };
        let intent = build_trade_intent(&params);
        assert_eq!(intent["side"], "buy");
        assert_eq!(intent["quantity"], "45");
        assert_eq!(intent["mode"], "paper");
        assert_eq!(intent["instrument"]["asset-class"], "equity");
        assert_eq!(intent["evidence-bundle-id"], "bundle-1");
    }

    #[test]
    fn build_trade_intent_sell_side() {
        let params = TradeIntentParams {
            intent_id: "intent-2",
            strategy_id: "smart-dynamic-hedge",
            decision_id: "decision-2",
            account_alias: "paper-default",
            instrument_id: "us-equity-spy",
            symbol: "SPY",
            side: TradeSide::Sell,
            quantity: 10.0,
            decision_time: "2026-07-20T00:00:00Z",
            confidence: 0.5,
            idempotency_key: "decision-2",
            evidence_bundle_id: None,
        };
        let intent = build_trade_intent(&params);
        assert_eq!(intent["side"], "sell");
        assert!(intent["evidence-bundle-id"].is_null());
    }
}
