# Traceability matrix — `smart-dynamic-hedge`

Full requirement text lives in `HLR.md`/`LLR.md`; this is the
quick-reference matrix for the two questions the methodology exists to
answer: forward (does this requirement have code and a test?) and
backward (does this test verify a named requirement?).

`Rust` and `Python` columns show test status: **T** = a passing test
exists, **O** = Open (implemented but untested), **—** = not applicable
(not ported to that language, or not implemented in that language at all).

| LLR | Traces to | Rust code | Rust test | Python code | Python test |
|---|---|---|---|---|---|
| SDH-LLR-001 | HLR-040 | `evaluate::evaluate_policy` | O | `policy.py` | O |
| SDH-LLR-002 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-003 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | T |
| SDH-LLR-004 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-005 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-006 | HLR-090 | `evaluate::evaluate_policy` | T | `policy.py` | T |
| SDH-LLR-007 | HLR-040 | `evaluate::evaluate_policy` | T | `policy.py` | T |
| SDH-LLR-008 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-009 | HLR-040 | `evaluate::evaluate_policy` | T | `policy.py` | T |
| SDH-LLR-010 | HLR-040 | `rounding::round_half_to_even` | T | `policy.py` (`round()`) | O |
| SDH-LLR-011 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-012 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-013 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-014 | HLR-050 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-015 | HLR-010 | `evaluate::evaluate_policy` | T | `policy.py` | O |
| SDH-LLR-020 | HLR-030 | `loader::load_config` | T | `config.py` | O |
| SDH-LLR-021 | HLR-030 | `loader::load_config` | T | `config.py` | O |
| SDH-LLR-022 | HLR-030 | `merge::deep_merge` | T | `config.py` | O |
| SDH-LLR-023 | HLR-030 | `loader::load_config` | T | `config.py` | O |
| SDH-LLR-024 | HLR-030 | `paths::resolve_project_path` | T | `config.py` | O |
| SDH-LLR-025 | HLR-030 | `types::ContractConfig` | T | `core_bridge.py` | O |
| SDH-LLR-030 | HLR-120 | `paths::default_binary_path` | T | `core_bridge.py` | O |
| SDH-LLR-031 | HLR-020 | `paths::resolve_binary` | O | `core_bridge.py` | O |
| SDH-LLR-032 | HLR-120 | `build::build_core`, `which::which` | T | `core_bridge.py` | O |
| SDH-LLR-033 | HLR-120 | `paths::windows_multi_config_fallback` | T (partial) | `core_bridge.py` | O |
| SDH-LLR-034 | HLR-020 | `build::ensure_core` | O | `core_bridge.py` | O |
| SDH-LLR-035 | HLR-020 | `run::run_core` | T | `core_bridge.py` | O |
| SDH-LLR-036 | HLR-020 | `run_with_timeout::run_command_with_timeout` | T | `core_bridge.py` | O |
| SDH-LLR-037 | HLR-020 | `core_response::CoreResponse` | T | `core_bridge.py` | O |
| SDH-LLR-038 | HLR-020 | — (C++) | O | — (C++ `json_number`) | O |
| SDH-LLR-050 | HLR-080 | — (not ported) | — | `model_advisor.py` | T |
| SDH-LLR-051 | HLR-080 | — | — | `model_advisor.py` | O |
| SDH-LLR-052 | HLR-080 | — | — | `model_advisor.py` | T |
| SDH-LLR-053 | HLR-080 | — | — | `model_advisor.py` | O |
| SDH-LLR-054 | HLR-080 | — | — | `model_advisor.py` | O |
| SDH-LLR-055 | HLR-100 | — | — | `model_advisor.py` | O |
| SDH-LLR-056 | HLR-100 | — | — | `model_advisor.py` | O |
| SDH-LLR-057 | HLR-100 | — | — | `engine.py` | O |
| SDH-LLR-060 | HLR-070 | `evidence::default_untrusted_text` | O | `models.py`, `data.py` | O |
| SDH-LLR-061 | HLR-070 | — | — | `model_advisor.py` | Inspection |
| SDH-LLR-062 | HLR-070 | — | — | `model_advisor.py` | O |
| SDH-LLR-070 | HLR-060 | `canonical::canonical_json` | T | `store.py` | T (transitive) |
| SDH-LLR-071 | HLR-060 | `canonical::hash_payload` | T | `store.py` | T (transitive) |
| SDH-LLR-072 | HLR-060 | `store::DecisionStore::get` | T | `store.py` | T (transitive) |
| SDH-LLR-073 | HLR-060 | `store::DecisionStore::{append,get}` | Inspection | `store.py`, `engine.py` | Inspection |
| SDH-LLR-080 | HLR-110 | — (not ported; needs `engine.rs`) | — | `engine.py` | Inspection |
| SDH-LLR-081 | HLR-140 | — (not ported; needs `data.rs`) | — | `data.py` | Inspection |
| SDH-LLR-082 | HLR-140 | — (not ported; needs `mcp_server.rs`) | — | `mcp_server.py` | O |
| SDH-LLR-090 | HLR-150 | `defaults::default_config_json` | O | `config.py`, `mcp_server.py` | Inspection |
| SDH-LLR-100 | HLR-160 | — (C++) | Inspection | — (C++) | Inspection |
| SDH-LLR-101 | HLR-160 | `Cargo.toml` | Inspection | n/a | n/a |
| SDH-LLR-110 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-111 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-112 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-113 | HLR-040/050 | `build::build_features` | T | `features.py` | O |

