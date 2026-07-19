# Changelog

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
