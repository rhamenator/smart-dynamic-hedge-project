use std::collections::BTreeSet;

use smart_hedge_config::Config;
use smart_hedge_models::{AppliedLimits, CoreResponse, FeatureSet, MarketSnapshot, ModelAssessment, PolicyDecision, TimestampUtc};

use crate::rounding::round_half_to_even;

pub const POLICY_VERSION: &str = "paper-guard-v1";

fn clamp_like_python(x: f64, lo: f64, hi: f64) -> f64 {
    // Deliberately not `f64::clamp`, which panics if `lo > hi` — a
    // malformed config (e.g. `min_band_multiplier > max_band_multiplier`)
    // must not be able to crash the policy gate. `x.max(lo).min(hi)`
    // matches Python's `min(max(x, lo), hi)` exactly, including its
    // behavior when `lo > hi` (silently no-op-ish rather than panicking).
    x.max(lo).min(hi)
}

/// Port of `smart_hedge.policy.evaluate_policy`.
///
/// Unlike Python, this is infallible: Python's only failure mode was
/// "deterministic core response is malformed" (a `KeyError`/`TypeError`
/// from indexing an untyped dict), which is now a `CoreResponse`
/// deserialization error caught at the JSON-parsing boundary — by the time
/// a `&CoreResponse` reaches this function, that shape has already been
/// validated. Every existing Python test case's inputs are well-formed, so
/// this is not a parity loss for any covered behavior, only a move of
/// where the "malformed" class of error surfaces.
pub fn evaluate_policy(
    config: &Config,
    snapshot: &MarketSnapshot,
    features: &FeatureSet,
    core: &CoreResponse,
    assessment: &ModelAssessment,
) -> PolicyDecision {
    let policy = &config.policy;
    let mut blockers: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = features.warnings.clone();

    if !policy.paper_only || config.mode.to_lowercase() != "paper" {
        blockers.push("LIVE_MODE_FORBIDDEN".to_string());
    }

    let midpoint = snapshot.quote.midpoint();
    if !midpoint.is_finite() || midpoint <= 0.0 {
        blockers.push("INVALID_QUOTE".to_string());
    }

    let quote_time = TimestampUtc::parse_flexible(&snapshot.quote.timestamp);
    let max_age = policy.max_quote_age_seconds;
    let quote_age = match quote_time {
        Some(qt) => qt.seconds_until(&TimestampUtc::now()).max(0.0),
        None => f64::INFINITY,
    };
    if quote_age > max_age {
        blockers.push("STALE_QUOTE".to_string());
    }

    let spread = snapshot.quote.spread_bps();
    let max_spread = policy.max_spread_bps;
    if !spread.is_finite() || spread > max_spread {
        blockers.push("SPREAD_TOO_WIDE".to_string());
    }

    let min_quality = policy.min_data_quality;
    if features.data_quality < min_quality {
        blockers.push("DATA_QUALITY_TOO_LOW".to_string());
    }

    if !features.missing.is_empty() {
        warnings.push(format!("missing_features:{}", features.missing.join(",")));
    }

    let allowed_evidence: BTreeSet<&str> = features.evidence_ids.iter().map(String::as_str).collect();
    let mut unknown_citations: Vec<&str> = assessment
        .evidence_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !allowed_evidence.contains(id))
        .collect();
    unknown_citations.sort_unstable();
    unknown_citations.dedup();
    if !unknown_citations.is_empty() {
        blockers.push("MODEL_CITED_UNKNOWN_EVIDENCE".to_string());
    }

    let min_confidence = policy.min_model_confidence_for_band_change;
    let min_multiplier = policy.min_band_multiplier;
    let max_multiplier = policy.max_band_multiplier;
    let multiplier = if assessment.confidence >= min_confidence {
        clamp_like_python(assessment.band_multiplier, min_multiplier, max_multiplier)
    } else {
        warnings.push("model_confidence_too_low_for_band_change".to_string());
        1.0
    };

    let target = core.hedge.target_stock_shares;
    let current = core.inputs.current_shares;
    let raw_trade = target - current;
    let base_band = core.inputs.base_no_trade_band_shares;
    if ![target, current, raw_trade, base_band].iter().all(|v| v.is_finite()) {
        blockers.push("NONFINITE_CORE_VALUE".to_string());
    }

    let effective_band = (base_band * multiplier).max(0.0);
    let inside_band = raw_trade.abs() <= effective_band;
    let mut preview_trade = if inside_band { 0.0 } else { raw_trade };

    if !policy.allow_fractional_shares {
        preview_trade = round_half_to_even(preview_trade);
    }

    let max_shares = policy.max_abs_trade_shares;
    if preview_trade.abs() > max_shares {
        blockers.push("TRADE_SHARE_LIMIT".to_string());
    }

    let mut notional = preview_trade.abs() * midpoint;
    let max_notional = policy.max_preview_notional;
    if notional > max_notional {
        blockers.push("PREVIEW_NOTIONAL_LIMIT".to_string());
    }

    let require_open = policy.require_market_open_for_preview;
    if require_open && snapshot.quote.market_state != "open" && !inside_band {
        blockers.push("MARKET_NOT_OPEN".to_string());
    }

    let approved = blockers.is_empty();
    let action = if !blockers.is_empty() {
        preview_trade = 0.0;
        notional = 0.0;
        "observe_blocked"
    } else if inside_band {
        "hold_inside_effective_band"
    } else {
        "paper_rebalance_preview"
    };

    PolicyDecision {
        action: action.to_string(),
        paper_preview_approved: approved,
        live_execution_allowed: false,
        effective_no_trade_band_shares: effective_band,
        target_stock_shares: target,
        current_stock_shares: current,
        raw_trade_shares: raw_trade,
        paper_trade_preview_shares: preview_trade,
        paper_trade_preview_notional: notional,
        blocking_reasons: blockers,
        warnings,
        applied_limits: AppliedLimits {
            policy_version: POLICY_VERSION.to_string(),
            paper_only: policy.paper_only,
            quote_age_seconds: quote_age,
            max_quote_age_seconds: max_age,
            spread_bps: spread,
            max_spread_bps: max_spread,
            data_quality: features.data_quality,
            min_data_quality: min_quality,
            model_confidence: assessment.confidence,
            band_multiplier_applied: multiplier,
            max_abs_trade_shares: max_shares,
            max_preview_notional: max_notional,
        },
    }
}
