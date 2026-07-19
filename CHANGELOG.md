# Changelog

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