## Summary

- **HLRs**: 16 (`SDH-HLR-010` .. `SDH-HLR-160`).
- **LLRs**: 47.
- **Rust-verified (T)**: 31. **Rust-implemented-but-open**: 6.
  **Not applicable to Rust yet (not ported)**: 10 (model adviser/schema —
  `SDH-LLR-050` through `-062` — and the three interface-surface items
  `SDH-LLR-080` through `-082`, which need `engine.rs`/`data.rs`/
  `mcp_server.rs`).
- **Python-verified (T)**: 6 (mostly pre-existing `test_policy.py`/
  `test_model_schema.py` tests). **Python open**: most of the rest — the
  existing Python test suite is much thinner than the new Rust parity
  suite, which is itself a finding of this recovery pass, not a surprise:
  the Rust port added tests the Python original never had.
- **Known structural gap**: `SDH-LLR-080` (no-secrets/no-order-endpoint
  audit assertion) is self-asserted, not runtime-verified against the
  actual codebase shape. Closing it properly would mean a repo-wide
  static check ("no code path constructs an order-placement HTTP
  request"), which is future work, not a quick test to add.
- **124 Rust tests total** across `smart-hedge-{models,config,policy,
  core-bridge,features,store}`, all passing; `cargo clippy --workspace`
  clean.

## Next actions this recovery pass surfaced

1. Highest value: close the Python **O** rows for LLRs that already have
   a passing Rust test (SDH-LLR-002, -004, -005, -008, -010 through -015,
   -020 through -025, -030, -032, -035, -036, -037, -110 through -113) —
   these are cases where the Rust parity suite proves the *behavior* is
   correct but the Python original (still the running production code
   until cutover) has no direct test of its own.
2. `SDH-LLR-031`/`-034` (explicit binary override, auto-build gating): add
   direct tests in Rust — currently only exercised indirectly.
3. `SDH-LLR-050` through `SDH-LLR-062` (model adviser/schema) and
   `SDH-LLR-080` through `SDH-LLR-082` (audit/interface surfaces) are the
   remaining un-ported "CLI/dashboard/MCP-server layer" work — needs
   `model_advisor.rs`, `data.rs`, `engine.rs`, `cli.rs`, `dashboard.rs`,
   `mcp_server.rs`, roughly in that dependency order.
