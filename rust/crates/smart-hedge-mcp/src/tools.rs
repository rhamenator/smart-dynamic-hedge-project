use serde_json::{json, Value};
use smart_hedge_config::{ContractConfig, StrikeSpec};
use smart_hedge_engine::{ContractOverrides, SmartHedgeEngine};

/// Renders a `Value` the way Python's `_json` helper does:
/// `json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False)`.
/// `serde_json::to_string_pretty` already produces sorted keys for free —
/// see `smart_hedge_store::canonical`'s doc comment for why
/// `serde_json::Value::Object` is `BTreeMap`-backed (no `preserve_order`
/// feature enabled anywhere in this workspace).
fn render(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("Value serialization is infallible")
}

fn tool_error(message: impl std::fmt::Display) -> Result<String, String> {
    Err(message.to_string())
}

/// Port of the `health` MCP tool.
pub fn health(engine: &SmartHedgeEngine) -> Result<String, String> {
    Ok(render(&engine.health()))
}

/// Port of the `get_market_recommendation` MCP tool.
pub fn get_market_recommendation(engine: &SmartHedgeEngine, symbol: &str) -> Result<String, String> {
    match engine.recommendation(symbol, &ContractOverrides::default()) {
        Ok(value) => Ok(render(&value)),
        Err(e) => tool_error(e),
    }
}

#[derive(Debug, Clone)]
pub struct PriceOptionArgs {
    pub symbol: String,
    pub spot: f64,
    pub strike: f64,
    pub implied_volatility: f64,
    pub days_to_expiry: f64,
    pub option_type: String,
    pub exercise_style: String,
    pub contracts: i64,
    pub current_shares: f64,
}

impl Default for PriceOptionArgs {
    fn default() -> Self {
        PriceOptionArgs {
            symbol: "SPY".to_string(),
            spot: 100.0,
            strike: 100.0,
            implied_volatility: 0.20,
            days_to_expiry: 30.0,
            option_type: "put".to_string(),
            exercise_style: "american".to_string(),
            contracts: 1,
            current_shares: 0.0,
        }
    }
}

/// Builds the contract `price_option` runs the core against: the symbol's
/// configured contract as a base (for `multiplier`/`rate`/`dividend_yield`/
/// `base_no_trade_band_shares`, which aren't tool parameters), or — for a
/// symbol with no configured contract — a fresh `ContractConfig` built from
/// only `strike`/`implied_volatility` and letting its own
/// `#[serde(default = ...)]` per-field defaults fill in the rest (the same
/// defaulting `smart_hedge_config::loader` already relies on for a
/// brand-new contract symbol — see `SDH-LLR-025`). Every tool-provided
/// field then overrides the base directly. **Deviation from Python**:
/// `mcp_server.py`'s `price_option` starts from
/// `_engine().contract_for(symbol.upper())`, whose exact behavior for an
/// unconfigured symbol isn't visible from this crate; this port's
/// "configured-or-schema-defaulted" fallback is a reasonable, documented,
/// directly-testable choice for the same observable contract (a raw
/// pricing utility that works for any symbol, not just configured ones).
fn build_contract(loaded: &smart_hedge_config::LoadedConfig, args: &PriceOptionArgs) -> ContractConfig {
    let mut base = loaded.config.contracts.get(&args.symbol).cloned().unwrap_or_else(|| {
        serde_json::from_value(json!({"strike": args.strike, "implied_volatility": args.implied_volatility}))
            .expect("ContractConfig defaults every field except strike/implied_volatility")
    });
    base.strike = StrikeSpec::Fixed(args.strike);
    base.implied_volatility = args.implied_volatility;
    base.days_to_expiry = args.days_to_expiry;
    base.option_type = args.option_type.clone();
    base.exercise_style = args.exercise_style.clone();
    base.contracts = args.contracts;
    base.current_shares = args.current_shares;
    base.expiry = None;
    base
}

/// Port of the `price_option` MCP tool: runs the deterministic core
/// directly, with no market-data retrieval and no adviser/policy/store
/// involvement — a pure pricing calculator.
pub fn price_option(engine: &SmartHedgeEngine, args: &PriceOptionArgs) -> Result<String, String> {
    let loaded = engine.loaded_config();
    let contract = build_contract(loaded, args);
    match smart_hedge_core_bridge::run_core(loaded, engine.project_root(), engine.cpp_source(), &contract, args.spot, args.strike) {
        Ok(response) => Ok(render(&serde_json::to_value(response).expect("CoreResponse serialization is infallible"))),
        Err(e) => tool_error(e),
    }
}

