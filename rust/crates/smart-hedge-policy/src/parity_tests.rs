//! Direct transcription of `tests/test_policy.py`'s four cases (parity with
//! the existing Python test suite), plus additional boundary coverage for
//! blockers the Python suite doesn't currently exercise — consistent with
//! this project's "raise the testing bar past what already exists, don't
//! just match it" practice established in the sibling Rust repositories.

use smart_hedge_config::EnvOverrides;
use smart_hedge_models::{
    Bar, CoreGreeks, CoreHedge, CoreInputs, CorePricing, CoreResponse, CoreRisk, EvidenceItem, FeatureSet,
    MarketSnapshot, ModelAssessment, Quote, TimestampUtc,
};

use crate::evaluate::evaluate_policy;

fn base_config() -> smart_hedge_config::Config {
    let mut loaded =
        smart_hedge_config::load_config(None, &EnvOverrides::default(), std::path::Path::new("/root")).unwrap();
    loaded.config.policy.max_preview_notional = 1_000_000.0;
    loaded.config
}

fn base_snapshot(timestamp: String, market_state: &str) -> MarketSnapshot {
    MarketSnapshot::new(
        "TEST",
        Quote::new("TEST", 99.99, 100.01, 100.0, timestamp.clone(), "unit-test", market_state),
        vec![Bar { timestamp, open: 100.0, high: 101.0, low: 99.0, close: 100.0, volume: 1000.0 }],
        Vec::<EvidenceItem>::new(),
    )
}

fn base_features() -> FeatureSet {
    FeatureSet {
        values: Default::default(),
        missing: vec![],
        warnings: vec![],
        data_quality: 1.0,
        evidence_ids: vec![],
    }
}

fn base_core() -> CoreResponse {
    CoreResponse {
        engine_version: "test".to_string(),
        inputs: CoreInputs {
            spot: 100.0,
            strike: 100.0,
            rate: 0.045,
            dividend_yield: 0.012,
            volatility: 0.20,
            days_to_expiry: 30.0,
            option_type: "put".to_string(),
            exercise_style: "american".to_string(),
            contracts: 1,
            multiplier: 100.0,
            current_shares: 0.0,
            tree_steps: 600,
            base_no_trade_band_shares: 2.0,
        },
        pricing: CorePricing { model_price: 3.5, european_price: 3.4, early_exercise_premium: 0.1 },
        greeks: CoreGreeks {
            delta: -0.45,
            gamma: 0.02,
            vega_per_vol_point: 0.15,
            theta_per_calendar_day: -0.01,
            rho_per_rate_point: -0.05,
        },
        hedge: CoreHedge {
            option_position_delta_shares: -45.0,
            target_stock_shares: 10.0,
            raw_trade_shares: 10.0,
            recommended_trade_shares: 10.0,
            action: "paper_rebalance_preview".to_string(),
            stock_notional: 1000.0,
        },
        risk: CoreRisk { position_gamma_pnl_for_1pct_move: 1.0 },
    }
}

fn base_assessment() -> ModelAssessment {
    ModelAssessment {
        advisor_kind: "test".to_string(),
        model: "test".to_string(),
        regime: "calm".to_string(),
        confidence: 0.9,
        hedge_urgency: 0.3,
        band_multiplier: 2.0,
        summary: "test".to_string(),
        evidence_ids: vec![],
        risks: vec![],
        scenario_spot_shocks: vec![-0.05, 0.05],
        data_requests: vec![],
        raw_response_id: String::new(),
        fallback_reason: String::new(),
    }
}

/// `test_model_can_only_change_band_not_target`
#[test]
fn model_can_only_change_band_not_target() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert_eq!(decision.target_stock_shares, 10.0);
    assert_eq!(decision.effective_no_trade_band_shares, 4.0);
    assert_eq!(decision.paper_trade_preview_shares, 10.0);
    assert!(!decision.live_execution_allowed);
}

/// `test_stale_quote_blocks_preview`
#[test]
fn stale_quote_blocks_preview() {
    let config = base_config();
    // Any fixed far-past timestamp is stale relative to "now" regardless
    // of what "now" actually is when the test runs.
    let snapshot = base_snapshot("2000-01-01T00:00:00Z".to_string(), "open");
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"STALE_QUOTE".to_string()));
    assert_eq!(decision.paper_trade_preview_shares, 0.0);
}

/// `test_unknown_model_citation_is_blocked`
#[test]
fn unknown_model_citation_is_blocked() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut assessment = base_assessment();
    assessment.evidence_ids = vec!["invented-id".to_string()];
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &assessment, TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"MODEL_CITED_UNKNOWN_EVIDENCE".to_string()));
}

