# rust/

The Rust workspace that implements this project, per the Python-to-Rust
migration described in `../docs/ROADMAP.md` "Language and dependency
policy". The migration followed a strangler-fig pattern — every module was
proven out fully in isolation, with zero changes to the Python or C++ code,
before cutover — and **cutover is now complete**: the `smart-hedge` binary
built from this workspace is the only supported implementation. The former
`python/` package and its `tests/` suite have been removed from the active
tree; they remain available in git history for reference.

## Status

Every module that used to live in `python/smart_hedge/` now has a Rust
port, and the port is what actually runs (see "Connecting it together"
below). The table below documents provenance — the pre-cutover Python file
each crate replaced — even though those files no longer exist in the
working tree.

| Crate | Ports | Status |
|---|---|---|
| `smart-hedge-models` | `python/smart_hedge/models.py` | fixture-tested — 30 tests, including a hand-rolled `UtcTimestamp`/`TimestampUtc`-style parser, the `CoreResponse` type matching the C++ core's exact JSON output, SHA-256 (verified against NIST vectors), and a UUID-v4-shaped unique-ID generator |
| `smart-hedge-config` | `python/smart_hedge/config.py` | fixture-tested — 29 tests; JSON-tree deep-merge (parity with Python's dict merge) feeding a statically-typed `Config`, not an untyped dict; `StrikeSpec` handles the `"ATM"`-or-number contract strike field |
| `smart-hedge-policy` | `python/smart_hedge/policy.py` | fixture-tested — 18 tests, including exact transcriptions of all four cases in `tests/test_policy.py` plus additional boundary coverage (`TRADE_SHARE_LIMIT`, `PREVIEW_NOTIONAL_LIMIT`, `NONFINITE_CORE_VALUE`, round-half-to-even) the Python suite doesn't currently exercise |
| `smart-hedge-core-bridge` | `python/smart_hedge/core_bridge.py` | fixture-tested + real integration tests — 13 tests, including one that actually builds and runs the real `cpp/smart_dynamic_hedge.cpp` binary end to end when a toolchain is available (skips gracefully otherwise), plus direct coverage of the explicit-binary-override and `auto_build: false` gating paths |
| `smart-hedge-features` | `python/smart_hedge/features.py` | fixture-tested — 33 tests covering data-quality composition, missing-feature marking, the volume-z-score/trend-score history-and-floor guards |
| `smart-hedge-store` | `python/smart_hedge/store.py` | fixture-tested — 20 tests, including one that directly corrupts a stored row via raw SQL and confirms replay detects the tamper |
| `smart-hedge-model-advisor` | `python/smart_hedge/model_advisor.py` (schema, `HeuristicAdvisor`, `OpenAIAdvisor`) | fixture-tested + real end-to-end + adversarial tests — 46 tests, including exact transcriptions of `tests/test_model_schema.py`'s cases, real HTTP round trips against a local mock OpenAI server, and a battery of a dozen deliberately hostile fake model outputs (the live `api.openai.com` endpoint itself still isn't exercised — see `SDH-LLR-056`) |
| `smart-hedge-data` | `python/smart_hedge/data.py` (`SyntheticProvider`, `AlpacaReadOnlyProvider`, evidence-file/FRED/RSS loading) | fixture-tested + real end-to-end + adversarial tests — 99 tests, including a hand-rolled, DTD/entity-free RSS/Atom XML extractor tested against CDATA, XML entities, and a deliberate XXE-attempt fixture that proves the entity is never expanded, real HTTP round trips against local mock Alpaca/FRED/RSS servers, adversarial-response batteries per provider, and a real XXE-driven-SSRF proof using an independent "canary" server that must never be contacted |
| `smart-hedge-engine` | `python/smart_hedge/engine.py` | fixture-tested + real end-to-end + randomized workout tests — 26 tests, including a full `recommendation` → `replay`/`recent` round trip against the real C++ core, both branches of the adviser-failure/fallback path, and a 25-iteration randomized "chaos" run (random symbols including unconfigured ones, boundary/out-of-range contract overrides, an unpredictably-failing adviser) asserting no panic and paper-only invariants hold throughout |
| `smart-hedge-dashboard` | `python/smart_hedge/dashboard.py` | fixture-tested + real end-to-end integration tests — 32 tests, including 8 that bind a real ephemeral TCP port, run the real accept loop, and make real HTTP requests against it |
| `smart-hedge-mcp` | `python/smart_hedge/mcp_server.py` | fixture-tested — 19 tests covering the JSON-RPC 2.0 envelope, all six tools, and the MCP-specific "tool failure is an `isError` result, not a protocol error" distinction |
| `smart-hedge-cli` | `python/smart_hedge/cli.py` (`build-core`/`once`/`loop`/`replay`/`recent`/`self-test`/`serve`/`mcp` — every subcommand) | fixture-tested + real subprocess integration tests — 35 tests (26 unit + 9 integration), including spawning the real binary as `serve` and making a real HTTP request against it, and spawning it as `mcp` and driving a real JSON-RPC exchange over its stdio |
| `smart-hedge-audit` | (no Python equivalent — a new, repo-wide structural check) | 5 tests: a real scan of every `.rs` file in this workspace asserting none names or constructs an order-placement request, plus four self-tests proving the checker actually detects a planted violation rather than being vacuously true |
| `smart-hedge-mcp-client` | (no Python equivalent — new, for the V2 multi-repository integration) | fixture-tested + real end-to-end — 7 tests. A generic, dependency-free MCP stdio JSON-RPC client (spawn a server binary, one request per line, read one response line), the client-side counterpart to this workspace's own `smart-hedge-mcp` server. Tests spawn this repository's own `smart-hedge` binary as the server under test, so this crate's suite needs no sibling repository built. |
| `smart-hedge-intelligence-client` | (no Python equivalent) | 1 test — a thin typed wrapper over `smart-hedge-mcp-client` for `market-intelligence-mcp`'s 11 read-only tools (`health`, `list-configured-sources`, `build-evidence-bundle`, etc.). |
| `smart-hedge-guard-client` | (no Python equivalent) | fixture-tested — 4 tests, including `build_trade_intent`'s `decimal-string` formatting (`common.schema.json`'s exact grammar) verified against several boundary cases. A thin typed wrapper over `smart-hedge-mcp-client` for `trade-guard-mcp`'s `authorize-and-submit-paper-order` and account-snapshot tools. |
| `smart-hedge-portfolio` | (no Python equivalent — Phase 4 "portfolio pricing/Greeks/hedging expansion") | fixture-tested + real end-to-end — 6 tests. Calls the *unchanged* C++ core once per position and aggregates into dollar-denominated portfolio Greeks (dollar delta, dollar gamma P&L, dollar vega/theta/rho, stock/option notional) — additive across different underlyings, unlike raw per-underlying share counts. No pricing math lives here; see the crate's own module doc comment for why that split matters. |

