use serde::{Deserialize, Serialize};

/// Exact structural port of the JSON `cpp/smart_dynamic_hedge.cpp --json`
/// emits (`result_json` in that file). The C++ core writes `null` for any
/// non-finite (`NaN`/`Infinity`) numeric field, so a field that should be
/// non-finite fails to deserialize into `f64` here — this is the Rust
/// equivalent of Python's `float(hedge["target_stock_shares"])` raising
/// `TypeError` on `None` and being caught as "deterministic core response
/// is malformed" in `core_bridge`/`policy`. Unlike Python, that validation
/// happens once, at the JSON-parsing boundary (deserializing this struct),
/// rather than being re-checked by every consumer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreResponse {
    pub engine_version: String,
    pub inputs: CoreInputs,
    pub pricing: CorePricing,
    pub greeks: CoreGreeks,
    pub hedge: CoreHedge,
    pub risk: CoreRisk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreInputs {
    pub spot: f64,
    pub strike: f64,
    pub rate: f64,
    pub dividend_yield: f64,
    pub volatility: f64,
    pub days_to_expiry: f64,
    pub option_type: String,
    pub exercise_style: String,
    pub contracts: i64,
    pub multiplier: f64,
    pub current_shares: f64,
    pub tree_steps: i64,
    #[serde(default)]
    pub base_no_trade_band_shares: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorePricing {
    pub model_price: f64,
    pub european_price: f64,
    pub early_exercise_premium: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreGreeks {
    pub delta: f64,
    pub gamma: f64,
    pub vega_per_vol_point: f64,
    pub theta_per_calendar_day: f64,
    pub rho_per_rate_point: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreHedge {
    pub option_position_delta_shares: f64,
    pub target_stock_shares: f64,
    pub raw_trade_shares: f64,
    pub recommended_trade_shares: f64,
    pub action: String,
    pub stock_notional: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreRisk {
    pub position_gamma_pnl_for_1pct_move: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_json() -> &'static str {
        r#"{
            "engine_version":"1.0",
            "inputs":{"spot":100.0,"strike":100.0,"rate":0.045,"dividend_yield":0.012,
                "volatility":0.2,"days_to_expiry":30.0,"option_type":"put",
                "exercise_style":"american","contracts":1,"multiplier":100.0,
                "current_shares":0.0,"tree_steps":600,"base_no_trade_band_shares":2.0},
            "pricing":{"model_price":3.5,"european_price":3.4,"early_exercise_premium":0.1},
            "greeks":{"delta":-0.45,"gamma":0.02,"vega_per_vol_point":0.15,
                "theta_per_calendar_day":-0.01,"rho_per_rate_point":-0.05},
            "hedge":{"option_position_delta_shares":-45.0,"target_stock_shares":45.0,
                "raw_trade_shares":45.0,"recommended_trade_shares":45.0,
                "action":"paper_rebalance_preview","stock_notional":4500.0},
            "risk":{"position_gamma_pnl_for_1pct_move":1.0}
        }"#
    }

    #[test]
    fn deserializes_a_well_formed_response() {
        let parsed: CoreResponse = serde_json::from_str(sample_json()).unwrap();
        assert_eq!(parsed.hedge.target_stock_shares, 45.0);
        assert_eq!(parsed.inputs.current_shares, 0.0);
    }

    #[test]
    fn null_for_a_non_finite_field_fails_to_deserialize() {
        let json = sample_json().replace("\"target_stock_shares\":45.0", "\"target_stock_shares\":null");
        let result: Result<CoreResponse, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "a null (non-finite) core value must not silently become 0.0");
    }

    #[test]
    fn missing_required_field_fails_to_deserialize() {
        let json = sample_json().replace("\"target_stock_shares\":45.0,", "");
        let result: Result<CoreResponse, _> = serde_json::from_str(&json);
        assert!(result.is_err());
    }

    #[test]
    fn missing_base_no_trade_band_shares_defaults_to_zero() {
        let json = sample_json().replace(",\"base_no_trade_band_shares\":2.0", "");
        let parsed: CoreResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.inputs.base_no_trade_band_shares, 0.0);
    }
}
