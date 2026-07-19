//! End-to-end tests of `build_features` against realistic snapshots,
//! each tagged with the requirement it verifies (see `requirements/LLR.md`).

use serde_json::Value;
use smart_hedge_config::FeaturesConfig;
use smart_hedge_models::{Bar, EvidenceItem, MarketSnapshot, Quote};

use crate::build::build_features;

fn config() -> FeaturesConfig {
    FeaturesConfig { bars_per_year: 98_280.0, ewma_lambda: 0.94, short_window: 20, long_window: 90 }
}

fn bar(i: usize, close: f64, volume: f64) -> Bar {
    Bar {
        timestamp: format!("2026-07-19T00:{i:02}:00Z"),
        open: close,
        high: close,
        low: close,
        close,
        volume,
    }
}

fn snapshot_with_bars(bars: Vec<Bar>) -> MarketSnapshot {
    let last_close = bars.last().map(|b| b.close).unwrap_or(100.0);
    MarketSnapshot::new(
        "TEST",
        Quote::new("TEST", last_close - 0.01, last_close + 0.01, last_close, "2026-07-19T00:00:00Z", "test", "open"),
        bars,
        Vec::new(),
    )
}

/// SDH-LLR-111: fewer than 2 log returns (i.e. 0 or 1 usable closes)
/// marks `realized_volatility` missing rather than defaulting it.
#[test]
fn realized_volatility_is_missing_with_fewer_than_two_closes() {
    let snapshot = snapshot_with_bars(vec![bar(0, 100.0, 1000.0)]);
    let features = build_features(&snapshot, &config());
    assert!(features.missing.contains(&"realized_volatility".to_string()));
    assert_eq!(features.values.get("realized_volatility"), Some(&Value::Null));
}

/// SDH-LLR-111 (positive case): with enough history, realized volatility
/// is present and is not silently `0.0`.
#[test]
fn realized_volatility_is_present_with_enough_closes() {
    let closes = [100.0, 101.0, 99.5, 102.0, 98.0, 103.0];
    let bars: Vec<Bar> = closes.iter().enumerate().map(|(i, &c)| bar(i, c, 1000.0)).collect();
    let snapshot = snapshot_with_bars(bars);
    let features = build_features(&snapshot, &config());
    assert!(!features.missing.contains(&"realized_volatility".to_string()));
    match features.values.get("realized_volatility") {
        Some(Value::Number(n)) => assert!(n.as_f64().unwrap() > 0.0),
        other => panic!("expected a positive number, got {other:?}"),
    }
}

/// SDH-LLR-112: volume z-score requires at least 21 bars; with fewer, it
/// is `None` and a warning (not a blocker) is recorded.
#[test]
fn volume_zscore_unavailable_with_fewer_than_21_bars() {
    let bars: Vec<Bar> = (0..10).map(|i| bar(i, 100.0 + i as f64, 1000.0)).collect();
    let snapshot = snapshot_with_bars(bars);
    let features = build_features(&snapshot, &config());
    assert_eq!(features.values.get("volume_zscore"), Some(&Value::Null));
    assert!(features.warnings.contains(&"volume_zscore_unavailable".to_string()));
}

/// SDH-LLR-112 (positive case): 21+ bars with varying volume produces a
/// real z-score.
#[test]
fn volume_zscore_present_with_21_or_more_bars() {
    let bars: Vec<Bar> =
        (0..25).map(|i| bar(i, 100.0 + i as f64 * 0.1, 1000.0 + (i as f64 * 37.0) % 200.0)).collect();
    let snapshot = snapshot_with_bars(bars);
    let features = build_features(&snapshot, &config());
    assert!(!features.warnings.contains(&"volume_zscore_unavailable".to_string()));
    assert!(matches!(features.values.get("volume_zscore"), Some(Value::Number(_))));
}

/// SDH-LLR-113: a realized volatility at/below the `1e-9` floor (a
/// perfectly flat price series has zero volatility) must not produce a
/// trend score — this is the guard against dividing by ~0.
#[test]
fn trend_score_is_none_when_realized_volatility_is_at_the_floor() {
    // A perfectly flat close series: every log return is exactly 0, so
    // realized volatility is exactly 0.0, which is not `> 1e-9`.
    let bars: Vec<Bar> = (0..30).map(|i| bar(i, 100.0, 1000.0)).collect();
    let snapshot = snapshot_with_bars(bars);
    let features = build_features(&snapshot, &config());
    assert_eq!(features.values.get("trend_score"), Some(&Value::Null));
}

