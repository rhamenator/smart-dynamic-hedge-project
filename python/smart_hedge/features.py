from __future__ import annotations

import math
import statistics
from typing import Any

from .models import FeatureSet, MarketSnapshot


def _safe_mean(values: list[float]) -> float | None:
    return statistics.fmean(values) if values else None


def _safe_stdev(values: list[float]) -> float | None:
    return statistics.stdev(values) if len(values) >= 2 else None


def _ewma_variance(returns: list[float], decay: float) -> float | None:
    if not returns:
        return None
    variance = returns[0] * returns[0]
    for value in returns[1:]:
        variance = decay * variance + (1.0 - decay) * value * value
    return variance


def build_features(snapshot: MarketSnapshot, config: dict[str, Any]) -> FeatureSet:
    feature_cfg = config.get("features", {})
    bars_per_year = float(feature_cfg.get("bars_per_year", 252.0 * 390.0))
    decay = float(feature_cfg.get("ewma_lambda", 0.94))
    short_window = int(feature_cfg.get("short_window", 20))
    long_window = int(feature_cfg.get("long_window", 90))

    closes = [bar.close for bar in snapshot.bars if bar.close > 0]
    volumes = [max(0.0, bar.volume) for bar in snapshot.bars]
    returns = [math.log(closes[i] / closes[i - 1]) for i in range(1, len(closes))]
    missing: list[str] = []
    warnings: list[str] = []

    realized = None
    if len(returns) >= 2:
        realized = statistics.stdev(returns) * math.sqrt(bars_per_year)
    else:
        missing.append("realized_volatility")

    ewma_var = _ewma_variance(returns, decay)
    ewma_vol = math.sqrt(ewma_var * bars_per_year) if ewma_var is not None else None
    if ewma_vol is None:
        missing.append("ewma_volatility")

    def horizon_return(window: int) -> float | None:
        if len(closes) <= window:
            return None
        return closes[-1] / closes[-1 - window] - 1.0

    short_return = horizon_return(short_window)
    long_return = horizon_return(long_window)
    if short_return is None:
        missing.append(f"return_{short_window}_bars")
    if long_return is None:
        missing.append(f"return_{long_window}_bars")

    rolling_peak = max(closes[-long_window:]) if closes else None
    drawdown = closes[-1] / rolling_peak - 1.0 if closes and rolling_peak else None

    volume_z = None
    if len(volumes) >= 21:
        history = volumes[-21:-1]
        sd = _safe_stdev(history)
        if sd and sd > 0:
            volume_z = (volumes[-1] - statistics.fmean(history)) / sd
    if volume_z is None:
        warnings.append("volume_zscore_unavailable")

    trend_score = None
    if short_return is not None and realized and realized > 1e-9:
        horizon_years = short_window / bars_per_year
        trend_score = short_return / (realized * math.sqrt(horizon_years))

    evidence_numeric: dict[str, float] = {}
    event_risk = False
    evidence_quality: list[float] = []
    for item in snapshot.evidence:
        evidence_quality.append(item.quality)
        if item.kind == "event" and item.value is True:
            event_risk = True
        if isinstance(item.value, (int, float)) and not isinstance(item.value, bool):
            key = "evidence_" + "".join(ch if ch.isalnum() else "_" for ch in item.title.lower())[:64]
            evidence_numeric[key] = float(item.value)

    values: dict[str, float | str | bool | None] = {
        "spot": snapshot.quote.midpoint,
        "bid": snapshot.quote.bid,
        "ask": snapshot.quote.ask,
        "spread_bps": snapshot.quote.spread_bps,
        "market_state": snapshot.quote.market_state,
        "bar_count": float(len(snapshot.bars)),
        "realized_volatility": realized,
        "ewma_volatility": ewma_vol,
        f"return_{short_window}_bars": short_return,
        f"return_{long_window}_bars": long_return,
        "drawdown_from_rolling_peak": drawdown,
        "volume_zscore": volume_z,
        "trend_score": trend_score,
        "event_risk_flag": event_risk,
    }
    values.update(evidence_numeric)

    quality_components = [
        1.0 if snapshot.quote.midpoint > 0 else 0.0,
        1.0 if math.isfinite(snapshot.quote.spread_bps) else 0.0,
        min(1.0, len(snapshot.bars) / max(1.0, float(long_window + 1))),
        1.0 - min(1.0, len(missing) / 6.0),
    ]
    if evidence_quality:
        quality_components.append(statistics.fmean(evidence_quality))
    data_quality = max(0.0, min(1.0, statistics.fmean(quality_components)))

    return FeatureSet(
        values=values,
        missing=missing,
        warnings=warnings,
        data_quality=data_quality,
        evidence_ids=[item.evidence_id for item in snapshot.evidence],
    )
