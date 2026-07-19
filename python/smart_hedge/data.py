from __future__ import annotations

import json
import math
import os
import random
import statistics
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Protocol
from zoneinfo import ZoneInfo

from .config import resolve_project_path
from .models import Bar, EvidenceItem, MarketSnapshot, Quote, utc_now_iso


class MarketDataProvider(Protocol):
    def snapshot(self, symbol: str) -> MarketSnapshot: ...


def _iso(value: str | None) -> str:
    if not value:
        return utc_now_iso()
    return value.replace("+00:00", "Z")


def _regular_market_state(now: datetime | None = None) -> str:
    # Deliberately conservative: this catches weekends and clock hours but does
    # not pretend to be a complete exchange-holiday calendar.
    eastern = (now or datetime.now(timezone.utc)).astimezone(ZoneInfo("America/New_York"))
    if eastern.weekday() >= 5:
        return "closed"
    minutes = eastern.hour * 60 + eastern.minute
    return "open" if 570 <= minutes < 960 else "closed"


class SyntheticProvider:
    """Deterministic-enough changing data for a zero-cost local demonstration."""

    def __init__(self, config: dict[str, Any]):
        self.config = config

    def snapshot(self, symbol: str) -> MarketSnapshot:
        now = datetime.now(timezone.utc)
        bucket = int(now.timestamp() // 5)
        seed = sum((i + 1) * ord(ch) for i, ch in enumerate(symbol.upper())) + bucket
        rng = random.Random(seed)

        base = float(self.config.get("contracts", {}).get(symbol, {}).get("strike", 100.0))
        anchor = base * (1.0 + 0.03 * math.sin(bucket / 240.0))
        sigma_per_bar = 0.20 / math.sqrt(252.0 * 390.0)
        closes = [anchor]
        for _ in range(179):
            jump = rng.gauss(0.0, sigma_per_bar)
            if rng.random() < 0.01:
                jump += rng.choice([-1.0, 1.0]) * rng.uniform(0.002, 0.008)
            closes.append(max(0.01, closes[-1] * math.exp(jump)))

        bars: list[Bar] = []
        start = now - timedelta(minutes=len(closes) - 1)
        for i, close in enumerate(closes):
            previous = closes[i - 1] if i else close
            high = max(previous, close) * (1.0 + rng.uniform(0.0, 0.0007))
            low = min(previous, close) * (1.0 - rng.uniform(0.0, 0.0007))
            bars.append(
                Bar(
                    timestamp=(start + timedelta(minutes=i)).isoformat().replace("+00:00", "Z"),
                    open=previous,
                    high=high,
                    low=low,
                    close=close,
                    volume=max(100.0, rng.lognormvariate(9.0, 0.55)),
                )
            )

        last = closes[-1]
        spread_bps = rng.uniform(0.5, 4.0)
        half_spread = last * spread_bps / 20_000.0
        quote = Quote(
            symbol=symbol.upper(),
            bid=last - half_spread,
            ask=last + half_spread,
            last=last,
            timestamp=utc_now_iso(),
            source="synthetic",
            market_state="open",  # synthetic market is intentionally always available
        )
        realized = statistics.pstdev(
            [math.log(closes[i] / closes[i - 1]) for i in range(1, len(closes))]
        ) * math.sqrt(252.0 * 390.0)
        evidence = [
            EvidenceItem(
                evidence_id=f"synthetic-rv-{bucket}",
                kind="option_metric",
                title="Synthetic realized volatility",
                timestamp=utc_now_iso(),
                source="synthetic",
                value=realized,
                text="Generated solely for exercising the pipeline.",
                quality=1.0,
                untrusted_text=False,
            ),
            EvidenceItem(
                evidence_id=f"synthetic-event-{bucket}",
                kind="event",
                title="Synthetic event-risk flag",
                timestamp=utc_now_iso(),
                source="synthetic",
                value=bool(bucket % 29 == 0),
                text="No real-world event is represented.",
                quality=1.0,
                untrusted_text=False,
            ),
        ]
        evidence.extend(load_evidence_file(self.config, symbol))
        return MarketSnapshot(symbol=symbol.upper(), quote=quote, bars=bars, evidence=evidence)


class AlpacaReadOnlyProvider:
    """Read-only U.S. equity quote/bar adapter. It contains no order URL or method."""

    def __init__(self, config: dict[str, Any]):
        self.root_config = config
        self.config = config["provider"].get("alpaca", {})
        self.api_key = os.getenv("ALPACA_API_KEY_ID", "")
        self.api_secret = os.getenv("ALPACA_API_SECRET_KEY", "")
        if not self.api_key or not self.api_secret:
            raise RuntimeError(
                "Alpaca read-only provider requires ALPACA_API_KEY_ID and "
                "ALPACA_API_SECRET_KEY"
            )
        self.base = str(self.config.get("data_base_url", "https://data.alpaca.markets")).rstrip("/")
        self.feed = str(self.config.get("feed", "iex"))
        self.timeout = float(self.config.get("timeout_seconds", 8.0))

    def _get(self, path: str, query: dict[str, Any]) -> dict[str, Any]:
        url = f"{self.base}{path}?{urllib.parse.urlencode(query)}"
        request = urllib.request.Request(
            url,
            headers={
                "APCA-API-KEY-ID": self.api_key,
                "APCA-API-SECRET-KEY": self.api_secret,
                "Accept": "application/json",
                "User-Agent": "smart-dynamic-hedge/0.2 read-only",
            },
            method="GET",
        )
        with urllib.request.urlopen(request, timeout=self.timeout) as response:
            body = response.read(2_000_000)
        decoded = json.loads(body)
        if not isinstance(decoded, dict):
            raise RuntimeError("unexpected market-data response")
        return decoded

    def snapshot(self, symbol: str) -> MarketSnapshot:
        normalized = symbol.upper()
        bar_limit = int(self.config.get("bar_limit", 180))
        quote_payload = self._get(
            f"/v2/stocks/{urllib.parse.quote(normalized)}/quotes/latest",
            {"feed": self.feed},
        )
        start = (datetime.now(timezone.utc) - timedelta(days=7)).isoformat().replace("+00:00", "Z")
        bars_payload = self._get(
            f"/v2/stocks/{urllib.parse.quote(normalized)}/bars",
            {
                "feed": self.feed,
                "timeframe": self.config.get("bar_timeframe", "1Min"),
                "limit": bar_limit,
                "adjustment": "all",
                "start": start,
                "sort": "desc",
            },
        )
        q = quote_payload.get("quote") or {}
        # The request asks for descending data to obtain the most recent bars;
        # normalize it back to chronological order before feature calculation.
        raw_bars = list(reversed(bars_payload.get("bars") or []))
        bars = [
            Bar(
                timestamp=_iso(item.get("t")),
                open=float(item["o"]),
                high=float(item["h"]),
                low=float(item["l"]),
                close=float(item["c"]),
                volume=float(item.get("v", 0.0)),
            )
            for item in raw_bars
            if all(key in item for key in ("o", "h", "l", "c"))
        ]
        if not bars:
            raise RuntimeError("market-data provider returned no bars")
        bid = float(q.get("bp") or bars[-1].close)
        ask = float(q.get("ap") or bars[-1].close)
        last = bars[-1].close
        quote = Quote(
            symbol=normalized,
            bid=bid,
            ask=ask,
            last=last,
            timestamp=_iso(q.get("t") or bars[-1].timestamp),
            source=f"alpaca:{self.feed}",
            market_state=_regular_market_state(),
        )
        evidence = load_evidence_file(self.root_config, normalized)
        evidence.extend(load_fred_evidence(self.root_config))
        evidence.extend(load_rss_evidence(self.root_config, normalized))
        return MarketSnapshot(symbol=normalized, quote=quote, bars=bars, evidence=evidence)


def load_evidence_file(config: dict[str, Any], symbol: str) -> list[EvidenceItem]:
    raw_path = str(config.get("provider", {}).get("evidence_file", "")).strip()
    if not raw_path:
        return []
    path = resolve_project_path(config, raw_path)
    if not path.exists():
        return []
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    rows = payload.get("evidence", payload if isinstance(payload, list) else [])
    if not isinstance(rows, list):
        return []
    output: list[EvidenceItem] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            continue
        applies = row.get("symbols", [symbol])
        if applies and symbol.upper() not in [str(x).upper() for x in applies] and "*" not in applies:
            continue
        output.append(
            EvidenceItem(
                evidence_id=str(row.get("evidence_id") or f"file-{index}"),
                kind=str(row.get("kind", "external")),
                title=str(row.get("title", "Untitled evidence"))[:240],
                timestamp=str(row.get("timestamp") or utc_now_iso()),
                source=str(row.get("source", f"file:{path.name}"))[:120],
                value=row.get("value"),
                text=str(row.get("text", ""))[:5000],
                quality=max(0.0, min(1.0, float(row.get("quality", 0.5)))),
                untrusted_text=bool(row.get("untrusted_text", True)),
            )
        )
    return output


def load_fred_evidence(config: dict[str, Any]) -> list[EvidenceItem]:
    fred = config.get("provider", {}).get("fred", {})
    if not fred.get("enabled"):
        return []
    api_key = os.getenv("FRED_API_KEY", "")
    if not api_key:
        return [
            EvidenceItem(
                evidence_id="fred-missing-key",
                kind="data_quality",
                title="FRED connector disabled at runtime",
                timestamp=utc_now_iso(),
                source="fred",
                text="FRED_API_KEY is not set.",
                quality=1.0,
                untrusted_text=False,
            )
        ]
    timeout = float(fred.get("timeout_seconds", 8.0))
    output: list[EvidenceItem] = []
    for series_id in list(fred.get("series", []))[:20]:
        query = urllib.parse.urlencode(
            {
                "series_id": series_id,
                "api_key": api_key,
                "file_type": "json",
                "sort_order": "desc",
                "limit": 1,
            }
        )
        request = urllib.request.Request(
            f"https://api.stlouisfed.org/fred/series/observations?{query}",
            headers={"User-Agent": "smart-dynamic-hedge/0.2"},
        )
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                payload = json.loads(response.read(1_000_000))
            observation = (payload.get("observations") or [{}])[0]
            raw_value = observation.get("value")
            numeric = float(raw_value) if raw_value not in (None, ".") else None
            output.append(
                EvidenceItem(
                    evidence_id=f"fred-{series_id}-{observation.get('date', 'latest')}",
                    kind="macro",
                    title=f"FRED {series_id}",
                    timestamp=str(observation.get("date") or utc_now_iso()),
                    source="FRED",
                    value=numeric,
                    quality=0.9,
                    untrusted_text=False,
                )
            )
        except Exception as exc:  # connector errors are evidence, not process failures
            output.append(
                EvidenceItem(
                    evidence_id=f"fred-error-{series_id}",
                    kind="data_quality",
                    title=f"FRED {series_id} retrieval error",
                    timestamp=utc_now_iso(),
                    source="FRED",
                    text=type(exc).__name__,
                    quality=1.0,
                    untrusted_text=False,
                )
            )
    return output


def load_rss_evidence(config: dict[str, Any], symbol: str) -> list[EvidenceItem]:
    rss = config.get("provider", {}).get("rss", {})
    if not rss.get("enabled"):
        return []
    output: list[EvidenceItem] = []
    max_items = max(0, min(20, int(rss.get("max_items_per_feed", 3))))
    for feed_index, url in enumerate(list(rss.get("feeds", []))[:10]):
        try:
            request = urllib.request.Request(
                str(url), headers={"User-Agent": "smart-dynamic-hedge/0.2 research reader"}
            )
            with urllib.request.urlopen(request, timeout=8.0) as response:
                raw = response.read(2_000_000)
            root = ET.fromstring(raw)
            entries = root.findall(".//item") or root.findall(".//{*}entry")
            for item_index, item in enumerate(entries[:max_items]):
                title = (item.findtext("title") or item.findtext("{*}title") or "RSS item").strip()
                description = (
                    item.findtext("description")
                    or item.findtext("summary")
                    or item.findtext("{*}summary")
                    or ""
                )
                published = (
                    item.findtext("pubDate")
                    or item.findtext("published")
                    or item.findtext("{*}updated")
                    or utc_now_iso()
                )
                output.append(
                    EvidenceItem(
                        evidence_id=f"rss-{feed_index}-{item_index}-{abs(hash(title))}",
                        kind="news",
                        title=f"{symbol}: {title}"[:240],
                        timestamp=published,
                        source=f"rss:{urllib.parse.urlparse(str(url)).netloc}",
                        text=description[:5000],
                        quality=0.45,
                        untrusted_text=True,
                    )
                )
        except Exception as exc:
            output.append(
                EvidenceItem(
                    evidence_id=f"rss-error-{feed_index}",
                    kind="data_quality",
                    title="RSS retrieval error",
                    timestamp=utc_now_iso(),
                    source=f"rss:{urllib.parse.urlparse(str(url)).netloc}",
                    text=type(exc).__name__,
                    quality=1.0,
                    untrusted_text=False,
                )
            )
    return output


def build_provider(config: dict[str, Any]) -> MarketDataProvider:
    kind = str(config.get("provider", {}).get("kind", "synthetic")).lower()
    if kind == "synthetic":
        return SyntheticProvider(config)
    if kind in {"alpaca", "alpaca-readonly", "alpaca_readonly"}:
        return AlpacaReadOnlyProvider(config)
    raise ValueError(f"unknown provider kind: {kind}")
