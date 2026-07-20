# Changelog

## Unreleased (Rust migration: sixth slice — traceability matrix complete, live-testing readiness workout)

Completed the requirements traceability matrix (every LLR now Rust-verified,
zero open items) and gave the system a real workout with fake data, per
direction to build confidence before considering this ready for
live-data (still paper-mode) testing.

- **Closed `SDH-LLR-080`'s structural gap**, open since the original
  requirements-recovery pass: added `smart-hedge-audit`, a new dependency-
  free crate that scans every `.rs` file in the workspace on every
  `cargo test` run and fails if any names or constructs an
  order-placement request (a `place_order`/`submit_order`/`cancel_order`-
  shaped identifier, a broker order-endpoint path, or a mutating HTTP verb
  outside the one file that legitimately needs `POST`). Includes four
  self-tests proving the checker actually detects a planted violation,
  correctly ignores mentions inside test code, and correctly allows the
  one legitimate `POST` call site. `python/` and `cpp/` were manually
  re-checked for the same patterns (clean) but aren't covered by an
  automated check, since Python is scheduled for cutover.
- **Found and fixed a real gap versus Python**: the `ureq` HTTP client
  integrations (Alpaca, FRED, RSS, OpenAI) had no response-body size cap
  at all, unlike Python's own defensive `response.read(2_000_000)`/
  `response.read(1_000_000)` calls in `data.py` — an unbounded
  `.into_string()` read could have let a misbehaving or hostile endpoint
  (an arbitrary operator-configured RSS feed URL, especially) exhaust
  memory. Fixed with `http_util::read_capped_body`, matching Python's
  exact bounds for Alpaca/FRED/RSS and a new 5MB bound for OpenAI (which
  Python doesn't explicitly cap either, since it's the `openai` SDK
  client, not a hand-rolled fetch).
- **Added adversarial "workout" test batteries** using deliberately
  extreme, malformed, or hostile fake data against real local mock
  servers — for Alpaca (extreme-magnitude/negative prices, null fields,
  5,000-bar responses), FRED (numeric/string value types, the `.`
  placeholder, infinity-overflow), RSS (truncated XML, 2,000-item feeds,
  unicode/CDATA), and OpenAI (non-JSON output, extra fields, evidence-ID
  arrays past the cap, absurd numeric values). Every case asserts only
  "never panics" — some are expected to succeed, others to fail cleanly.
- **Added a dedicated XXE-driven-SSRF proof for RSS**: a feed's
  `<!DOCTYPE>` declares an external entity pointing at a second,
  independent local "canary" server; the test asserts the canary is never
  contacted, proving the RSS parser's no-DTD-support design decision
  holds with a real, working HTTP client in the picture — not just in the
  parser's own unit-tested output text.
- **Added a randomized full-pipeline "chaos" test** in `smart-hedge-engine`:
  25 iterations (fixed-seed xorshift64 PRNG) across random symbols —
  including one with no configured contract at all — and boundary/
  out-of-range contract overrides, with an adviser that fails ~25% of
  calls unpredictably. Every iteration must either succeed with
  `mode: "paper"`/`live_execution_allowed: false` and a valid replay
  hash, or fail with one of a small, explicitly-allowed set of error
  variants; anything else (including a panic) fails the test.

**405 tests total across the Rust workspace (was 389), all passing,
`cargo clippy --workspace --all-targets` clean.** Updated
`requirements/LLR.md` (three new requirements, `SDH-LLR-157` through
`-159`, plus corrections closing `SDH-LLR-080`) and
`requirements/TRACEABILITY.md` (61 LLRs total, all Rust-verified — zero
"open" or "not applicable" rows for the first time). `rust/README.md`
gained a "Readiness for live testing" section spelling out exactly what
this pass verified and what real-credential testing would still need to
confirm.

## Unreleased (Rust migration: fifth slice — real end-to-end network testing, no more open gaps)

Closed the remaining honestly-reported test gaps from the previous pass
by building real local mock HTTP servers (`std::net::TcpListener`-based,
test-only, no new dependency) for Alpaca, FRED, RSS, and OpenAI, using the
existing Python source (`data.py`, `model_advisor.py`) as the reference
for the exact wire shapes to mock — per the user's direction to use the
Python code to define the behavior the Rust modules and their tests
should expect.

- `smart-hedge-data`: Alpaca and RSS needed no code change (their endpoint
  URLs are already configuration-driven) — new tests point
  `provider.alpaca.data_base_url`/`provider.rss.feeds` at local mock
  servers and make real `ureq` requests against them. FRED's URL isn't
  configurable (matching Python, which also hardcodes it), so it gained a
  small internal `base_url` parameter used only by tests. 92 tests (was 86).
