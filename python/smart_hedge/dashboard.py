from __future__ import annotations

import asyncio
import json
import threading
import time
from typing import Any

from .config import load_config
from .engine import SmartHedgeEngine


_HTML = r"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Smart Dynamic Hedge — Paper Debugger</title>
  <style>
    :root { color-scheme: dark; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    body { margin: 0; background: #11151a; color: #e9eef5; }
    header { padding: 18px 24px; border-bottom: 1px solid #37414d; background: #171d24; position: sticky; top: 0; }
    .warning { background: #5b1b1b; border: 1px solid #c55252; padding: 10px 14px; font-weight: 700; margin-top: 12px; }
    main { max-width: 1380px; margin: 0 auto; padding: 22px; }
    .controls { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 18px; }
    input, button { font: inherit; padding: 9px 12px; border-radius: 4px; border: 1px solid #526171; background: #1a222b; color: #e9eef5; }
    button { cursor: pointer; }
    button:hover { background: #273440; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 14px; }
    .card { background: #171d24; border: 1px solid #34404c; border-radius: 6px; padding: 14px; min-height: 95px; }
    .label { color: #9eb0c2; font-size: 12px; text-transform: uppercase; letter-spacing: .08em; }
    .value { font-size: 22px; margin-top: 8px; overflow-wrap: anywhere; }
    .small { font-size: 13px; line-height: 1.45; }
    .approved { border-color: #388c5b; }
    .blocked { border-color: #b34d4d; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; background: #0c1014; border: 1px solid #2e3944; padding: 14px; border-radius: 6px; max-height: 720px; overflow: auto; }
    details { margin-top: 16px; }
    a { color: #9cc6ff; }
  </style>
</head>
<body>
<header>
  <div><strong>Smart Dynamic Hedge</strong> — audit/debug console</div>
  <div class="warning">PAPER / OBSERVE ONLY. This service has no broker-order endpoint and cannot place a trade.</div>
</header>
<main>
  <div class="controls">
    <label for="symbol">Configured symbol</label>
    <input id="symbol" value="SPY" maxlength="12" autocomplete="off">
    <button id="refresh">Generate recommendation</button>
    <label><input id="auto" type="checkbox"> refresh every 15 seconds</label>
    <span id="status" class="small"></span>
  </div>
  <div id="cards" class="grid"></div>
  <details open>
    <summary>Model explanation and policy diagnostics</summary>
    <pre id="explanation">No decision generated yet.</pre>
  </details>
  <details>
    <summary>Complete replayable decision record</summary>
    <pre id="raw"></pre>
  </details>
</main>
<script>
const cards = document.getElementById('cards');
const raw = document.getElementById('raw');
const explanation = document.getElementById('explanation');
const statusNode = document.getElementById('status');
let timer = null;
const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}[c]));
const num = (v, d=4) => Number.isFinite(Number(v)) ? Number(v).toFixed(d) : 'n/a';
function card(label, value, cls='') { return `<section class="card ${cls}"><div class="label">${esc(label)}</div><div class="value">${esc(value)}</div></section>`; }
async function refresh() {
  const symbol = document.getElementById('symbol').value.trim().toUpperCase();
  statusNode.textContent = 'working…';
  try {
    const response = await fetch(`/api/recommendation?symbol=${encodeURIComponent(symbol)}&fresh=true`);
    const data = await response.json();
    if (!response.ok) throw new Error(data.detail || JSON.stringify(data));
    const q = data.snapshot.quote;
    const g = data.deterministic_core.greeks;
    const p = data.policy;
    const m = data.model_assessment;
    cards.innerHTML = [
      card('Decision', p.action, p.blocking_reasons.length ? 'blocked' : 'approved'),
      card('Underlying midpoint', num((q.bid + q.ask) / 2, 4)),
      card('Spread', `${num(data.features.values.spread_bps, 2)} bps`),
      card('Option model value', num(data.deterministic_core.pricing.model_price, 4)),
      card('Delta / Gamma', `${num(g.delta, 5)} / ${num(g.gamma, 6)}`),
      card('Target stock shares', num(p.target_stock_shares, 3)),
      card('Paper trade preview', num(p.paper_trade_preview_shares, 3)),
      card('Effective no-trade band', `±${num(p.effective_no_trade_band_shares, 3)}`),
      card('Regime / confidence', `${m.regime} / ${num(m.confidence, 2)}`),
      card('Data quality', num(data.features.data_quality, 2)),
      card('Decision ID', data.decision_id),
      card('Live execution allowed', String(p.live_execution_allowed), 'blocked')
    ].join('');
    explanation.textContent = JSON.stringify({
      summary: m.summary,
      risks: m.risks,
      cited_evidence: m.evidence_ids,
      requested_data: m.data_requests,
      blockers: p.blocking_reasons,
      warnings: p.warnings,
      limits: p.applied_limits,
      audit: data.audit
    }, null, 2);
    raw.textContent = JSON.stringify(data, null, 2);
    statusNode.textContent = `updated ${new Date().toLocaleTimeString()}`;
  } catch (error) {
    statusNode.textContent = `error: ${error.message}`;
  }
}
document.getElementById('refresh').addEventListener('click', refresh);
document.getElementById('auto').addEventListener('change', event => {
  if (timer) clearInterval(timer);
  timer = event.target.checked ? setInterval(refresh, 15000) : null;
});
refresh();
</script>
</body>
</html>"""


class _Cache:
    def __init__(self, seconds: float):
        self.seconds = max(0.0, seconds)
        self.lock = threading.Lock()
        self.values: dict[str, tuple[float, dict[str, Any]]] = {}

    def get(self, symbol: str) -> dict[str, Any] | None:
        with self.lock:
            row = self.values.get(symbol)
            if row and time.monotonic() - row[0] <= self.seconds:
                return row[1]
            return None

    def put(self, symbol: str, value: dict[str, Any]) -> None:
        with self.lock:
            self.values[symbol] = (time.monotonic(), value)


def create_app(config_path: str | None = None):
    try:
        from fastapi import FastAPI, HTTPException, Query
        from fastapi.responses import HTMLResponse
    except ImportError as exc:
        raise RuntimeError("install dashboard dependencies: pip install -e '.[dashboard]'") from exc

    config = load_config(config_path)
    engine = SmartHedgeEngine(config)
    cache = _Cache(float(config.get("dashboard", {}).get("cache_seconds", 5.0)))
    app = FastAPI(
        title="Smart Dynamic Hedge Paper Debugger",
        version="0.2.0",
        description="Read-only market evidence and paper hedge recommendations. No order endpoint.",
    )

    @app.get("/", response_class=HTMLResponse)
    async def index() -> str:
        return _HTML

    @app.get("/api/health")
    async def health() -> dict[str, Any]:
        return engine.health()

    @app.get("/api/recommendation")
    async def recommendation(
        symbol: str = Query("SPY", min_length=1, max_length=12, pattern=r"^[A-Za-z0-9._-]+$"),
        fresh: bool = False,
    ) -> dict[str, Any]:
        normalized = symbol.upper()
        if not fresh:
            cached = cache.get(normalized)
            if cached:
                return cached
        try:
            value = await asyncio.to_thread(engine.recommendation, normalized)
        except Exception as exc:
            raise HTTPException(status_code=400, detail=f"{type(exc).__name__}: {str(exc)[:600]}") from exc
        cache.put(normalized, value)
        return value

    @app.get("/api/history")
    async def history(
        limit: int = Query(20, ge=1, le=200),
        symbol: str | None = Query(None, max_length=12, pattern=r"^[A-Za-z0-9._-]+$"),
    ) -> list[dict[str, Any]]:
        return await asyncio.to_thread(engine.recent, limit, symbol)

    @app.get("/api/replay/{decision_id}")
    async def replay(decision_id: str) -> dict[str, Any]:
        try:
            return await asyncio.to_thread(engine.replay, decision_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail=str(exc)) from exc

    return app
