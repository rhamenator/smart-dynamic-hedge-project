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

Live execution is not implemented in this repository, and by design it
never will be: this repository stays paper-only. Live trading is a real,
intended goal of the overall system — it is built and armed exclusively
through the separate `trade-guard-mcp` repository (see below), so that
broker credentials and order execution never share a codebase or process
with this repository's model-facing surface.

## V2 multi-repository expansion

A 2026-07-19 prompt bundle (`smart-dynamic-hedge-v2-prompt-bundle`) expands
this project from a U.S.-centric research scaffold into an international,
multi-asset, public-market-intelligence and guarded-execution platform,
split across three repositories with separate security boundaries:

* `smart-dynamic-hedge` (this repo) — strategy, research, GUI, autonomy.
* [`market-system-contracts`](https://github.com/rhamenator/market-system-contracts) —
  shared JSON Schema contracts. **Status: Phase 1 scaffold** — hand-written
  schemas and golden fixtures validate; no generated language bindings or
  canonical-hashing spec yet.
* [`market-intelligence-mcp`](https://github.com/rhamenator/market-intelligence-mcp) —
  public/licensed intelligence collection. **Status: Phase 2 fixture-only
  vertical slice** — deterministic source-policy/MNPI gate and domain
  types are implemented and tested against fixtures; no real external
  provider, MCP transport, or durable storage backend exists yet.
* [`trade-guard-mcp`](https://github.com/rhamenator/trade-guard-mcp) —
  account/risk/execution guard. **Status: not-started** — repository and
  module skeleton only; no risk engine, execution protocol, or broker
  adapter exists.

None of this repository's own code changed as part of that expansion yet.
The next milestone for this repository is Phase 4 in
`06-implementation-order-and-acceptance.md` of that bundle: add typed
clients for the two sibling services, international instrument/venue
schemas, and the `MODEL_URI` router — only after `trade-guard-mcp` has at
least a paper-only vertical slice (its own Phase 3) to integrate against.

## Language and dependency policy (decided 2026-07-19)

The user asked for all four repositories to move toward the fastest
language practicable, with third-party dependencies minimized toward zero,
and for every repository to carry SQLite-developer-grade testing. Decision
for this repository specifically:

* **The C++ deterministic core (`cpp/smart_dynamic_hedge.cpp`) stays as
  C++.** It is already in the fastest-practicable tier, already
  zero-dependency, and already tested against known Black-Scholes
  references. Rewriting correct, tested financial math into Rust purely
  for stylistic/toolchain consistency would introduce risk (a subtle bug
  in option pricing is a much worse outcome than an extra language in the
  stack) without a speed or safety benefit — Rust and C++ are both
  "fastest practicable" for this workload.
* **The Python orchestration layer
  (`python/smart_hedge/{cli,config,core_bridge,dashboard,data,engine,
  features,mcp_server,model_advisor,models,policy,store}.py`) is a
  migration candidate to Rust.** Python is not in the fastest-practicable
  tier (interpreter overhead, GIL), and `market-intelligence-mcp` /
  `trade-guard-mcp` are already Rust — migrating this layer would let the
  whole system share one toolchain, one dependency-minimization policy, and
  one testing philosophy (see `market-intelligence-mcp`'s
  `market_intelligence_core::utc_timestamp` for the "hand-roll instead of
  depending on `time`/`uuid`/`thiserror`" pattern this repo should copy).
  This is **not started** — it is a substantial rewrite of working, tested
  code and should happen incrementally, module by module, with the
  existing Python test suite kept green throughout rather than as a
  big-bang rewrite. No target date is set; it is not blocking the V2
  multi-repository work above.
* Until the migration happens, keep the existing Python/C++ code's test
  suite (`tests/test_engine.py`, `tests/test_model_schema.py`,
  `tests/test_policy.py`) as the baseline and raise its rigor
  incrementally (property/boundary tests, no-panic guarantees on malformed
  model output) rather than leaving it while effort goes entirely to new
  repositories.
* This repository's `.venv`/`build/` artifacts and the `market-system-contracts`
  schema-validation dev dependencies (`jsonschema`, `referencing`) remain
  the only "many dependencies" surface in the system by design — Python
  tooling dependencies are cheaper to accept than compiled-binary
  dependencies, since this repo is not itself a compiled artifact shipped
  to end users the way the Rust services are.