/// `test_low_confidence_cannot_change_band`
#[test]
fn low_confidence_cannot_change_band() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut assessment = base_assessment();
    assessment.confidence = 0.1;
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &assessment, TimestampUtc::now());
    assert_eq!(decision.effective_no_trade_band_shares, 2.0);
    assert!(decision.warnings.contains(&"model_confidence_too_low_for_band_change".to_string()));
}

// --- Additional boundary coverage beyond the existing Python suite ---

#[test]
fn invalid_quote_blocks_when_bid_ask_and_last_are_all_nonpositive() {
    let config = base_config();
    let mut snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    snapshot.quote.bid = 0.0;
    snapshot.quote.ask = 0.0;
    snapshot.quote.last = 0.0;
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"INVALID_QUOTE".to_string()));
    assert_eq!(decision.action, "observe_blocked");
}

#[test]
fn wide_spread_is_blocked() {
    let config = base_config();
    let mut snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    snapshot.quote.bid = 90.0;
    snapshot.quote.ask = 110.0; // ~20% spread, far past the 35bps default limit
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"SPREAD_TOO_WIDE".to_string()));
}

#[test]
fn low_data_quality_is_blocked() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut features = base_features();
    features.data_quality = 0.1;
    let decision = evaluate_policy(&config, &snapshot, &features, &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"DATA_QUALITY_TOO_LOW".to_string()));
}

#[test]
fn market_not_open_blocks_an_out_of_band_preview() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "closed");
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"MARKET_NOT_OPEN".to_string()));
}

#[test]
fn market_closed_does_not_block_a_hold_inside_band_decision() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "closed");
    let mut core = base_core();
    core.hedge.target_stock_shares = 0.0; // inside the +/-2 (x2 multiplier = 4) band
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &core, &base_assessment(), TimestampUtc::now());
    assert!(!decision.blocking_reasons.contains(&"MARKET_NOT_OPEN".to_string()));
    assert_eq!(decision.action, "hold_inside_effective_band");
}

#[test]
fn trade_share_limit_blocks_an_oversized_preview() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut core = base_core();
    core.hedge.target_stock_shares = 10_000.0; // far past max_abs_trade_shares (500 default)
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &core, &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"TRADE_SHARE_LIMIT".to_string()));
}

#[test]
fn preview_notional_limit_blocks_when_configured_tightly() {
    let mut config = base_config();
    config.policy.max_preview_notional = 1.0; // effectively any nonzero trade exceeds this
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"PREVIEW_NOTIONAL_LIMIT".to_string()));
}

#[test]
fn fractional_shares_disallowed_rounds_half_to_even() {
    let mut config = base_config();
    config.policy.allow_fractional_shares = false;
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut core = base_core();
    // raw_trade = 10.5 - 0.0 = 10.5, effective_band = 2.0 * 2.0 = 4.0, outside
    // band, preview = 10.5 -> round-half-to-even -> 10.0.
    core.hedge.target_stock_shares = 10.5;
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &core, &base_assessment(), TimestampUtc::now());
    assert_eq!(decision.paper_trade_preview_shares, 10.0);
}

#[test]
fn nonfinite_core_value_is_blocked() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let mut core = base_core();
    // A raw_trade computed from two large, opposite-signed finite values
    // that overflows to infinity on subtraction — the case the Python
    // `math.isfinite` check after arithmetic (not the JSON `null` case) is
    // actually guarding against.
    core.hedge.target_stock_shares = f64::MAX;
    core.inputs.current_shares = -f64::MAX;
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &core, &base_assessment(), TimestampUtc::now());
    assert!(decision.blocking_reasons.contains(&"NONFINITE_CORE_VALUE".to_string()));
}

#[test]
fn blocked_decision_zeroes_preview_trade_and_notional() {
    let config = base_config();
    let mut snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    snapshot.quote.bid = 0.0;
    snapshot.quote.ask = 0.0;
    snapshot.quote.last = 0.0; // INVALID_QUOTE
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(!decision.paper_preview_approved);
    assert_eq!(decision.paper_trade_preview_shares, 0.0);
    assert_eq!(decision.paper_trade_preview_notional, 0.0);
    assert_eq!(decision.action, "observe_blocked");
}

#[test]
fn live_execution_is_never_allowed_regardless_of_inputs() {
    let config = base_config();
    let snapshot = base_snapshot(TimestampUtc::now().to_iso_string(), "open");
    let decision = evaluate_policy(&config, &snapshot, &base_features(), &base_core(), &base_assessment(), TimestampUtc::now());
    assert!(!decision.live_execution_allowed);
}