/// Port of the `replay_decision` MCP tool.
pub fn replay_decision(engine: &SmartHedgeEngine, decision_id: &str) -> Result<String, String> {
    match engine.replay(decision_id) {
        Ok(value) => Ok(render(&value)),
        Err(e) => tool_error(e),
    }
}

/// Port of the `list_recent_decisions` MCP tool.
pub fn list_recent_decisions(engine: &SmartHedgeEngine, limit: i64, symbol: &str) -> Result<String, String> {
    let symbol_filter = if symbol.is_empty() { None } else { Some(symbol) };
    match engine.recent(limit, symbol_filter) {
        Ok(values) => Ok(render(&Value::Array(values))),
        Err(e) => tool_error(e),
    }
}

/// Port of the `get_policy_snapshot` MCP tool.
pub fn get_policy_snapshot(engine: &SmartHedgeEngine) -> Result<String, String> {
    let loaded = engine.loaded_config();
    let policy = serde_json::to_value(&loaded.config.policy).expect("PolicyConfig serialization is infallible");
    Ok(render(&json!({
        "mode": loaded.config.mode,
        "policy": policy,
        "broker_order_endpoint_present": false,
        "live_execution_allowed": false,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;

    fn loaded_config_with_contracts(contracts_json: &str) -> smart_hedge_config::LoadedConfig {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smart-hedge-mcp-tools-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, format!(r#"{{"contracts": {contracts_json}}}"#)).unwrap();
        let loaded = smart_hedge_config::load_config(Some(&path), &EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        loaded
    }

    #[test]
    fn build_contract_uses_the_configured_base_for_a_known_symbol() {
        let loaded = loaded_config_with_contracts(
            r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2, "multiplier": 50.0}}"#,
        );
        let args = PriceOptionArgs { symbol: "SPY".to_string(), strike: 150.0, ..Default::default() };
        let contract = build_contract(&loaded, &args);
        assert_eq!(contract.multiplier, 50.0); // preserved from the configured base
        assert_eq!(contract.strike, StrikeSpec::Fixed(150.0)); // overridden by the tool argument
    }

    #[test]
    fn build_contract_defaults_reasonably_for_an_unconfigured_symbol() {
        let loaded = loaded_config_with_contracts(r#"{"SPY": {"strike": 100.0, "implied_volatility": 0.2}}"#);
        let args = PriceOptionArgs { symbol: "ZZZZ".to_string(), strike: 42.0, contracts: 3, ..Default::default() };
        let contract = build_contract(&loaded, &args);
        assert_eq!(contract.strike, StrikeSpec::Fixed(42.0));
        assert_eq!(contract.contracts, 3);
        assert_eq!(contract.multiplier, 100.0); // schema default
    }

    #[test]
    fn build_contract_never_leaves_an_atm_strike_or_an_expiry_date() {
        let loaded = loaded_config_with_contracts(r#"{"SPY": {"strike": "ATM", "implied_volatility": 0.2, "expiry": "2026-12-19"}}"#);
        let args = PriceOptionArgs { symbol: "SPY".to_string(), strike: 88.0, ..Default::default() };
        let contract = build_contract(&loaded, &args);
        assert_eq!(contract.strike, StrikeSpec::Fixed(88.0));
        assert!(contract.expiry.is_none());
    }

    #[test]
    fn get_policy_snapshot_never_claims_live_execution_or_an_order_endpoint() {
        let loaded = loaded_config_with_contracts("{}");
        let root = std::env::temp_dir();
        let engine = SmartHedgeEngine::new(loaded, root.clone(), root.join("nonexistent.cpp")).unwrap();
        let text = get_policy_snapshot(&engine).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["broker_order_endpoint_present"], false);
        assert_eq!(value["live_execution_allowed"], false);
        assert_eq!(value["mode"], "paper");
    }

    #[test]
    fn list_recent_decisions_with_empty_symbol_string_means_no_filter() {
        let loaded = loaded_config_with_contracts("{}");
        let root = std::env::temp_dir();
        let engine = SmartHedgeEngine::new(loaded, root.clone(), root.join("nonexistent.cpp")).unwrap();
        let text = list_recent_decisions(&engine, 10, "").unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert!(value.is_array());
    }
}
