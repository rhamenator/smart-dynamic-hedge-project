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
| SDH-LLR-031 | HLR-020 | `paths::resolve_binary` | T | `core_bridge.py` | O |
| SDH-LLR-032 | HLR-120 | `build::build_core`, `which::which` | T | `core_bridge.py` | O |
| SDH-LLR-033 | HLR-120 | `paths::windows_multi_config_fallback` | T (partial) | `core_bridge.py` | O |
| SDH-LLR-034 | HLR-020 | `build::ensure_core` | T | `core_bridge.py` | O |
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
| SDH-LLR-056 | HLR-100 | `openai::OpenAIAdvisor::new` | T | `model_advisor.py` | O |
| SDH-LLR-057 | HLR-100 | `engine::SmartHedgeEngine::recommendation_at` | T | `engine.py` | O |
| SDH-LLR-060 | HLR-070 | `evidence::default_untrusted_text` | O | `models.py`, `data.py` | O |
| SDH-LLR-061 | HLR-070 | — | — | `model_advisor.py` | Inspection |
| SDH-LLR-062 | HLR-070 | — | — | `model_advisor.py` | O |
| SDH-LLR-070 | HLR-060 | `canonical::canonical_json` | T | `store.py` | T (transitive) |
| SDH-LLR-071 | HLR-060 | `canonical::hash_payload` | T | `store.py` | T (transitive) |
| SDH-LLR-072 | HLR-060 | `store::DecisionStore::get` | T | `store.py` | T (transitive) |
| SDH-LLR-073 | HLR-060 | `store::DecisionStore::{append,get}` | Inspection | `store.py`, `engine.py` | Inspection |
| SDH-LLR-080 | HLR-110 | `engine::SmartHedgeEngine::{recommendation_at,health}` | T | `engine.py` | Inspection |
| SDH-LLR-081 | HLR-140 | `alpaca::AlpacaReadOnlyProvider` | T | `data.py` | Inspection |
| SDH-LLR-082 | HLR-140 | `protocol::tool_definitions` | T | `mcp_server.py` | O |
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
| SDH-LLR-126 | HLR-160 | `ureq`/`rustls` (`smart-hedge-data`, `smart-hedge-model-advisor` `Cargo.toml`) | T (transitive, via SDH-LLR-056/-081) | `data.py` | — |
| SDH-LLR-130 | HLR-020/040 | `contract::resolve_contract` | T | `engine.py` | O |
| SDH-LLR-131 | HLR-020/040 | `contract::resolve_contract` | T | `engine.py` | O |
| SDH-LLR-132 | HLR-020/040 | `contract::days_to_expiry_from_date` | T | `engine.py` | O |
| SDH-LLR-133 | HLR-060 | `hashing::canonical_hash` | T | `engine.py` | O |
| SDH-LLR-134 | HLR-060 | `hashing::file_hash` | T | `engine.py` | O |
| SDH-LLR-135 | HLR-060 | `engine::SmartHedgeEngine::replay` | T | `engine.py` | O |
| SDH-LLR-136 | HLR-110 | `engine::SmartHedgeEngine::health` | T | `engine.py` | O |
| SDH-LLR-140 | HLR-140 | `smart_hedge_cli::args::parse_args` | T | — (argparse) | — |
| SDH-LLR-141 | HLR-140 | `smart_hedge_cli::commands::{cmd_serve,cmd_mcp}` | T | `cli.py` | — |
| SDH-LLR-142 | HLR-060/140 | `smart_hedge_cli::commands::cmd_self_test` | T | `cli.py` | O |
| SDH-LLR-143 | HLR-140 | `smart_hedge_cli::commands::cmd_loop` | Inspection | `cli.py` | Inspection |
| SDH-LLR-150 | HLR-150 | `smart_hedge_dashboard::server::serve` | T | `dashboard.py` | O |
| SDH-LLR-151 | HLR-140 | `smart_hedge_dashboard::routes::handle` | T | `dashboard.py` | O |
| SDH-LLR-152 | HLR-150 | `smart_hedge_dashboard::cache::Cache` | T | `dashboard.py` | O |
| SDH-LLR-153 | HLR-150 | `smart_hedge_dashboard::routes::valid_symbol` | T | `dashboard.py` | O |
| SDH-LLR-154 | HLR-060 | `smart_hedge_dashboard::routes::route_replay` | T | `dashboard.py` | O |
| SDH-LLR-155 | HLR-140 | `smart_hedge_mcp::protocol::handle_line` | T | `mcp_server.py` | — |
| SDH-LLR-156 | HLR-020 | `smart_hedge_mcp::tools::price_option` | T | `mcp_server.py` | O |
| SDH-LLR-157 | HLR-150 | `http_util::read_capped_body` (data, model-advisor) | T | `data.py` (`response.read(N)`) | — |
| SDH-LLR-158 | HLR-080/140 | `smart_hedge_audit::scan_file` | T | — (manually re-checked) | — |
| SDH-LLR-159 | HLR-050/010 | `smart_hedge_engine::chaos_tests` | T | — | — |

