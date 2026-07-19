# rust/

Isolated Rust workspace: the first slice of the Python-to-Rust migration
described in `../docs/ROADMAP.md` "Language and dependency policy". Built
with **zero changes to any existing Python or C++ file** — the strangler-fig
pattern the user asked for: prove the Rust side out fully in isolation,
then connect it to the CLI/dashboard/MCP entry points in a later phase.

## Status

| Crate | Ports | Status |
|---|---|---|
| `smart-hedge-models` | `python/smart_hedge/models.py` | fixture-tested — 30 tests, including a hand-rolled `UtcTimestamp`/`TimestampUtc`-style parser, the `CoreResponse` type matching the C++ core's exact JSON output, SHA-256 (verified against NIST vectors), and a UUID-v4-shaped unique-ID generator |
| `smart-hedge-config` | `python/smart_hedge/config.py` | fixture-tested — 29 tests; JSON-tree deep-merge (parity with Python's dict merge) feeding a statically-typed `Config`, not an untyped dict; `StrikeSpec` handles the `"ATM"`-or-number contract strike field |
| `smart-hedge-policy` | `python/smart_hedge/policy.py` | fixture-tested — 18 tests, including exact transcriptions of all four cases in `tests/test_policy.py` plus additional boundary coverage (`TRADE_SHARE_LIMIT`, `PREVIEW_NOTIONAL_LIMIT`, `NONFINITE_CORE_VALUE`, round-half-to-even) the Python suite doesn't currently exercise |
| `smart-hedge-core-bridge` | `python/smart_hedge/core_bridge.py` | fixture-tested + one real integration test — 7 tests, including one that actually builds and runs the real `cpp/smart_dynamic_hedge.cpp` binary end to end when a toolchain is available (skips gracefully otherwise) |
| `smart-hedge-features` | `python/smart_hedge/features.py` | fixture-tested — 33 tests covering data-quality composition, missing-feature marking, the volume-z-score/trend-score history-and-floor guards |
| `smart-hedge-store` | `python/smart_hedge/store.py` | fixture-tested — 20 tests, including one that directly corrupts a stored row via raw SQL and confirms replay detects the tamper |
| `smart-hedge-model-advisor` | `python/smart_hedge/model_advisor.py` (schema + `HeuristicAdvisor`; `OpenAIAdvisor` deferred) | fixture-tested — 25 tests, including exact transcriptions of `tests/test_model_schema.py`'s cases |
| `smart-hedge-data` | `python/smart_hedge/data.py` (`SyntheticProvider` + evidence-file loading; Alpaca/FRED/RSS deferred) | fixture-tested — 28 tests, including determinism tests across 5-second seed buckets |
| `smart-hedge-engine` | `python/smart_hedge/engine.py` | fixture-tested + real end-to-end integration tests — 25 tests, including a full `recommendation` → `replay`/`recent` round trip against the real C++ core, and both branches of the adviser-failure/fallback path via a deliberately-failing `Advisor` stub |
| `smart-hedge-cli` | `python/smart_hedge/cli.py` (`build-core`/`once`/`loop`/`replay`/`recent`/`self-test`; `serve`/`mcp` recognized but not yet implemented) | fixture-tested + real subprocess integration tests — 31 tests (23 unit + 8 integration), the integration tests shelling out to the actual compiled `smart-hedge` binary |

Not yet ported: `dashboard.py`, `mcp_server.py`, and the network-backed
providers/adviser (`AlpacaReadOnlyProvider`, FRED/RSS evidence,
`OpenAIAdvisor`) — each needs its own HTTP-client (or MCP-protocol)
dependency decision, deliberately deferred rather than silently stubbed
(see `SDH-LLR-126`, `-056`, `-081`, `-082` in `../requirements/LLR.md`).

**The zero-cost path — synthetic data + heuristic adviser + deterministic
core + policy gate + decision store — is now a fully working, independently
runnable Rust program** (`cargo run -p smart_hedge_cli --bin smart-hedge --
once`), not just a set of tested libraries. It is not yet the program a user
actually runs (`python/smart_hedge/cli.py` still is); cutover is a distinct,
later decision.

**Total: 246 tests, `cargo test --workspace` all green, `cargo clippy
--workspace --all-targets` clean under `clippy::all`.**

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

`smart-hedge-store` adds one more: `rusqlite` (`bundled` feature). The
SQLite file format (WAL, B-tree pages, journal recovery) is exactly the
kind of complex, correctness-critical format that's a *worse* trade-off to
hand-roll than to depend on — the same reasoning as `serde_json`, applied
to a much bigger surface. SHA-256, by contrast, *is* hand-rolled
(`smart_hedge_models::sha256`) since it's small, completely specified, and
has official NIST test vectors to verify against — see that module.

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
`SDH-LLR-072`'s correction note in `../requirements/LLR.md`). That last one
is the clearest evidence yet for why this migration insists on real
end-to-end tests, not just unit tests against hand-built fixtures: no unit
test happened to construct a float in the specific shape that triggers it.

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

## Connecting it together (in progress)

Per the plan agreed with the user: prove out each ported component fully
isolated first, then decide the cutover shape once more of the system
exists. Current direction (see `docs/ROADMAP.md`): a standalone Rust
`smart-hedge` binary (CLI + dashboard + MCP server) that eventually
replaces the Python package outright, not a PyO3 embedding — the latter
would keep a Python runtime in production permanently, which contradicts
the goal of getting away from Python.

`smart-hedge-cli` is the first slice of that binary: `build-core`, `once`,
`loop`, `replay`, `recent`, and `self-test` are real, working subcommands
against the synthetic-data/heuristic-adviser path. `serve` and `mcp` are
recognized but exit with an explicit "not yet implemented" error rather
than silently doing nothing — they need the dashboard/MCP-server ports
first, which in turn need HTTP-server and MCP-protocol dependency
decisions not yet made. The Python CLI (`python -m smart_hedge.cli`)
remains the one actually in use; nothing has cut over.

## Building and testing

```bash
cd rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

A `.cargo/config.toml` disables incremental compilation — see the comment
in that file; it works around this development machine's antivirus
intermittently corrupting incremental build artifacts.