- `smart-hedge-model-advisor`: `OpenAIAdvisor` gained a `#[cfg(test)]`-only
  `responses_url` override for the same reason. Building its mock server
  surfaced a real, intermittent (~1-in-5) bug: the mock never drained the
  POST request body before closing the connection, racing with `ureq`
  still writing it and occasionally producing a spurious transport error
  — fixed by draining the body per `Content-Length` first, the same
  defensive pattern `smart-hedge-dashboard`'s request parser already used.
  Caught by stress-running the new tests 20+ times, not by a single run.
  43 tests (was 39).
- `smart-hedge-core-bridge`: added direct tests for the two remaining
  indirectly-tested requirements — `resolve_binary`'s explicit-path
  precedence (`SDH-LLR-031`) and `ensure_core`'s `auto_build: false`
  gating (`SDH-LLR-034`), the latter asserting the *specific* error
  variant so a regression to "always attempt a build" would be caught,
  not just "some error occurred". 13 tests (was 7).

**389 tests total across the Rust workspace (was 373), all passing,
`cargo clippy --workspace --all-targets` clean.** Every LLR previously
marked "Rust-implemented-but-open" or noted as network-untestable is now
closed except the live third-party endpoints themselves, which no
automated test can reach without real credentials.

Updated `requirements/LLR.md` (addenda under `SDH-LLR-031`, `-034`,
`-056`, `-081`, `-126`) and `requirements/TRACEABILITY.md` (56→58
Rust-verified rows, 0 remaining "implemented-but-open").

## Unreleased (Rust migration: fourth slice — network providers, dashboard, MCP server; migration functionally complete)

Ported every remaining Python module, including the previously-deferred
network-dependent surfaces, with **zero changes to any file in `python/`
or `cpp/`**. The Rust migration now covers the entire Python package's
observable behavior; cutover remains a distinct, undecided future step.

Made and documented one new dependency decision: `ureq` (built on
`rustls`, a memory-safe pure-Rust TLS implementation — no system OpenSSL
dependency) for the three HTTPS **clients** this slice needed (Alpaca,
FRED, OpenAI). Scoped only to `smart-hedge-data` and
`smart-hedge-model-advisor`, not the whole workspace — same "documented
exception to hand-roll-instead-of-depend" reasoning as
`smart-hedge-store`'s `rusqlite`, since hand-rolling TLS is a security
non-starter.