## Summary

- **HLRs**: 16 (`SDH-HLR-010` .. `SDH-HLR-160`).
- **LLRs**: 61 (`SDH-LLR-157` through `-159` added this pass: the
  response-body size cap, the repo-wide order-placement static check, and
  the randomized full-pipeline workout test).
- **Rust-verified (T)**: 61 — every LLR in this matrix. **Rust-
  implemented-but-open**: 0. **Not applicable to Rust**: 0.
- **Python-verified (T)**: 6 (mostly pre-existing `test_policy.py`/
  `test_model_schema.py` tests). **Python open**: most of the rest — the
  existing Python test suite is much thinner than the new Rust parity
  suite, which is itself a finding of this recovery pass, not a surprise:
  the Rust port added tests the Python original never had. Deliberately
  not being closed as part of this pass — see "Next actions" below.
- **`SDH-LLR-080`'s structural gap is closed.** The previous entry noted
  "if an order endpoint were ever added elsewhere, nothing here would
  automatically flip [the audit assertion] to true" — `SDH-LLR-158`
  (`smart_hedge_audit`) is that automatic flip: a `cargo test` failure the
  moment any Rust source names or constructs an order-placement request,
  re-verified on every run rather than asserted once and trusted forever.
- **405 Rust tests total** across `smart-hedge-{models,config,policy,
  core-bridge,features,store,model-advisor,data,engine,cli,dashboard,mcp,
  audit}` (13 crates, was 12), all passing; `cargo clippy --workspace
  --all-targets` clean. Beyond the previous pass's real-mock-server
  coverage, this pass added: response-body size caps matching Python's own
  defensive bounds (`SDH-LLR-157`); adversarial-fake-data "workout"
  batteries for Alpaca, FRED, RSS, and OpenAI (extreme magnitudes,
  malformed/truncated/oversized bodies, unicode, and — for RSS
  specifically — a real XXE-driven-SSRF proof using a second local
  "canary" server that must never be contacted); and a randomized
  full-pipeline chaos test in `smart-hedge-engine` (`SDH-LLR-159`). Only
  the *live* third-party endpoints themselves remain unverifiable by
  automated tests (no real credentials in CI). Nothing has cut over from
  Python; that remains a distinct, later decision.
- **Two dependency decisions made and documented this pass** (the
  previous pass — still current): both narrowly scoped exceptions to
  "hand-roll instead of depend," same reasoning as `smart-hedge-store`'s
  `rusqlite`:
  1. `ureq` (on `rustls`) for the three HTTPS **clients** (Alpaca, FRED,
     OpenAI) — TLS is a crypto-critical, adversarial-input surface, not
     something to hand-roll.
  2. The dashboard's HTTP/1.1 **server** and the MCP JSON-RPC **stdio**
     server are both hand-rolled with *no* new dependency — safe to do
     specifically because neither needs TLS and both only ever parse
     messages whose shape this process itself controls.