/// SDH-LLR-113 (positive case): enough history and nonzero volatility
/// produces a real trend score.
#[test]
fn trend_score_is_present_with_a_real_trend_and_volatility() {
    let mut bars = Vec::new();
    for i in 0..30 {
        // Trending up with small noise so realized volatility is nonzero.
        let noise = if i % 2 == 0 { 0.05 } else { -0.03 };
        bars.push(bar(i, 100.0 + i as f64 * 0.5 + noise, 1000.0));
    }
    let snapshot = snapshot_with_bars(bars);
    let mut cfg = config();
    cfg.short_window = 10;
    cfg.long_window = 20;
    let features = build_features(&snapshot, &cfg);
    assert!(matches!(features.values.get("trend_score"), Some(Value::Number(_))));
}

/// SDH-LLR-110: data-quality composition — a "perfect" snapshot (valid
/// midpoint, finite spread, full bar history, nothing missing, no
/// evidence) should score at or very near 1.0.
#[test]
fn data_quality_is_high_for_a_complete_snapshot() {
    let bars: Vec<Bar> = (0..100).map(|i| bar(i, 100.0 + (i as f64 * 0.01), 1000.0)).collect();
    let snapshot = snapshot_with_bars(bars);
    let features = build_features(&snapshot, &config());
    assert!(features.data_quality > 0.9, "expected high data quality, got {}", features.data_quality);
}

/// SDH-LLR-110: an invalid quote (non-positive bid/ask/last) plus no bar
/// history should score data quality near 0, not silently pass as "fine".
#[test]
fn data_quality_is_low_for_a_degenerate_snapshot() {
    let snapshot = MarketSnapshot::new(
        "TEST",
        Quote::new("TEST", 0.0, 0.0, 0.0, "2026-07-19T00:00:00Z", "test", "closed"),
        Vec::new(),
        Vec::new(),
    );
    let features = build_features(&snapshot, &config());
    assert!(features.data_quality < 0.3, "expected low data quality, got {}", features.data_quality);
}

/// SDH-LLR-110: evidence quality feeds into the overall data-quality
/// score when evidence is present.
#[test]
fn evidence_quality_influences_data_quality() {
    let bars: Vec<Bar> = (0..100).map(|i| bar(i, 100.0 + (i as f64 * 0.01), 1000.0)).collect();
    let mut snapshot = snapshot_with_bars(bars);
    snapshot.evidence = vec![EvidenceItem {
        evidence_id: "e1".to_string(),
        kind: "news".to_string(),
        title: "Low quality item".to_string(),
        timestamp: "2026-07-19T00:00:00Z".to_string(),
        source: "test".to_string(),
        value: Value::Null,
        text: String::new(),
        quality: 0.0,
        untrusted_text: true,
    }];
    let features = build_features(&snapshot, &config());
    let without_evidence = build_features(&snapshot_with_evidence_removed(&snapshot), &config());
    assert!(features.data_quality < without_evidence.data_quality);
}

fn snapshot_with_evidence_removed(snapshot: &MarketSnapshot) -> MarketSnapshot {
    let mut cloned = snapshot.clone();
    cloned.evidence.clear();
    cloned
}

/// Evidence IDs pass through into `FeatureSet.evidence_ids` unchanged —
/// this is what the policy gate's evidence-citation check
/// (SDH-LLR-006/SDH-HLR-090) relies on.
#[test]
fn evidence_ids_pass_through_to_the_feature_set() {
    let mut snapshot = snapshot_with_bars(vec![bar(0, 100.0, 1000.0)]);
    snapshot.evidence = vec![EvidenceItem {
        evidence_id: "known-id".to_string(),
        kind: "news".to_string(),
        title: "Something".to_string(),
        timestamp: "2026-07-19T00:00:00Z".to_string(),
        source: "test".to_string(),
        value: Value::Null,
        text: String::new(),
        quality: 0.5,
        untrusted_text: true,
    }];
    let features = build_features(&snapshot, &config());
    assert_eq!(features.evidence_ids, vec!["known-id".to_string()]);
}