- `smart-hedge-data` — added `AlpacaReadOnlyProvider` (read-only
  quote/bar HTTP client), FRED evidence loading, and RSS/Atom evidence
  loading. RSS parsing uses a new hand-rolled, narrowly-scoped XML text
  extractor (`rss_xml`) rather than a general XML library dependency —
  deliberately chosen *because* it never parses `<!DOCTYPE>`/`<!ENTITY>`
  declarations at all, which eliminates the XXE attack surface by
  construction instead of requiring it to be configured off. Also added
  `market_hours` (a hand-rolled, DST-aware NYSE-hours approximation,
  matching Python's own "not a full exchange calendar" caveat). 86 tests
  (was 28).
- `smart-hedge-model-advisor` — added `OpenAIAdvisor`, sending only
  derived, non-secret market data/evidence to the model (never a
  credential) and marking evidence text untrusted in the system
  instructions, matching the Python original's boundary. 39 tests (was 25).
- `smart-hedge-dashboard` (new crate) — full port of `dashboard.py`: the
  same read-only HTML console and `/api/health`, `/api/recommendation`,
  `/api/history`, `/api/replay/{id}` JSON endpoints, backed by a
  **hand-rolled** minimal HTTP/1.1 server (no framework dependency) —
  safe to hand-roll specifically because it needs no TLS (localhost only,
  matching Python's own `uvicorn` default) and only parses requests whose
  shape this process itself defines. 32 tests, 8 of them binding a real
  ephemeral port and making real HTTP requests against the running server.
- `smart-hedge-mcp` (new crate) — full port of `mcp_server.py`: a
  **hand-rolled** JSON-RPC 2.0 / MCP stdio server (no framework
  dependency) implementing `initialize`, `ping`, `tools/list`,
  `tools/call`, and all six tools (`health`, `get_market_recommendation`,
  `price_option`, `replay_decision`, `list_recent_decisions`,
  `get_policy_snapshot`) — no tool named or shaped like an order-placement
  tool. 19 tests.
- `smart-hedge-cli` — `serve` and `mcp` now launch the real dashboard/MCP
  servers instead of reporting "not yet implemented"; `serve` accepts
  `--host`/`--port` overrides. New end-to-end tests spawn the real binary
  as `serve` and make a real HTTP request against it, and spawn it as
  `mcp` and drive a real `initialize`/`tools/list` exchange over its
  stdio. 35 tests (was 31).

**373 tests total across the Rust workspace (was 246), all passing,
`cargo clippy --workspace --all-targets` clean.**

Updated `requirements/LLR.md` and `requirements/TRACEABILITY.md`: closed
`SDH-LLR-056`, `-081`, `-082`, and `-126` (all previously "deferred" or
"not ported"), corrected `SDH-LLR-141` (serve/mcp are real now, not
recognized-but-deferred), and added seven new requirements
(`SDH-LLR-150` through `-156`) for the dashboard and MCP server. 58 LLRs
total; 56 Rust-verified.

## Unreleased (Rust migration: third slice — engine + CLI, zero-cost path is now runnable)

Completed the zero-cost path (synthetic data + heuristic adviser +
deterministic core + policy gate + decision store) as a real, independently
runnable Rust program, still with **zero changes to any file in `python/`
or `cpp/`**:

- `smart-hedge-model-advisor` — full port of `model_advisor.py`'s schema
  validation and `HeuristicAdvisor` (`OpenAIAdvisor` deliberately deferred —
  needs an HTTP-client dependency decision), 25 tests including exact
  transcriptions of `tests/test_model_schema.py`'s cases and a regression
  test for a falsy-`or`-semantics porting trap (`ewma_volatility: 0.0` must
  fall back to `realized_volatility`, same as Python's `or`, which
  `Option::or` alone does not replicate).
- `smart-hedge-data` — full port of `data.py`'s `SyntheticProvider` and
  evidence-file loading (Alpaca/FRED/RSS deliberately deferred, same
  reason), 28 tests including determinism tests across 5-second seed
  buckets. Adds a hand-rolled xorshift64 PRNG — deliberately not a
  Mersenne-Twister port, since only same-seed determinism and plausible
  statistics matter here, not cross-language bit-identical output.
- `smart-hedge-engine` — full port of `engine.py`'s orchestration
  (contract/ATM/expiry resolution, canonical audit hashing, adviser-failure
  fallback, replay, health), 25 tests including real end-to-end integration
  tests against the actual C++ core binary and both branches of the
  adviser-fallback path via a deliberately-failing `Advisor` stub.
- `smart-hedge-cli` — a real `smart-hedge` binary porting the
  network-free subset of `cli.py`: `build-core`, `once`, `loop`, `replay`,
  `recent`, `self-test`. `serve`/`mcp` are recognized subcommands that
  report "not yet implemented" rather than "unknown command" or silently
  doing nothing. 31 tests (23 unit + 8 integration), the integration tests
  shelling out to the actual compiled binary — including one that persists
  a decision in one process and reads it back correctly in another.

**246 tests total across the Rust workspace (was 124), all passing,
`cargo clippy --workspace --all-targets` clean.**

Found and fixed a fourth real bug this session, this time via the CLI's
`self-test` integration test rather than a unit test: `serde_json`'s
default float parser doesn't guarantee exact round-tripping, so a decision
containing a float that wasn't already its own shortest round-trip
representation (ordinary floating-point arithmetic produces these
constantly) could reparse to a different bit pattern and fail its own
stored content-hash check on replay — a false tamper report against data
that was never tampered with. Fixed by enabling `serde_json`'s
`float_roundtrip` Cargo feature workspace-wide; see the correction note
under `SDH-LLR-072` in `requirements/LLR.md`.

Updated `requirements/LLR.md` and `requirements/TRACEABILITY.md`
accordingly: closed the "not yet ported" rows for `SDH-LLR-050` through
`-055`, `-057`, `-080`, `-120` through `-136`, and added four new CLI
requirements (`SDH-LLR-140` through `-143`). 51 LLRs total; 46 Rust-verified.

## Unreleased (requirements recovery + Rust migration: second slice)

Added a DO-178-inspired requirements-recovery baseline (`requirements/`:
`HLR.md`, `LLR.md`, `TRACEABILITY.md`) covering the C++ core, the full
Python package, and the Rust port to date — 16 high-level and 47
low-level requirements, each traced to source, implementation, and
verifying test(s), per the shared methodology now documented in
`market-system-contracts`'s `docs/REQUIREMENTS_METHODOLOGY.md`. This
recovery pass is itself where a real gap got found: the existing Python
test suite (`tests/test_policy.py`, `tests/test_model_schema.py`) verifies
far fewer of the system's actual behaviors than the new Rust parity suite
does — most Python LLR rows are marked `Open` in the traceability matrix,
honestly, rather than papered over.

Continued the Python-to-Rust migration with two more crates, same
zero-changes-to-Python/C++ discipline as the first slice:

- `smart-hedge-features` — full port of `features.py` (data-quality
  composition, missing-feature marking, volume-z-score/trend-score
  history-and-floor guards), 33 tests.
- `smart-hedge-store` — full port of `store.py` (canonical-JSON hashing,
  SHA-256 content hash, tamper-detecting replay), 19 tests. Adds
  `rusqlite` (`bundled`) as a deliberate, documented exception to the
  dependency-minimization policy — the SQLite file format is too complex
  and correctness-critical to safely hand-roll, the same reasoning that
  already justified keeping `serde_json`. SHA-256 itself *is* hand-rolled
  (`smart_hedge_models::sha256`), verified against four official NIST/
  FIPS 180-4 test vectors including the million-character stress vector.

124 tests total across the Rust workspace (was 66), all passing,
`cargo clippy --workspace` clean. Found and fixed a third real bug this
session: a hand-transcription typo in one SHA-256 test's own expected
constant, caught by comparing against Python's `hashlib.sha256` directly
rather than trusting a memorized value — the implementation itself was
already correct on all three other independent NIST vectors.

## Unreleased (Rust migration: first slice)

Started the Python-to-Rust migration in an isolated `rust/` workspace, with
**zero changes to any file in `python/` or `cpp/`** — strangler-fig style,
per the plan agreed with the user: build and fully prove out the
replacement before touching anything that currently interfaces with it.

- Ported `models.py`, `config.py`, and `policy.py` in full, plus enough of
  `core_bridge.py` to build/resolve/invoke the existing C++ binary
  end-to-end (cross-platform: `.exe` suffix handling, Windows multi-config
  generator fallback, `cmake`/`g++`/`clang++` toolchain discovery all
  ported faithfully) — four crates: `smart-hedge-models`,
  `smart-hedge-config`, `smart-hedge-policy`, `smart-hedge-core-bridge`.
- 66 tests total, all passing, including exact transcriptions of all four
  cases in `tests/test_policy.py` and one real integration test that builds
  and runs the actual `cpp/smart_dynamic_hedge.cpp` binary.
- Same dependency-minimization and testing policy as the two Rust sibling
  repositories: only `serde`/`serde_json` as third-party dependencies,
  `unsafe_code` forbidden workspace-wide, hand-rolled dependency-free
  fuzz-smoke tests. Found and fixed two real bugs this way before they
  could ship: an `Option::then_some` eager-evaluation panic in the
  timestamp parser, and a missing `ContractConfig` field default that would
  have broken adding a new contract symbol with partial fields.
- Documented deliberate behavioral differences from the Python originals in
  `rust/README.md` (no filesystem-touching path resolution, malformed core
  responses caught at the parse boundary instead of inside the policy
  function, a from-scratch round-half-to-even implementation matching
  Python's `round()` instead of Rust's round-half-away-from-zero
  `f64::round()`).
- Nothing is connected to a real running binary yet; see `rust/README.md`
  "Connecting it together" and `docs/ROADMAP.md` for the plan.

## Unreleased — 2026-07-19

- Adopted the V2 multi-repository architecture: created and scaffolded
  sibling repositories `market-system-contracts`, `market-intelligence-mcp`,
  and `trade-guard-mcp`. See `docs/ROADMAP.md` "V2 multi-repository
  expansion" for their status and `README.md` "Related repositories".
- Added `.gitignore` (none existed before) excluding `.scratch/`, `build/`,
  `.venv/`, and other local artifacts.
- Reworded `NOTICE`, `LEGAL_NOTICE.md`, and `docs/ROADMAP.md`: live trading
  is a real, intended goal of the overall Smart Dynamic Hedge system,
  provided exclusively through the separate `trade-guard-mcp` repository —
  previous wording read as if live trading were unintended or merely a
  downstream fork's concern, which was not accurate to the project's actual
  direction.
- Documented the language/dependency-minimization decision for this
  repository in `docs/ROADMAP.md` "Language and dependency policy": keep
  the existing tested C++ deterministic core as-is; the Python
  orchestration layer is a migration candidate to Rust (not started) so the
  whole system eventually shares one toolchain and one dependency/testing
  policy with the two Rust sibling repositories.
- No source code in `cpp/` or `python/` changed this entry — this is a
  documentation/repository-structure changelog entry only.
