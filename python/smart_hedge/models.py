from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from typing import Any


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


@dataclass(slots=True)
class Quote:
    symbol: str
    bid: float
    ask: float
    last: float
    timestamp: str
    source: str
    market_state: str = "unknown"

    @property
    def midpoint(self) -> float:
        if self.bid > 0 and self.ask >= self.bid:
            return 0.5 * (self.bid + self.ask)
        return self.last

    @property
    def spread_bps(self) -> float:
        mid = self.midpoint
        if mid <= 0 or self.ask < self.bid:
            return float("inf")
        return (self.ask - self.bid) / mid * 10_000.0


@dataclass(slots=True)
class Bar:
    timestamp: str
    open: float
    high: float
    low: float
    close: float
    volume: float


@dataclass(slots=True)
class EvidenceItem:
    evidence_id: str
    kind: str
    title: str
    timestamp: str
    source: str
    value: float | str | bool | None = None
    text: str = ""
    quality: float = 0.5
    untrusted_text: bool = True


@dataclass(slots=True)
class MarketSnapshot:
    symbol: str
    quote: Quote
    bars: list[Bar]
    evidence: list[EvidenceItem]
    received_at: str = field(default_factory=utc_now_iso)


@dataclass(slots=True)
class FeatureSet:
    values: dict[str, float | str | bool | None]
    missing: list[str]
    warnings: list[str]
    data_quality: float
    evidence_ids: list[str]


@dataclass(slots=True)
class ModelAssessment:
    advisor_kind: str
    model: str
    regime: str
    confidence: float
    hedge_urgency: float
    band_multiplier: float
    summary: str
    evidence_ids: list[str]
    risks: list[str]
    scenario_spot_shocks: list[float]
    data_requests: list[str]
    raw_response_id: str = ""
    fallback_reason: str = ""


@dataclass(slots=True)
class PolicyDecision:
    action: str
    paper_preview_approved: bool
    live_execution_allowed: bool
    effective_no_trade_band_shares: float
    target_stock_shares: float
    current_stock_shares: float
    raw_trade_shares: float
    paper_trade_preview_shares: float
    paper_trade_preview_notional: float
    blocking_reasons: list[str]
    warnings: list[str]
    applied_limits: dict[str, float | int | bool | str]


@dataclass(slots=True)
class Recommendation:
    decision_id: str
    created_at: str
    mode: str
    symbol: str
    contract: dict[str, Any]
    snapshot: MarketSnapshot
    features: FeatureSet
    deterministic_core: dict[str, Any]
    model_assessment: ModelAssessment
    policy: PolicyDecision
    audit: dict[str, Any]


def to_dict(value: Any) -> Any:
    if hasattr(value, "__dataclass_fields__"):
        return asdict(value)
    return value
