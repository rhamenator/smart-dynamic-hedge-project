from __future__ import annotations

import math
from datetime import datetime, timezone
from typing import Any

from .models import FeatureSet, MarketSnapshot, ModelAssessment, PolicyDecision

POLICY_VERSION = "paper-guard-v1"


def _parse_time(value: str) -> datetime | None:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=timezone.utc)
        return parsed.astimezone(timezone.utc)
    except (TypeError, ValueError):
        return None


def evaluate_policy(
    config: dict[str, Any],
    snapshot: MarketSnapshot,
    features: FeatureSet,
    core: dict[str, Any],
    assessment: ModelAssessment,
) -> PolicyDecision:
    policy = config.get("policy", {})
    blockers: list[str] = []
    warnings = list(features.warnings)

    paper_only = bool(policy.get("paper_only", True))
    if not paper_only or str(config.get("mode", "paper")).lower() != "paper":
        blockers.append("LIVE_MODE_FORBIDDEN")

    midpoint = snapshot.quote.midpoint
    if not math.isfinite(midpoint) or midpoint <= 0:
        blockers.append("INVALID_QUOTE")

    quote_time = _parse_time(snapshot.quote.timestamp)
    max_age = float(policy.get("max_quote_age_seconds", 45.0))
    quote_age = float("inf")
    if quote_time:
        quote_age = max(0.0, (datetime.now(timezone.utc) - quote_time).total_seconds())
    if quote_age > max_age:
        blockers.append("STALE_QUOTE")

    spread = snapshot.quote.spread_bps
    max_spread = float(policy.get("max_spread_bps", 35.0))
    if not math.isfinite(spread) or spread > max_spread:
        blockers.append("SPREAD_TOO_WIDE")

    min_quality = float(policy.get("min_data_quality", 0.65))
    if features.data_quality < min_quality:
        blockers.append("DATA_QUALITY_TOO_LOW")

    if features.missing:
        warnings.append("missing_features:" + ",".join(features.missing))

    allowed_evidence = set(features.evidence_ids)
    unknown_citations = sorted(set(assessment.evidence_ids) - allowed_evidence)
    if unknown_citations:
        blockers.append("MODEL_CITED_UNKNOWN_EVIDENCE")

    min_confidence = float(policy.get("min_model_confidence_for_band_change", 0.55))
    min_multiplier = float(policy.get("min_band_multiplier", 0.50))
    max_multiplier = float(policy.get("max_band_multiplier", 3.00))
    if assessment.confidence >= min_confidence:
        multiplier = min(max(assessment.band_multiplier, min_multiplier), max_multiplier)
    else:
        multiplier = 1.0
        warnings.append("model_confidence_too_low_for_band_change")

    hedge = core.get("hedge", {})
    inputs = core.get("inputs", {})
    try:
        target = float(hedge["target_stock_shares"])
        current = float(inputs["current_shares"])
        raw_trade = target - current
        base_band = float(inputs.get("base_no_trade_band_shares", 0.0))
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError("deterministic core response is malformed") from exc
    if not all(math.isfinite(value) for value in (target, current, raw_trade, base_band)):
        blockers.append("NONFINITE_CORE_VALUE")

    effective_band = max(0.0, base_band * multiplier)
    inside_band = abs(raw_trade) <= effective_band
    preview_trade = 0.0 if inside_band else raw_trade

    if not bool(policy.get("allow_fractional_shares", True)):
        preview_trade = float(round(preview_trade))

    max_shares = float(policy.get("max_abs_trade_shares", 500.0))
    if abs(preview_trade) > max_shares:
        blockers.append("TRADE_SHARE_LIMIT")

    notional = abs(preview_trade) * midpoint
    max_notional = float(policy.get("max_preview_notional", 50_000.0))
    if notional > max_notional:
        blockers.append("PREVIEW_NOTIONAL_LIMIT")

    require_open = bool(policy.get("require_market_open_for_preview", True))
    if require_open and snapshot.quote.market_state != "open" and not inside_band:
        blockers.append("MARKET_NOT_OPEN")

    approved = not blockers
    if blockers:
        action = "observe_blocked"
        preview_trade = 0.0
        notional = 0.0
    elif inside_band:
        action = "hold_inside_effective_band"
    else:
        action = "paper_rebalance_preview"

    return PolicyDecision(
        action=action,
        paper_preview_approved=approved,
        live_execution_allowed=False,
        effective_no_trade_band_shares=effective_band,
        target_stock_shares=target,
        current_stock_shares=current,
        raw_trade_shares=raw_trade,
        paper_trade_preview_shares=preview_trade,
        paper_trade_preview_notional=notional,
        blocking_reasons=blockers,
        warnings=warnings,
        applied_limits={
            "policy_version": POLICY_VERSION,
            "paper_only": paper_only,
            "quote_age_seconds": quote_age,
            "max_quote_age_seconds": max_age,
            "spread_bps": spread,
            "max_spread_bps": max_spread,
            "data_quality": features.data_quality,
            "min_data_quality": min_quality,
            "model_confidence": assessment.confidence,
            "band_multiplier_applied": multiplier,
            "max_abs_trade_shares": max_shares,
            "max_preview_notional": max_notional,
        },
    )
