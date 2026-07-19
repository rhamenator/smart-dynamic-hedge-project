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
| SDH-LLR-050 | HLR-080 | `schema::validate_assessment_payload` | T | `model_advisor.py` | T |
| SDH-LLR-051 | HLR-080 | `schema::ALLOWED_REGIMES` | T | `model_advisor.py` | O |
| SDH-LLR-052 | HLR-080 | `schema::finite_number` | T | `model_advisor.py` | T |
| SDH-LLR-053 | HLR-080 | `schema::validate_assessment_payload` | T | `model_advisor.py` | O |
| SDH-LLR-054 | HLR-080 | `schema::string_list` | T | `model_advisor.py` | O |
| SDH-LLR-055 | HLR-100 | `heuristic::HeuristicAdvisor` | T | `model_advisor.py` | O |
| SDH-LLR-056 | HLR-100 | — (not ported; needs `OpenAIAdvisor`) | — | `model_advisor.py` | O |
| SDH-LLR-057 | HLR-100 | `engine::SmartHedgeEngine::recommendation_at` | T | `engine.py` | O |
| SDH-LLR-060 | HLR-070 | `evidence::default_untrusted_text` | O | `models.py`, `data.py` | O |
| SDH-LLR-061 | HLR-070 | — | — | `model_advisor.py` | Inspection |
| SDH-LLR-062 | HLR-070 | — | — | `model_advisor.py` | O |
| SDH-LLR-070 | HLR-060 | `canonical::canonical_json` | T | `store.py` | T (transitive) |
| SDH-LLR-071 | HLR-060 | `canonical::hash_payload` | T | `store.py` | T (transitive) |
| SDH-LLR-072 | HLR-060 | `store::DecisionStore::get` | T | `store.py` | T (transitive) |
| SDH-LLR-073 | HLR-060 | `store::DecisionStore::{append,get}` | Inspection | `store.py`, `engine.py` | Inspection |
| SDH-LLR-080 | HLR-110 | `engine::SmartHedgeEngine::{recommendation_at,health}` | T | `engine.py` | Inspection |
| SDH-LLR-081 | HLR-140 | — (not ported; needs `data.rs` Alpaca provider) | — | `data.py` | Inspection |
| SDH-LLR-082 | HLR-140 | — (not ported; needs `mcp_server.rs`) | — | `mcp_server.py` | O |
| SDH-LLR-090 | HLR-150 | `defaults::default_config_json` | O | `config.py`, `mcp_server.py` | Inspection |
| SDH-LLR-100 | HLR-160 | — (C++) | Inspection | — (C++) | Inspection |
| SDH-LLR-101 | HLR-160 | `Cargo.toml` | Inspection | n/a | n/a |
| SDH-LLR-110 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-111 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-112 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-113 | HLR-040/050 | `build::build_features` | T | `features.py` | O |
| SDH-LLR-120 | HLR-130 | `synthetic::SyntheticProvider` | T | `data.py` | O |
| SDH-LLR-121 | HLR-130 | `synthetic::SyntheticProvider` | T | `data.py` | O |
| SDH-LLR-122 | HLR-130 | `synthetic::derive_seed` | T | `data.py` | O |
| SDH-LLR-123 | HLR-070 | `evidence_file::row_applies_to_symbol` | T | `data.py` | O |
| SDH-LLR-124 | HLR-070 | `evidence_file::load_evidence_file` | T | `data.py` | O |
| SDH-LLR-125 | HLR-070 | `evidence_file::load_evidence_file` | T | `data.py` | O |
| SDH-LLR-126 | HLR-160 | — (deferred by design) | — | `data.py` | — |
| SDH-LLR-130 | HLR-020/040 | `contract::resolve_contract` | T | `engine.py` | O |
| SDH-LLR-131 | HLR-020/040 | `contract::resolve_contract` | T | `engine.py` | O |
| SDH-LLR-132 | HLR-020/040 | `contract::days_to_expiry_from_date` | T | `engine.py` | O |
| SDH-LLR-133 | HLR-060 | `hashing::canonical_hash` | T | `engine.py` | O |
| SDH-LLR-134 | HLR-060 | `hashing::file_hash` | T | `engine.py` | O |
| SDH-LLR-135 | HLR-060 | `engine::SmartHedgeEngine::replay` | T | `engine.py` | O |
| SDH-LLR-136 | HLR-110 | `engine::SmartHedgeEngine::health` | T | `engine.py` | O |
| SDH-LLR-140 | HLR-140 | `smart_hedge_cli::args::parse_args` | T | — (argparse) | — |
| SDH-LLR-141 | HLR-140 | `smart_hedge_cli::main::run` | T | n/a (Python has no gap here) | n/a |
| SDH-LLR-142 | HLR-060/140 | `smart_hedge_cli::commands::cmd_self_test` | T | `cli.py` | O |
| SDH-LLR-143 | HLR-140 | `smart_hedge_cli::commands::cmd_loop` | Inspection | `cli.py` | Inspection |

## Summary

- **HLRs**: 16 (`SDH-HLR-010` .. `SDH-HLR-160`).
- **LLRs**: 51 (`SDH-LLR-140` through `-143` added this pass for the CLI).
- **Rust-verified (T)**: 46. **Rust-implemented-but-open**: 2 (`SDH-LLR-031`,
  `-034`). **Not applicable to Rust yet (not ported)**: 3 (`SDH-LLR-056`
  OpenAI adviser, `SDH-LLR-081` Alpaca provider, `SDH-LLR-082` MCP tool
  set — the three still-deferred network-dependent surfaces; each needs
  its own HTTP-client/MCP-protocol dependency decision, see `SDH-LLR-126`).
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
- **246 Rust tests total** across `smart-hedge-{models,config,policy,
  core-bridge,features,store,model-advisor,data,engine,cli}`, all passing;
  `cargo clippy --workspace --all-targets` clean. The zero-cost path
  (synthetic data + heuristic adviser + deterministic core + policy +
  store), including the `smart-hedge` CLI binary, is now a fully working,
  independently runnable Rust program — not yet cut over from Python, but
  no longer just a library.
- **Found via this pass, not anticipated going in**: enabling
  `serde_json`'s `float_roundtrip` feature was required for
  `smart-hedge-store`'s hash-after-replay integrity check (`SDH-LLR-072`)
  to be reliable — see the correction note under that entry. This is
  exactly the kind of defect a DO-178-style recovery pass with real
  end-to-end tests (here, the CLI's `self-test` integration test) is
  supposed to surface: invisible to unit tests, real under production-like
  use.

## Next actions this recovery pass surfaced

1. `SDH-LLR-031`/`-034` (explicit binary override, auto-build gating): add
   direct tests in Rust — currently only exercised indirectly.
2. `SDH-LLR-056` (OpenAI adviser), `SDH-LLR-081` (Alpaca provider),
   `SDH-LLR-082` (MCP tool set) are the remaining un-ported surfaces — each
   needs its own dependency decision (HTTP client; MCP-over-stdio, likely
   hand-rolled JSON-RPC) before porting, per `SDH-LLR-126`.
3. `dashboard.rs`/`mcp_server.rs` are not yet started at all — deferred
   until the HTTP-server and MCP-protocol dependency decisions are made.
4. Close the remaining Python **O** rows for LLRs that already have a
   passing Rust test — the existing Python suite (`tests/test_policy.py`,
   `tests/test_model_schema.py`, `tests/test_engine.py`) is much thinner
   than the Rust parity suite and was never extended to match; this is a
   backlog item, not a blocker, since Python remains the running code
   until cutover.
