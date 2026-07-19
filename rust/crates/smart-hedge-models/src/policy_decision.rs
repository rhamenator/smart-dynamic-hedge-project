use serde::{Deserialize, Serialize};

/// Port of the `applied_limits` dict Python's `evaluate_policy` always
/// builds with exactly this fixed set of keys — a typed struct here instead
/// of `dict[str, float | int | bool | str]`, since the key set is never
/// actually dynamic in practice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedLimits {
    pub policy_version: String,
    pub paper_only: bool,
    pub quote_age_seconds: f64,
    pub max_quote_age_seconds: f64,
    pub spread_bps: f64,
    pub max_spread_bps: f64,
    pub data_quality: f64,
    pub min_data_quality: f64,
    pub model_confidence: f64,
    pub band_multiplier_applied: f64,
    pub max_abs_trade_shares: f64,
    pub max_preview_notional: f64,
}

/// Port of `smart_hedge.models.PolicyDecision`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub action: String,
    pub paper_preview_approved: bool,
    pub live_execution_allowed: bool,
    pub effective_no_trade_band_shares: f64,
    pub target_stock_shares: f64,
    pub current_stock_shares: f64,
    pub raw_trade_shares: f64,
    pub paper_trade_preview_shares: f64,
    pub paper_trade_preview_notional: f64,
    pub blocking_reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub applied_limits: AppliedLimits,
}
