use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Typed port of Python's `DEFAULT_CONFIG` dict shape. Unlike the Python
/// version (a `dict[str, Any]` merged at runtime with no schema), this is a
/// real, statically-checked structure — the deep-merge still happens on a
/// `serde_json::Value` tree (see `merge.rs`) so user-supplied partial JSON
/// files keep working exactly like before, but the *result* is this type,
/// not an untyped dict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub mode: String,
    pub provider: ProviderConfig,
    pub model: ModelConfig,
    pub core: CoreConfig,
    pub features: FeaturesConfig,
    pub policy: PolicyConfig,
    pub storage: StorageConfig,
    pub dashboard: DashboardConfig,
    pub contracts: BTreeMap<String, ContractConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: String,
    pub alpaca: AlpacaConfig,
    pub evidence_file: String,
    pub fred: FredConfig,
    pub rss: RssConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlpacaConfig {
    pub data_base_url: String,
    pub feed: String,
    pub bar_timeframe: String,
    pub bar_limit: i64,
    pub timeout_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FredConfig {
    pub enabled: bool,
    pub series: Vec<String>,
    pub timeout_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RssConfig {
    pub enabled: bool,
    pub feeds: Vec<String>,
    pub max_items_per_feed: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub kind: String,
    pub name: String,
    pub timeout_seconds: f64,
    pub max_evidence_items: i64,
    pub max_evidence_chars: i64,
    pub fallback_to_heuristic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreConfig {
    pub binary: String,
    pub tree_steps: i64,
    pub auto_build: bool,
    pub timeout_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeaturesConfig {
    pub bars_per_year: f64,
    pub ewma_lambda: f64,
    pub short_window: i64,
    pub long_window: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub paper_only: bool,
    pub max_quote_age_seconds: f64,
    pub max_spread_bps: f64,
    pub min_data_quality: f64,
    pub min_model_confidence_for_band_change: f64,
    pub min_band_multiplier: f64,
    pub max_band_multiplier: f64,
    pub max_abs_trade_shares: f64,
    pub max_preview_notional: f64,
    pub allow_fractional_shares: bool,
    pub require_market_open_for_preview: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageConfig {
    pub sqlite_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub host: String,
    pub port: i64,
    pub cache_seconds: f64,
}

/// A contract entry in `config.contracts`. The deep-merge in `merge.rs`
/// only merges a symbol's fields onto its *existing* base entry — a
/// brand-new symbol a user adds that isn't already in the defaults (e.g.
/// `"QQQ": {"strike": 50.0}` alongside the built-in `"SPY"`) ends up with
/// *only* the fields the user specified, exactly like Python's
/// `_deep_merge`. `core_bridge.py`'s `run_core` tolerates that for most
/// fields via `contract.get(key, default)`, so the fields below mirror
/// those exact per-field defaults with `#[serde(default = ...)]`.
/// `strike`, `days_to_expiry`, and `implied_volatility` are the three
/// fields Python indexes directly (`contract["strike"]`, a `KeyError` if
/// absent) rather than defaulting — kept required here too, which fails
/// fast at config-load time instead of Python's behavior of failing later,
/// only once that specific symbol is actually priced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractConfig {
    #[serde(default = "default_option_type")]
    pub option_type: String,
    #[serde(default = "default_exercise_style")]
    pub exercise_style: String,
    pub strike: f64,
    pub days_to_expiry: f64,
    #[serde(default)]
    pub contracts: i64,
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
    #[serde(default)]
    pub current_shares: f64,
    #[serde(default)]
    pub rate: f64,
    #[serde(default)]
    pub dividend_yield: f64,
    pub implied_volatility: f64,
    #[serde(default)]
    pub base_no_trade_band_shares: f64,
}

fn default_option_type() -> String {
    "call".to_string()
}

fn default_exercise_style() -> String {
    "american".to_string()
}

fn default_multiplier() -> f64 {
    100.0
}