**The full CLI surface — including `serve` (a real HTTP dashboard) and
`mcp` (a real MCP stdio server) — is now a fully working, independently
runnable Rust program** (`cargo run -p smart_hedge_cli --bin smart-hedge --
once`), and it is the program a user actually runs: `python/smart_hedge/cli.py`
has been removed as part of cutover.

**`guard-demo` is a new subcommand that exercises the full three-repository
V2 architecture end to end**: it runs this repository's own deterministic
recommendation, spawns `market-intelligence-mcp`'s real MCP server to fetch
a real evidence bundle, builds a typed `TradeIntent` from the
recommendation's paper-trade preview, and spawns `trade-guard-mcp`'s real
MCP server to authorize-and-submit it against that repository's real paper
simulator — three independently-built binaries from three separate
repositories, talking over real subprocess/stdio boundaries, not fixtures.
See "Connecting the three repositories" below.

**Total: 425 tests, `cargo test --workspace` all green, `cargo clippy
--workspace --all-targets` clean under `clippy::all`.**

### Testing the network providers without live credentials

The Alpaca/FRED/RSS/OpenAI integrations are tested against **real local
mock HTTP servers** (`std::net::TcpListener`-based, test-only, no
dependency), not just hand-built `serde_json::Value` fixtures — a genuine
`ureq` request goes out over real loopback TCP and gets parsed by the real
response-handling code. Alpaca and RSS needed no code change (their
endpoint URLs are already configuration-driven); FRED and OpenAI needed a
small internal test-only seam (`load_fred_evidence`'s `base_url`
parameter, `OpenAIAdvisor::with_responses_url`) since neither URL is
configurable in Python either. The exact response shapes these mocks
return are cross-referenced from `python/smart_hedge/data.py`'s and
`model_advisor.py`'s own field access — the Python source is the spec for
the wire format, not a runtime dependency. Building the OpenAI mock
surfaced one real, intermittent (~1-in-5) bug: its first version never
drained the POST request body before closing the connection, which raced
with `ureq` still writing it and occasionally produced a spurious
transport error — fixed by draining the body per `Content-Length` first.
Only the *live* third-party endpoints remain outside what an automated
test can verify (no real credentials in CI).

