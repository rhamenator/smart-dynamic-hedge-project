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
  account/risk/execution guard. **Status: paper-only vertical slice
  (2026-07-20)** — typed `TradeIntent`/`EvidenceBundle`/`InstrumentId`
  contracts, `check-evidence-eligibility`, the atomic
  `authorize-and-submit-paper-order` protocol (durable, idempotent) against
  an internal deterministic paper simulator, a hash-chained tamper-evident
  SQLite audit log, and a real MCP stdio server exposing 13 tools — 149
  tests, verified end to end against the compiled release binary. No
  live-execution path exists in source at all (not merely disabled); no
  real broker/venue/FIX adapter, market-abuse surveillance, remote
  transport, or operator-admin surface yet — see that repo's
  `docs/CAPABILITY_STATUS.md` for the exact scope cuts and why each is
  deliberate, not an oversight.

**Phase 4 — smallest complete slice done (2026-07-20).** Three new crates
(`smart-hedge-mcp-client`, `smart-hedge-intelligence-client`,
`smart-hedge-guard-client`) and a new `guard-demo` CLI subcommand
implement the core of Phase 4 in `06-implementation-order-and-acceptance.md`:
typed clients for both sibling services, and real
`TradeIntent -> trade-guard-mcp` paper-guard integration. Verified end to
end against all three repositories' independently built release
binaries — a real recommendation, a real `market-intelligence-mcp`
evidence bundle, and a real `trade-guard-mcp` paper fill, three separate
processes talking only over stdio. See `rust/README.md` "Connecting the
three repositories" for the full flow and exactly what is and isn't
proven.

**Portfolio-level Greeks — done (2026-07-20).** New `smart-hedge-portfolio`
crate and `portfolio` CLI subcommand: calls the unchanged C++ core once
per configured position and aggregates into dollar-denominated portfolio
Greeks (dollar delta, dollar gamma P&L, dollar vega/theta/rho, stock/
option notional) — additive across different underlyings, unlike raw
per-underlying share counts. Verified against a real two-position
(SPY put + QQQ call) config, two real C++ core invocations aggregated
correctly. See `rust/README.md`'s crate table.

Still not done, and explicitly out of scope so far: international
instrument/venue schemas, the `MODEL_URI` router (this repository still
selects its model adviser via `SMART_HEDGE_MODEL_KIND`/`SMART_HEDGE_PROVIDER`,
not a routed multi-model registry), whale/corporate/political/price/
options/FX/crypto signal integration beyond the one demo fixture,
evidence-graph/source-use UI, a point-in-time backtester, and a
paper-autonomous state machine (`guard-demo` is a one-shot manual
command, not an autonomous loop). Each remains a distinct, later
milestone, being worked through in sequence.

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
* **The Python orchestration layer that used to live in
  `python/smart_hedge/{cli,config,core_bridge,dashboard,data,engine,
  features,mcp_server,model_advisor,models,policy,store}.py` has been
  migrated to Rust, and the cutover is complete (decided and executed
  2026-07-19).** Python was not in the fastest-practicable tier
  (interpreter overhead, GIL), and `market-intelligence-mcp` /
  `trade-guard-mcp` are already Rust — migrating this layer let the whole
  system share one toolchain, one dependency-minimization policy, and one
  testing philosophy (see `market-intelligence-mcp`'s
  `market_intelligence_core::utc_timestamp` for the "hand-roll instead of
  depending on `time`/`uuid`/`thiserror`" pattern this repo followed).
  **Status: done.** Every module was ported in an isolated `rust/`
  workspace with zero changes to any Python or C++ file while it was being
  proven out (strangler-fig approach, per the user: prove each piece out
  fully isolated, then decide the cutover shape once more exists), and a
  DO-178-inspired requirements-traceability matrix
  (`requirements/TRACEABILITY.md`) was closed for the Rust side before
  cutover — see `rust/README.md` for exact crate/test-count status and
  documented behavioral differences from the original Python. The cutover
  itself replaced the Python package outright with a standalone Rust
  `smart-hedge` binary (matching `market-intelligence-mcp`'s shape), not a
  PyO3 embedding (which would have kept a Python runtime in production
  permanently). The former `python/` package and `tests/` suite were
  removed from the active tree as part of cutover; they remain available in
  git history. The C++ core (`cpp/smart_dynamic_hedge.cpp`) is unaffected —
  the Rust binary invokes it exactly as Python did.
* The former Python/C++ test suite (`tests/test_engine.py`,
  `tests/test_model_schema.py`, `tests/test_policy.py`) has been superseded
  by the Rust workspace's `cargo test --workspace` suite, which transcribes
  every case those files exercised plus substantial additional
  property/boundary/adversarial coverage — see `rust/README.md`. The C++
  core's own `ctest` suite is unchanged and still authoritative for the
  option-pricing math.
* With Python removed, the only third-party-dependency surface in this
  repository is the Rust workspace's own minimized set (`serde`/
  `serde_json`, `rusqlite` for SQLite, `ureq`/`rustls` for outbound HTTPS
  clients only — see `rust/README.md` "Dependency and testing policy") plus
  the `market-system-contracts` schema-validation dev dependencies
  (`jsonschema`, `referencing`), which are that sibling repository's
  concern, not this one's.
