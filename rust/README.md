# rust/

Isolated Rust workspace: the first slice of the Python-to-Rust migration
described in `../docs/ROADMAP.md` "Language and dependency policy". Built
with **zero changes to any existing Python or C++ file** — the strangler-fig
pattern the user asked for: prove the Rust side out fully in isolation,
then connect it to the CLI/dashboard/MCP entry points in a later phase.

## Status

| Crate | Ports | Status |
|---|---|---|
| `smart-hedge-models` | `python/smart_hedge/models.py` | fixture-tested — 19 tests, including a hand-rolled `UtcTimestamp`/`TimestampUtc`-style parser and the `CoreResponse` type matching the C++ core's exact JSON output |
| `smart-hedge-config` | `python/smart_hedge/config.py` | fixture-tested — 22 tests; JSON-tree deep-merge (parity with Python's dict merge) feeding a statically-typed `Config`, not an untyped dict |
| `smart-hedge-policy` | `python/smart_hedge/policy.py` | fixture-tested — 18 tests, including exact transcriptions of all four cases in `tests/test_policy.py` plus additional boundary coverage (`TRADE_SHARE_LIMIT`, `PREVIEW_NOTIONAL_LIMIT`, `NONFINITE_CORE_VALUE`, round-half-to-even) the Python suite doesn't currently exercise |
| `smart-hedge-core-bridge` | `python/smart_hedge/core_bridge.py` | fixture-tested + one real integration test — 7 tests, including one that actually builds and runs the real `cpp/smart_dynamic_hedge.cpp` binary end to end when a toolchain is available (skips gracefully otherwise) |

Not yet ported: `cli.py`, `dashboard.py`, `data.py`, `engine.py`,
`features.py`, `mcp_server.py`, `model_advisor.py`, `store.py`. Nothing in
this workspace is wired to a real running binary yet — see "Connecting it
together" below.

**Total: 66 tests, `cargo test --workspace` all green, `cargo clippy
--workspace` clean under `clippy::all`.**

## Dependency and testing policy

Same as `market-intelligence-mcp`/`trade-guard-mcp`: `serde`/`serde_json`
are the only third-party dependencies (kept deliberately — hand-rolling
JSON parsing would be a worse security trade-off, not a better one), every
crate forbids `unsafe_code` and warns on `clippy::all`
(`[workspace.lints]`), and testing favors hand-rolled, dependency-free
boundary/fuzz-smoke tests over pulling in `proptest`/`cargo-fuzz`.

This pass already found and fixed two real bugs the Python original didn't
have (or didn't need to worry about, being untyped) purely from writing the
tests: an `Option::then_some` eager-evaluation panic in the timestamp
parser's digit-conversion helper, and a missing `#[serde(default)]` on
`ContractConfig` fields that would have made adding a new contract symbol
with only partial fields (which Python's dict-merge tolerates) fail to
deserialize in Rust.

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

## Connecting it together (not started)

Per the plan agreed with the user: prove out each ported component fully
isolated first, then decide the cutover shape once more of the system
exists. Current direction (see `docs/ROADMAP.md`): a standalone Rust
`smart-hedge` binary (CLI + dashboard + MCP server) that eventually
replaces the Python package outright, not a PyO3 embedding — the latter
would keep a Python runtime in production permanently, which contradicts
the goal of getting away from Python.

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