### Adversarial "workout" testing with fake data

Beyond proving the happy path works, every network integration and the
engine's full pipeline are also exercised with deliberately extreme,
malformed, or hostile fake data — the point is to build confidence
*before* ever pointing this at a real feed, not after:

- **Alpaca**: 1e300-magnitude and negative prices, null OHLC fields,
  non-JSON garbage, a 5,000-bar response, unicode in the timestamp field,
  and a response larger than the size cap.
- **FRED**: numeric-vs-string `value` types, the `.` no-data placeholder,
  `null`, a value that overflows to `f64::INFINITY`, missing fields, and
  500 observations (only the first is ever used).
- **RSS**: truncated/malformed XML, 2,000-item feeds, unicode/emoji
  content, `CDATA` sections containing markup, and — the most
  security-relevant case — a feed whose `<!DOCTYPE>` declares an external
  entity pointing at a second, independent local "canary" server (the
  classic XXE-driven SSRF payload shape). The test asserts the canary is
  *never contacted*, proving the no-DTD-support design decision holds
  with a real, working HTTP client in the picture, not just in unit-level
  parser output.
- **OpenAI**: non-JSON model output, an extra `buy_shares`-shaped field,
  evidence-ID arrays far past the schema cap, an absurd `band_multiplier`,
  unicode/quote-heavy content, and an oversized response.
- **Engine**: a 25-iteration randomized run (fixed-seed xorshift64 PRNG)
  across random symbols — including one with no configured contract at
  all — and boundary/out-of-range contract overrides, with an adviser
  that fails ~25% of calls unpredictably. Every iteration must either
  succeed with `mode: "paper"`/`live_execution_allowed: false` and a
  valid replay hash, or fail with one of a small, explicitly-allowed set
  of error variants — anything else (including a panic) fails the test.

Building this out found two more real bugs beyond the ones already
documented below: an unbounded response-body read (see next paragraph),
and confirmation that the existing size-cap fix actually engages under a
genuinely oversized fake payload, not just in principle.

**Response bodies are now capped**, matching Python's own defensive
`response.read(2_000_000)`/`response.read(1_000_000)` calls in `data.py`
— the initial `ureq` integration read every response with `.into_string()`
unbounded, which would have let a misbehaving or hostile endpoint (an
arbitrary operator-configured RSS feed URL, especially) exhaust memory.
`smart_hedge_data::http_util::read_capped_body` (and a duplicated
equivalent in `smart_hedge_model_advisor`) fixes this; see `SDH-LLR-157`.

## Requirements traceability

This migration is tracked against a DO-178-inspired requirements-recovery
baseline in `../requirements/` (`HLR.md`, `LLR.md`, `TRACEABILITY.md`) —
see `market-system-contracts`'s `docs/REQUIREMENTS_METHODOLOGY.md` for the
scheme. Every crate above exists to satisfy specific recovered
requirements, not just to "port a file"; the traceability matrix is the
place to check what's actually verified versus still open.

## Dependency and testing policy

Same as `market-intelligence-mcp`/`trade-guard-mcp`: `serde`/`serde_json`
are the baseline third-party dependencies (kept deliberately — hand-rolling
JSON parsing would be a worse security trade-off, not a better one), every
crate forbids `unsafe_code` and warns on `clippy::all`
(`[workspace.lints]`), and testing favors hand-rolled, dependency-free
boundary/fuzz-smoke tests over pulling in `proptest`/`cargo-fuzz`.
`smart-hedge-audit` is the same philosophy applied to a repo-wide
structural property (no order-placement code path anywhere) instead of a
single function's behavior — a plain-text scan over `cargo test`, not a
dependency on a static-analysis framework.

