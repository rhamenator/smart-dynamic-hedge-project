use serde_json::{json, Value};

/// Exact JSON-tree equivalent of Python's `DEFAULT_CONFIG` dict literal in
/// `config.py`. Deep-merged with any user-supplied JSON before being
/// deserialized into the typed `Config` struct — see `merge.rs` and
/// `loader.rs`.
pub fn default_config_json() -> Value {
    json!({
        "mode": "paper",
        "provider": {
            "kind": "synthetic",
            "alpaca": {
                "data_base_url": "https://data.alpaca.markets",
                "feed": "iex",
                "bar_timeframe": "1Min",
                "bar_limit": 180,
                "timeout_seconds": 8.0
            },
            "evidence_file": "data/evidence.example.json",
            "fred": {
                "enabled": false,
                "series": ["VIXCLS", "DGS2", "DGS10"],
                "timeout_seconds": 8.0
            },
            "rss": { "enabled": false, "feeds": [], "max_items_per_feed": 3 }
        },
        "model": {
            "kind": "heuristic",
            "name": "configure-with-OPENAI_MODEL",
            "timeout_seconds": 20.0,
            "max_evidence_items": 20,
            "max_evidence_chars": 1200,
            "fallback_to_heuristic": true
        },
        "core": {
            "binary": "",
            "tree_steps": 600,
            "auto_build": true,
            "timeout_seconds": 12.0
        },
        "features": {
            "bars_per_year": 98280.0,
            "ewma_lambda": 0.94,
            "short_window": 20,
            "long_window": 90
        },
        "policy": {
            "paper_only": true,
            "max_quote_age_seconds": 45.0,
            "max_spread_bps": 35.0,
            "min_data_quality": 0.65,
            "min_model_confidence_for_band_change": 0.55,
            "min_band_multiplier": 0.50,
            "max_band_multiplier": 3.00,
            "max_abs_trade_shares": 500.0,
            "max_preview_notional": 50_000.0,
            "allow_fractional_shares": true,
            "require_market_open_for_preview": true
        },
        "storage": { "sqlite_path": ".smart_hedge/decisions.sqlite3" },
        "dashboard": { "host": "127.0.0.1", "port": 8765, "cache_seconds": 5.0 },
        "contracts": {
            "SPY": {
                "option_type": "put",
                "exercise_style": "american",
                "strike": 100.0,
                "days_to_expiry": 30.0,
                "contracts": 1,
                "multiplier": 100.0,
                "current_shares": 0.0,
                "rate": 0.045,
                "dividend_yield": 0.012,
                "implied_volatility": 0.20,
                "base_no_trade_band_shares": 2.0
            }
        }
    })
}