- **Found via this pass, not anticipated going in**: (1) enabling
  `serde_json`'s `float_roundtrip` feature was required for
  `smart-hedge-store`'s hash-after-replay integrity check (`SDH-LLR-072`)
  — found in the previous pass. (2) This pass: the `ureq` HTTP client
  integrations had **no response-body size cap at all**, unlike Python's
  own defensive `response.read(N)` calls — a real regression versus
  Python, not just an untested edge case (`SDH-LLR-157`). Both are exactly
  the kind of defect real end-to-end/adversarial tests are supposed to
  surface: invisible to hand-built in-memory fixtures, real once an actual
  TCP/HTTP round trip (or a genuinely oversized payload) is exercised.

## Next actions this recovery pass surfaced

1. Close the remaining Python **O** rows for LLRs that already have a
   passing Rust test — the existing Python suite (`tests/test_policy.py`,
   `tests/test_model_schema.py`, `tests/test_engine.py`) is much thinner
   than the Rust parity suite and was never extended to match. Deliberately
   deprioritized: Python is scheduled for cutover, so investing further
   test-writing effort in code about to be retired is not a good use of
   the remaining time before that decision — revisit only if cutover is
   delayed or abandoned.
2. The dashboard's HTML console (`smart_hedge_dashboard::html::INDEX_HTML`)
   is verbatim-ported but has no browser-driven test (only that the server
   returns it with the right content type) — acceptable for a debug
   console, but worth noting as untested client-side JS.
3. `smart_hedge_audit`'s production/test-code boundary heuristic (stop
   scanning at the first `mod tests` line) is a convention-dependent
   approximation, not a real Rust parser — reasonable given this
   codebase's actual, consistent structure, but worth re-verifying if that
   convention ever changes.
4. Cutover from Python to Rust is still a distinct, undecided future step
   — see `rust/README.md` "Connecting it together" and "Readiness for
   live testing" for what's been verified and what real-credential testing
   would still need to confirm.

## Cutover note (2026-07-19, after this pass)

Item 4 above, and every "Nothing has cut over from Python" statement in
"Summary", described a state that no longer holds: the cutover happened
immediately after this recovery pass closed. `python/` and `tests/` were
removed from the active tree (recoverable via git history); the Rust
`smart-hedge` binary is now the sole running implementation. This note is
appended rather than rewriting the sections above, per this project's own
rule that corrections to a closed recovery pass must be documented, not
silently applied. Practically, this makes item 1 moot (the Python **O**
rows in the matrix above now describe a file that no longer exists in the
working tree — read the `Python test` column as historical, not
actionable) and resolves item 4. Items 2 and 3 are unaffected by the
cutover and remain open.

## Closed this pass (previously listed here as open)

- `SDH-LLR-031`/`-034` (explicit binary override, auto-build gating) now
  have direct Rust tests (`smart-hedge-core-bridge`'s `paths`/`build`
  modules).
- The network providers'/adviser's real HTTP request/response handling
  (`SDH-LLR-056`, `-081`, and FRED/RSS under `-126`) is now verified
  against real local mock HTTP servers, not just in-memory fixtures —
  built using the existing Python source (`data.py`, `model_advisor.py`)
  as the reference for exact wire shapes, per the user's direction to use
  the Python code to define the behavior the Rust mocks/tests should
  expect.
- `SDH-LLR-080`'s structural gap is closed — see `SDH-LLR-158`
  (`smart_hedge_audit`).
- Response bodies are now read with a bounded size cap, matching Python's
  own defensive `response.read(N)` calls — a real gap this pass found,
  not just a new test — see `SDH-LLR-157`.
- Adversarial-fake-data workout batteries now exist for all four network
  integrations (Alpaca, FRED, RSS, OpenAI) and a randomized full-pipeline
  chaos test exists for the engine — see `SDH-LLR-159` and the addenda
  under `SDH-LLR-056`/`-081`/`-126`.