Two more crates add documented exceptions, same "worse trade-off to
hand-roll than to depend on" reasoning:

- `smart-hedge-store`: `rusqlite` (`bundled` feature). The SQLite file
  format (WAL, B-tree pages, journal recovery) is exactly the kind of
  complex, correctness-critical format that's a *worse* trade-off to
  hand-roll than to depend on.
- `smart-hedge-data` and `smart-hedge-model-advisor`: `ureq` (on `rustls`,
  a memory-safe pure-Rust TLS implementation — no system OpenSSL
  dependency). The Alpaca/FRED/OpenAI HTTP **clients** need real HTTPS
  calls to third-party hosts; hand-rolling TLS is a security non-starter.
  Scoped only to these two crates, not the whole workspace.

Notably, the dashboard's HTTP **server** and the MCP JSON-RPC **stdio**
server (`smart-hedge-dashboard`, `smart-hedge-mcp`) add **no** new
dependency at all, despite superficially looking like the same kind of
problem as the HTTP clients — see "Known, documented behavioral
differences from Python" below for why hand-rolling those specifically is
the safer choice, not a shortcut.

SHA-256, by contrast, *is* hand-rolled (`smart_hedge_models::sha256`)
since it's small, completely specified, and has official NIST test
vectors to verify against — see that module.

