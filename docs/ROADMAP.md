# Research roadmap

## Phase 1 — current scaffold

* deterministic C++ American/European core;
* synthetic and read-only equity quote/bar adapters;
* feature/provenance record;
* constrained adviser and policy gate;
* SQLite replay, dashboard, and local MCP tools.

## Phase 2 — data correctness

* exchange-grade U.S. trading calendar;
* point-in-time corporate actions and dividends;
* listed-option contract master and option-chain snapshots;
* implied-volatility surface and Greeks from observed option prices;
* data-quality reconciliation across at least two quote sources;
* publication timestamps for macro, filings, and news.

## Phase 3 — experiment framework

* immutable event store;
* walk-forward and purged cross-validation;
* transaction-cost and latency model;
* paper fill simulator with explicit account reset;
* benchmark advisers: no model, heuristic, small statistical model, LLM;
* pre-registered metrics for hedge error, turnover, tail loss, and cost.

## Phase 4 — portfolio risk

* aggregate delta, gamma, vega, theta, and scenario P&L;
* multiple expiries and underlyings;
* factor/correlation stress testing;
* assignment and exercise scenarios;
* capital, margin, and liquidity constraints.

Live execution is deliberately not a roadmap item for this repository.