This pass already found and fixed several real bugs purely from writing the
tests, none of which the Python original had to worry about (being
untyped, or simply never exercised this hard): an `Option::then_some`
eager-evaluation panic in the timestamp parser's digit-conversion helper; a
missing `#[serde(default)]` on `ContractConfig` fields that would have made
adding a new contract symbol with only partial fields (which Python's
dict-merge tolerates) fail to deserialize in Rust; a hand-transcription
typo in one of the SHA-256 test's own "expected" constants (caught by, and
then resolved against, an independent check via Python's `hashlib` — the
implementation was correct, the memorized test literal wasn't); and, found
via the `smart-hedge-cli` self-test integration test, a `serde_json`
float-parsing default that silently broke the decision store's
hash-after-replay integrity check for any payload containing a float that
wasn't already its own shortest round-trip representation (fixed by
enabling the `float_roundtrip` Cargo feature workspace-wide — see
`SDH-LLR-072`'s correction note in `../requirements/LLR.md`); and a
test-double bug in the OpenAI mock server (see "Testing the network
providers without live credentials" above) that produced an intermittent,
~1-in-5 spurious test failure until fixed. Both of the last two are the
clearest evidence yet for why this migration insists on real end-to-end
tests, not just unit tests against hand-built fixtures: neither was
visible until a real TCP/HTTP round trip was actually exercised.

## Known, documented behavioral differences from Python

- **`project_root()` has no Rust equivalent.** Python derives it from
  `__file__`; a compiled binary has nothing analogous. `load_config` and
  `core_bridge` functions take `project_root: &Path` as an explicit
  parameter instead of guessing — the future CLI/dashboard entry point
  decides that (current working directory, or an explicit flag), not this
  library code.
- **`resolve_project_path` normalizes lexically, not via the filesystem.**
  Python's `Path.resolve()` touches the filesystem to resolve symlinks it
  can find; this crate's `lexically_normalize` only collapses `.`/`..`
  components without touching disk, so it works identically for paths that
  don't exist yet (e.g. a `storage.sqlite_path` before its first run).
- **A malformed C++ core response is now caught at the JSON-parsing
  boundary**, not inside `evaluate_policy`. Python indexes an untyped dict
  and catches `KeyError`/`TypeError` inside the policy function itself;
  `CoreResponse` deserialization fails the same way at the point
  `core_bridge::run_core` parses the subprocess's stdout, before policy
  ever sees it. Every case `tests/test_policy.py` actually exercises is
  unaffected — all four of its test cases pass with this crate unchanged.
- **`round()` is deliberately re-implemented**, not delegated to
  `f64::round()` — see `smart_hedge_policy::rounding` for why (Python's
  `round()` is round-half-to-even; Rust's `f64::round()` is round-half-
  away-from-zero, and share counts routinely land exactly on a half-share
  boundary since `0.5` is exactly representable in binary).
- **RSS/Atom feed parsing is a hand-rolled, narrowly-scoped XML text
  extractor** (`smart_hedge_data::rss_xml`), not `xml.etree.ElementTree`
  or a general-purpose Rust XML crate. It only extracts the text of a
  handful of named leaf elements inside `<item>`/`<entry>` blocks, and it
  never parses `<!DOCTYPE ...>` internal subsets or `<!ENTITY ...>`
  declarations at all — it skips over them as opaque bytes. That omission
  is what actually prevents XXE (external entity expansion): there is no
  code path that could ever resolve an external entity, because entity
  declarations are never inspected in the first place. A general XML
  library with DTD/entity support would need to be explicitly configured
  to disable it to get the same guarantee; this parser gets it for free,
  by construction. Verified directly by a test that feeds it a
  `<!DOCTYPE>` declaring `<!ENTITY xxe SYSTEM "file:///etc/passwd">` and
  confirms the literal text `&xxe;` passes through undecoded.
- **The dashboard's HTTP server and the MCP server's stdio transport are
  both hand-rolled**, with no HTTP/JSON-RPC framework dependency — safe to
  do specifically because neither needs TLS (both are local-only, matching
  Python's own `uvicorn` dashboard default and MCP's stdio-only transport)
  and both only ever parse messages whose shape this process itself
  defines, unlike the *client* side (`ureq`/`rustls`), which parses
  arbitrary third-party HTTPS responses and genuinely needs a dependency.

## Connecting it together (cutover complete)

Per the plan agreed with the user: prove out each ported component fully
isolated first, then decide the cutover shape once more of the system
exists. That decision has now been made and executed (see `docs/ROADMAP.md`):
a standalone Rust `smart-hedge` binary (CLI + dashboard + MCP server) has
replaced the Python package outright — not a PyO3 embedding, which would
have kept a Python runtime in production permanently, contradicting the
goal of getting away from Python.

`smart-hedge-cli` is that binary, and every subcommand the former `cli.py`
had is implemented: `build-core`, `once`, `loop`, `replay`, `recent`,
`self-test`, `serve` (a real HTTP dashboard, hand-rolled server), and `mcp`
(a real MCP stdio server, hand-rolled JSON-RPC). The network-backed
providers/adviser (Alpaca, FRED, RSS, OpenAI) are implemented against real
HTTPS endpoints via `ureq`/`rustls`. Nothing here has been benchmarked or
run under real production load or against genuinely live market/model
feeds yet — see "Readiness for live testing" below for exactly what has and
hasn't been verified so far.

### Readiness for live testing

"Live" here always means **live market/model data feeds while remaining
in paper mode** — this repository has no order-placement capability at
all, by design and now by an automatic repo-wide check
(`smart_hedge_audit`, `SDH-LLR-158`); live order execution is out of
scope for this repository entirely (see `docs/THREAT_MODEL.md` and
`docs/ROADMAP.md` "V2 multi-repository expansion" — that capability lives
exclusively in the separate `trade-guard-mcp` repository).

What real-fake-data testing (this pass) has verified:

- Every network integration's request-shaping and response-parsing code
  survives a real HTTP round trip against a wide range of adversarial
  fake responses without panicking, without exceeding its size cap, and
  without ever resolving an XXE/SSRF-shaped payload.
- The full engine pipeline survives many randomized fake scenarios without
  panicking and without ever leaving paper mode.
- The one structural safety property this whole system depends on (no
  order-placement code path) is checked automatically, not just asserted.

What real-fake-data testing **cannot** verify, and real (live-data,
paper-mode) testing would still need to confirm before relying on this
port day to day:

- The real Alpaca/FRED/RSS/OpenAI endpoints' actual current response
  shapes match what this port's mocks assume — API behavior can drift
  independently of this codebase.
- Real-world timing/latency/rate-limit behavior under the real endpoints,
  as opposed to a local mock server that always responds instantly.
- Real credential handling end to end (this pass could only verify that
  credentials are read correctly and never leaked into a payload — see
  `openai::tests::build_payload_never_includes_a_secrets_field` — not that
  they authenticate successfully against a live account).
- Extended-duration/soak behavior beyond the ~25-iteration chaos test
  (SQLite file growth, long-running `serve`/`mcp` process stability).

None of the above blocks starting live-data testing — they're exactly the
things live-data testing exists to check, not gaps this pass could have
closed with more fake data.

## Connecting the three repositories

`docs/ROADMAP.md` "V2 multi-repository expansion" describes Phase 4 as
"add typed clients for the two sibling services... and paper guard
integration" — that gate was waiting on `market-intelligence-mcp` and
`trade-guard-mcp` each reaching their own vertical slice, which they now
have. This pass implements the smallest complete version of that phase,
not the full Phase 4 laundry list (no international schemas, no
`MODEL_URI` router, no portfolio-level Greeks, no backtester — those
remain future work).

```bash
export MARKET_INTELLIGENCE_MCP_BIN=/path/to/market-intelligence-mcp/target/release/market_intelligence_server
export TRADE_GUARD_MCP_BIN=/path/to/trade-guard-mcp/target/release/trade_guard_server
./target/release/smart-hedge --config ../config.example.json guard-demo --symbol SPY
```

(`--intelligence-binary`/`--guard-binary` flags override the env vars,
matching the `--config`/`SMART_HEDGE_CONFIG` precedent.) `guard-demo`:

1. runs this repository's own `recommendation` pipeline, unchanged;
2. if the policy's `action` isn't `paper_rebalance_preview` (nothing to
   propose), stops there — no sibling process is spawned at all;
3. otherwise spawns `market-intelligence-mcp`'s real MCP server, ingests
   its one demo fixture record, and builds a real `EvidenceBundle` via
   its `build-evidence-bundle` tool;
4. builds a typed `TradeIntent` (`smart_hedge_guard_client::build_trade_intent`)
   from the recommendation's `paper_trade_preview_shares`;
5. spawns `trade-guard-mcp`'s real MCP server and calls its
   `authorize-and-submit-paper-order` tool with the intent and evidence
   bundle, printing the full result — including a real paper fill when
   the guard's own policy (buying power, evidence eligibility) allows it.

Verified working end to end against all three repositories' independently
built release binaries: a real recommendation, a real evidence bundle
(`bundle-purpose: research`, one record, `quarantine-count: 0`), and a
real paper fill (`state: filled`, a real synthetic fill price, a real
position/cash update in `trade-guard-mcp`'s own account ledger) — three
separate processes from three separate repositories, talking only over
stdio, with no shared code beyond the wire format both sides hand-
transcribe from `market-system-contracts`.

An early version of this test caught a real bug in the demo's own
timestamp handling: reusing the recommendation's `created_at` as the
`TradeIntent`'s `decision-time` made `check-evidence-eligibility`
correctly reject the intent (`evidence-bundle-created-after-decision`),
since the evidence bundle — built moments later — necessarily postdated
it. Fixed by taking a fresh timestamp right before submission instead of
backdating to when the underlying recommendation was computed — see the
comment at that call site in `smart-hedge-cli`'s `commands.rs`.

**What this does and doesn't prove**: this proves the intended
`TradeIntent -> trade-guard-mcp` data flow works for real, with real
independently-built binaries, not just fixtures within one process — a
qualitatively different (and stronger) claim than "each repository's own
test suite passes in isolation." It does **not** prove anything about
concurrent/production load (each demo run is one sequential subprocess
call), about a real (non-fixture) intelligence source, or about anything
beyond the paper-only path — `trade-guard-mcp` has no live-execution path
in source at all, so there is nothing further this integration could
exercise on that axis yet.

## Building and testing

```bash
cd rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

A `.cargo/config.toml` disables incremental compilation — see the comment
in that file; it works around this development machine's antivirus
intermittently corrupting incremental build artifacts. The same overhead
means `cargo test --workspace` takes a couple of minutes here (dominated
by `smart-hedge-core-bridge`'s real-toolchain test and
`smart-hedge-engine`'s chaos workout, each doing real subprocess/file-
system round trips) — a machine without that overhead will be faster.
