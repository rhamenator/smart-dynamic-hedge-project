# Low-level requirements — `smart-dynamic-hedge`

Each entry traces to a parent HLR in `HLR.md`. See `README.md` for scope
and the shared methodology in `market-system-contracts`'s
`docs/REQUIREMENTS_METHODOLOGY.md`. `Implementation`/`Verifying tests` list
Python and Rust separately where both exist; `not yet ported` marks
Python-only behavior.

## Policy gate blockers (traces to SDH-HLR-040, SDH-HLR-050)

### SDH-LLR-001 — LIVE_MODE_FORBIDDEN blocker
Statement: The policy gate shall append `LIVE_MODE_FORBIDDEN` whenever
`policy.paper_only` is false or `mode` is not `"paper"`, independent of
the config-load-time hard stop (`SDH-LLR-020`/`-021`).
Source: `python/smart_hedge/policy.py:33-35`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `smart_hedge_policy::evaluate::evaluate_policy`.
Verifying tests: Rust — implied by config validation tests; no dedicated
policy-level test in either language (config load already prevents
reaching this state in practice). Status: **Open** — add a direct test
that bypasses `load_config` to construct an invalid `Config` and confirm
`evaluate_policy` still blocks it, in both languages.

### SDH-LLR-002 — INVALID_QUOTE blocker
Statement: The policy gate shall append `INVALID_QUOTE` when the quote
midpoint is non-finite or ≤ 0.
Source: `python/smart_hedge/policy.py:38-39`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::invalid_quote_blocks_when_bid_ask_and_last_are_all_nonpositive`.
Python: not directly tested (no equivalent case in `test_policy.py`). Status: **Open** (Python).

### SDH-LLR-003 — STALE_QUOTE blocker
Statement: The policy gate shall append `STALE_QUOTE` when the quote's
age (now minus its parsed timestamp, floored at zero, or infinite if
unparseable) exceeds `policy.max_quote_age_seconds`.
Source: `python/smart_hedge/policy.py:41-47`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`/`_parse_time`; Rust `evaluate::evaluate_policy`/`TimestampUtc::parse_flexible`.
Verifying tests: Python `test_stale_quote_blocks_preview`; Rust `parity_tests::stale_quote_blocks_preview`.

### SDH-LLR-004 — SPREAD_TOO_WIDE blocker
Statement: The policy gate shall append `SPREAD_TOO_WIDE` when the quote
spread (bps) is non-finite or exceeds `policy.max_spread_bps`.
Source: `python/smart_hedge/policy.py:49-52`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::wide_spread_is_blocked`. Python: **Open** (no direct test).

### SDH-LLR-005 — DATA_QUALITY_TOO_LOW blocker
Statement: The policy gate shall append `DATA_QUALITY_TOO_LOW` when
`features.data_quality` is below `policy.min_data_quality`.
Source: `python/smart_hedge/policy.py:54-56`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::low_data_quality_is_blocked`. Python: **Open**.

### SDH-LLR-006 — MODEL_CITED_UNKNOWN_EVIDENCE blocker
Statement: The policy gate shall append `MODEL_CITED_UNKNOWN_EVIDENCE`
when any evidence ID the model cites is not a member of the evidence IDs
supplied in `features.evidence_ids`.
Traces-to: also SDH-HLR-090.
Source: `python/smart_hedge/policy.py:61-64`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Python `test_unknown_model_citation_is_blocked`; Rust `parity_tests::unknown_model_citation_is_blocked`.

### SDH-LLR-007 — Band-multiplier clamp gated by confidence
Statement: When `assessment.confidence >= policy.min_model_confidence_for_band_change`,
the applied band multiplier shall be `assessment.band_multiplier` clamped
to `[policy.min_band_multiplier, policy.max_band_multiplier]`; otherwise
it shall be exactly `1.0` and a `model_confidence_too_low_for_band_change`
warning shall be added.
Source: `python/smart_hedge/policy.py:66-73`.
Rationale: the clamp must not panic even if a malformed config has
`min_band_multiplier > max_band_multiplier` — see Rust
`clamp_like_python`, which deliberately avoids `f64::clamp`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy::clamp_like_python`.
Verifying tests: Python `test_model_can_only_change_band_not_target`,
`test_low_confidence_cannot_change_band`; Rust `parity_tests::model_can_only_change_band_not_target`,
`parity_tests::low_confidence_cannot_change_band`.

### SDH-LLR-008 — NONFINITE_CORE_VALUE blocker
Statement: The policy gate shall append `NONFINITE_CORE_VALUE` when any
of target shares, current shares, the raw trade (their difference), or
the base no-trade band is non-finite — including values that only become
non-finite through arithmetic (e.g. subtracting two large finite values).
Source: `python/smart_hedge/policy.py:75-86`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::nonfinite_core_value_is_blocked`
(exercises the arithmetic-overflow case specifically, using `f64::MAX`
and `-f64::MAX`). Python: **Open**.

### SDH-LLR-009 — Effective band and inside-band determination
Statement: The effective no-trade band shall be
`max(0, base_band * applied_multiplier)`; the raw trade shall be
considered "inside the band" iff its absolute value is `<=` the effective
band, in which case the preview trade shall be exactly `0`.
Source: `python/smart_hedge/policy.py:87-89`.
Verification: Test. Status: Implemented.
Implementation/tests: same as SDH-LLR-007/-008.

### SDH-LLR-010 — Round-half-to-even for whole-share-only mode
Statement: When `policy.allow_fractional_shares` is false, the preview
trade shall be rounded using round-half-to-even (matching Python's
built-in `round()`), not round-half-away-from-zero.
Source: `python/smart_hedge/policy.py:91-92` (Python `round()` semantics).
Rationale: `0.5`, `1.5`, `2.5`, ... are exactly representable in binary
floating point, so this boundary is reached in practice, not just in
theory; Rust's `f64::round()` rounds the other way and would silently
diverge from Python without a dedicated implementation.
Verification: Test. Status: Implemented.
Implementation: Python built-in `round()`; Rust `smart_hedge_policy::rounding::round_half_to_even`.
Verifying tests: Rust `rounding::tests::matches_python_round_for_known_half_boundary_cases`,
`parity_tests::fractional_shares_disallowed_rounds_half_to_even`. Python:
**Open** (no dedicated test of this behavior; it's exercised implicitly
whenever `allow_fractional_shares: false` is configured, which no
existing Python test does).

### SDH-LLR-011 — TRADE_SHARE_LIMIT blocker
Statement: The policy gate shall append `TRADE_SHARE_LIMIT` when the
absolute preview trade exceeds `policy.max_abs_trade_shares`.
Source: `python/smart_hedge/policy.py:94-96`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::trade_share_limit_blocks_an_oversized_preview`. Python: **Open**.

### SDH-LLR-012 — PREVIEW_NOTIONAL_LIMIT blocker
Statement: The policy gate shall append `PREVIEW_NOTIONAL_LIMIT` when
`abs(preview_trade) * midpoint` exceeds `policy.max_preview_notional`.
Source: `python/smart_hedge/policy.py:98-101`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::preview_notional_limit_blocks_when_configured_tightly`. Python: **Open**.

### SDH-LLR-013 — MARKET_NOT_OPEN blocker
Statement: The policy gate shall append `MARKET_NOT_OPEN` when
`policy.require_market_open_for_preview` is true, the quote's market
state is not `"open"`, and the trade is outside the effective band (a
hold-inside-band decision is never blocked by market state).
Source: `python/smart_hedge/policy.py:103-105`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::market_not_open_blocks_an_out_of_band_preview`,
`parity_tests::market_closed_does_not_block_a_hold_inside_band_decision`. Python: **Open**.

### SDH-LLR-014 — Blocked decision zeroes preview trade and notional
Statement: Whenever any blocker is present, `action` shall be
`"observe_blocked"` and both the preview trade and its notional shall be
forced to exactly `0`, regardless of what they were computed as.
Source: `python/smart_hedge/policy.py:107-115`.
Verification: Test. Status: Implemented.
Implementation: Python `evaluate_policy`; Rust `evaluate::evaluate_policy`.
Verifying tests: Rust `parity_tests::blocked_decision_zeroes_preview_trade_and_notional`. Python: **Open**.

### SDH-LLR-015 — `live_execution_allowed` is unconditionally false
Statement: `PolicyDecision.live_execution_allowed` shall be `false` for
every possible input to `evaluate_policy` — there is no code path that
sets it otherwise.
Traces-to: also SDH-HLR-010.
Source: `python/smart_hedge/policy.py:120`.
Verification: Test, Inspection. Status: Implemented.
Implementation: Python `evaluate_policy` (hardcoded `False`); Rust `evaluate::evaluate_policy` (hardcoded `false`).
Verifying tests: Rust `parity_tests::live_execution_is_never_allowed_regardless_of_inputs`. Python: **Open**.

## Configuration (traces to SDH-HLR-030)

### SDH-LLR-020 — Reject non-paper mode at load time
Statement: `load_config` shall raise/return an error when the merged
config's `mode` (case-insensitive) is not `"paper"`.
Source: `python/smart_hedge/config.py:123-124`.
Verification: Test. Status: Implemented.
Implementation: Python `load_config`; Rust `smart_hedge_config::loader::load_config`.
Verifying tests: Rust `loader::tests::non_paper_mode_is_rejected`. Python: **Open** (no dedicated test).

### SDH-LLR-021 — Reject `policy.paper_only: false` at load time
Statement: `load_config` shall raise/return an error when the merged
config's `policy.paper_only` is false, even if `mode` is `"paper"`.
Source: `python/smart_hedge/config.py:125-126`.
Verification: Test. Status: Implemented.
Implementation: Python `load_config`; Rust `loader::load_config`.
Verifying tests: Rust `loader::tests::paper_only_false_is_rejected_even_if_mode_is_paper`. Python: **Open**.

### SDH-LLR-022 — Deep-merge semantics
Statement: Merging a user config onto defaults shall recurse into nested
objects present in both, and shall wholesale-replace any value where
either side is not an object (including replacing an object with a
non-object or vice versa).
Source: `python/smart_hedge/config.py:82-89` (`_deep_merge`).
Verification: Test. Status: Implemented.
Implementation: Python `_deep_merge`; Rust `smart_hedge_config::merge::deep_merge`.
Verifying tests: Rust `merge::tests::*` (5 cases). Python: **Open** (no
dedicated unit test of `_deep_merge` in isolation).

### SDH-LLR-023 — Environment-variable overrides
Statement: When set, `SMART_HEDGE_PROVIDER`, `SMART_HEDGE_MODEL_KIND`,
`OPENAI_MODEL`, `SMART_HEDGE_CORE`, and `SMART_HEDGE_DB` shall each
override exactly one corresponding config field
(`provider.kind`/`model.kind`/`model.name`/`core.binary`/`storage.sqlite_path`)
after the config-file merge, taking precedence over both defaults and the
config file.
Source: `python/smart_hedge/config.py:111-120`.
Verification: Test. Status: Implemented.
Implementation: Python `load_config`; Rust `loader::load_config`/`env_overrides::EnvOverrides`.
Verifying tests: Rust `loader::tests::env_overrides_apply_on_top_of_defaults`. Python: **Open**.

### SDH-LLR-024 — Config-relative path resolution
Statement: A configured relative path shall resolve against the
directory containing the config file (or the project root if no config
file was given), with `~`/`~/...` expanded first; an absolute path shall
be returned unchanged.
Source: `python/smart_hedge/config.py:130-135` (`resolve_project_path`).
Verification: Test. Status: Implemented.
Implementation: Python `resolve_project_path`; Rust `smart_hedge_config::paths::resolve_project_path`.
Verifying tests: Rust `paths::tests::*` (4 cases). Python: **Open**.

### SDH-LLR-025 — New contract symbols tolerate partial fields
Statement: A contract symbol not already present in the default config
(e.g. a user-added `"QQQ"`) shall deserialize successfully from only the
fields the user specifies, with `option_type`, `exercise_style`,
`days_to_expiry`, `expiry`, `contracts`, `multiplier`, `current_shares`,
`rate`, `dividend_yield`, and `base_no_trade_band_shares` defaulted; only
`strike` and `implied_volatility` remain required.
Source: `python/smart_hedge/core_bridge.py:90-112` (`contract.get(key,
default)` calls); `python/smart_hedge/engine.py:40-46` (`_days_to_expiry`,
which is what actually guarantees `days_to_expiry` is present and valid
by the time `run_core` reads it — see the correction below).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_config::types::ContractConfig`
(`#[serde(default = ...)]` per field). Python: implicit in
`core_bridge.run_core`'s per-field `.get()` calls plus `engine.py`'s
`_days_to_expiry` fallback — same net behavior, recovered from reading
the code, not from a written-down rule.
Verifying tests: Rust `loader::tests::a_brand_new_contract_symbol_gets_only_the_fields_it_specifies_plus_defaults`,
`loader::tests::a_contract_symbol_with_only_an_expiry_date_needs_no_days_to_expiry`.
Python: **Open**.

**Correction (2026-07-19, while porting `engine.py`):** an earlier version
of this entry, and of `ContractConfig`, made `days_to_expiry` required —
reasoned only from `core_bridge.py`'s raw `contract["days_to_expiry"]`
indexing, without accounting for `engine.py`'s `_days_to_expiry` helper,
which runs first and defaults it to `30.0` (or computes it from `expiry`)
whenever the config omits it. A config specifying only `expiry` is valid
Python input that the earlier, stricter Rust schema would have rejected
at load time. This is exactly the kind of cross-function interaction
that's easy to miss porting module-by-module without the whole picture —
recorded here rather than silently fixed, per the methodology's requirement
that corrections be traceable, not just applied.

## C++ core invocation and multiplatform behavior (traces to SDH-HLR-020, SDH-HLR-120)

### SDH-LLR-030 — Platform-correct default binary name
Statement: The default core binary path shall be
`{project_root}/build/smart_dynamic_hedge.exe` on Windows and
`{project_root}/build/smart_dynamic_hedge` elsewhere.
Source: `python/smart_hedge/core_bridge.py:17-19`.
Verification: Test. Status: Implemented.
Implementation: Python `default_binary_path`; Rust `smart_hedge_core_bridge::paths::default_binary_path`.
Verifying tests: Rust `paths::tests::default_binary_path_has_platform_correct_suffix`. Python: **Open**.

### SDH-LLR-031 — Explicit binary path takes precedence
Statement: When `core.binary` is a non-empty string, it shall be resolved
(config-relative if not absolute, per SDH-LLR-024) and used instead of
the platform default.
Source: `python/smart_hedge/core_bridge.py:22-24`.
Verification: Test. Status: Implemented.
Implementation: Python `resolve_binary`; Rust `paths::resolve_binary`.
Verifying tests: Rust — covered indirectly via `loader::tests::env_overrides_apply_on_top_of_defaults`
(sets `core.binary`); no direct `resolve_binary` unit test in either
language. Status: **Open** (add a direct test in both).

### SDH-LLR-032 — Toolchain discovery order: cmake, then g++, then clang++
Statement: Building the core shall prefer CMake if found on `PATH`; if
not found, it shall fall back to `g++`, then `clang++`; if none is found,
building shall fail with a clear error rather than a cryptic subprocess
failure.
Source: `python/smart_hedge/core_bridge.py:31-46`.
Verification: Test. Status: Implemented.
Implementation: Python `build_core`; Rust `smart_hedge_core_bridge::build::build_core`/`which::which`.
Verifying tests: Rust `which::tests::*`; `run::tests::run_core_against_the_real_cpp_binary_produces_a_parseable_response`
exercises the real discovery+build path end to end when a toolchain is
present. Python: **Open** (no test exercises the fallback order itself,
only that building works at all via `make`/CI).

### SDH-LLR-033 — Windows multi-config-generator fallback path
Statement: On Windows, if the resolved binary doesn't exist after a
build, the system shall additionally check
`{project_root}/build/Release/smart_dynamic_hedge.exe` before reporting
failure.
Source: `python/smart_hedge/core_bridge.py:64-68`.
Verification: Test. Status: Implemented.
Implementation: Python `build_core`; Rust `build::build_core`/`paths::windows_multi_config_fallback`.
Verifying tests: Rust `paths::tests::windows_fallback_path_is_under_release_subdirectory`
(logic-only; not exercised against a real Visual Studio/Ninja
Multi-Config build in CI on either language, since this environment
builds via CMake single-config or direct g++/clang++). Status:
**Partial** — path logic is tested, the actual multi-config-generator
integration is not.

### SDH-LLR-034 — Auto-build is configuration-gated
Statement: `ensure_core` shall build the binary automatically only when
`core.auto_build` is true; when false and the binary is missing, it shall
report an error rather than building unexpectedly.
Source: `python/smart_hedge/core_bridge.py:74-80`.
Verification: Test. Status: Implemented.
Implementation: Python `ensure_core`; Rust `build::ensure_core`.
Verifying tests: **Open** in both languages (no test exercises `auto_build: false` directly).

### SDH-LLR-035 — Exact core CLI argument set
Statement: Invoking the core shall pass exactly these flags, in this
form: `--spot --strike --rate --dividend-yield --vol --days --type
--style --contracts --multiplier --current-shares --tree-steps
--no-trade-band --json`, with contract/config values converted to their
natural numeric/string form (no locale-specific formatting).
Source: `python/smart_hedge/core_bridge.py:85-114`.
Verification: Test. Status: Implemented.
Implementation: Python `run_core`; Rust `smart_hedge_core_bridge::run::run_core`.
Verifying tests: Rust `run::tests::run_core_against_the_real_cpp_binary_produces_a_parseable_response`
(end-to-end, real binary). Python: **Open** (covered only transitively by
`tests/test_engine.py`, not asserted against the exact argument list).

### SDH-LLR-036 — Core invocation timeout
Statement: Invoking the core shall fail with a timeout error (not hang
indefinitely) after `core.timeout_seconds`, killing the child process.
Source: `python/smart_hedge/core_bridge.py:115-125` (`subprocess.run(...,
timeout=...)`).
Rationale: `std::process` has no built-in timeout; the Rust port
hand-rolls one (`run_command_with_timeout`) rather than adding a
dependency for it.
Verification: Test. Status: Implemented.
Implementation: Python (`subprocess.run` `timeout=`); Rust
`smart_hedge_core_bridge::run_with_timeout::run_command_with_timeout`.
Verifying tests: Rust `run_with_timeout::tests::times_out_a_command_that_sleeps_too_long`. Python: **Open**.

### SDH-LLR-037 — Core response shape validation
Statement: A core response missing required top-level structure (in
particular `hedge`/`greeks`) shall be treated as an error, not silently
passed through with missing fields.
Source: `python/smart_hedge/core_bridge.py:132-133` (explicit key check).
Rationale: the Rust port subsumes this into `CoreResponse`
deserialization — a structurally incomplete or mistyped response fails to
parse at all, at the point `run_core` reads the subprocess's stdout,
rather than being checked field-by-field by every later consumer. See
`rust/README.md` "Known, documented behavioral differences".
Verification: Test. Status: Implemented.
Implementation: Python `run_core` (explicit `"hedge" not in payload`
check); Rust `smart_hedge_models::core_response::CoreResponse` (typed
deserialization).
Verifying tests: Rust `core_response::tests::missing_required_field_fails_to_deserialize`. Python: **Open**.

### SDH-LLR-038 — Non-finite C++ output encodes as JSON `null`
Statement: The C++ core's `--json` output shall encode any non-finite
(`NaN`/`Infinity`) numeric field as JSON `null`, never as a bare `NaN`
literal (which is not valid JSON) or a silently-substituted number.
Source: `cpp/smart_dynamic_hedge.cpp` (`json_number`, lines ~229-236).
Verification: Test. Status: Implemented.
Implementation: C++ `json_number`.
Verifying tests: Rust `core_response::tests::null_for_a_non_finite_field_fails_to_deserialize`
(verifies the *consumer* side: a `null` in that position fails to
deserialize into `f64` rather than becoming `0.0`). **Open**: no test
directly forces the C++ core itself to emit a non-finite value and
inspects the raw JSON (would require a contrived degenerate input, e.g.
extreme volatility/expiry, not currently in the C++ self-test suite).

## Model adviser and schema (traces to SDH-HLR-080, SDH-HLR-090, SDH-HLR-100, SDH-HLR-070)

### SDH-LLR-050 — Exact required-key-set enforcement
Statement: `validate_assessment_payload` shall reject any payload whose
key set is not exactly `{regime, confidence, hedge_urgency,
band_multiplier, summary, evidence_ids, risks, scenario_spot_shocks,
data_requests}` — both missing and extra keys are rejected.
Source: `python/smart_hedge/model_advisor.py:99-103`.
Verification: Test. Status: Implemented.
Implementation: Python `validate_assessment_payload`; Rust
`smart_hedge_model_advisor::schema::validate_assessment_payload`.
Verifying tests: `tests/test_model_schema.py::test_extra_trade_field_rejected`;
Rust `schema::tests::missing_field_is_rejected`, `schema::tests::extra_field_is_rejected`.

### SDH-LLR-051 — Regime enum enforcement
Statement: `regime` shall be one of exactly seven values: `calm`,
`trend_up`, `trend_down`, `volatile`, `jump_risk`, `illiquid`,
`uncertain`.
Source: `python/smart_hedge/model_advisor.py:11-19,104-106`.
Verification: Test. Status: Implemented.
Implementation: Python `ALLOWED_REGIMES`, `validate_assessment_payload`;
Rust `smart_hedge_model_advisor::schema::ALLOWED_REGIMES`.
Verifying tests: Rust `schema::tests::invalid_regime_is_rejected`,
`schema::tests::regime_as_non_string_is_rejected`. Python: **Open** —
`test_model_schema.py` doesn't test an invalid regime directly.

### SDH-LLR-052 — Numeric field bounds
Statement: `confidence`/`hedge_urgency` shall be finite numbers in
`[0, 1]`; `band_multiplier` shall be a finite number in `[0.5, 3.0]`;
values outside these ranges (or non-numeric, or boolean) shall be
rejected.
Source: `python/smart_hedge/model_advisor.py:76-82,118-120`.
Verification: Test. Status: Implemented.
Implementation: Python `_finite_number`; Rust `schema::finite_number`.
Verifying tests: `test_model_schema.py::test_out_of_range_band_rejected`;
Rust `schema::tests::out_of_range_band_multiplier_is_rejected`,
`schema::tests::boolean_confidence_is_rejected_as_non_numeric`,
`schema::tests::nonfinite_scenario_shock_is_rejected`.

### SDH-LLR-053 — Scenario-shock bounds
Statement: `scenario_spot_shocks` shall contain 1 to 7 finite numbers,
each in `[-0.30, 0.30]`.
Source: `python/smart_hedge/model_advisor.py:107-110`.
Verification: Test. Status: Implemented.
Implementation: Rust `schema::validate_assessment_payload`.
Verifying tests: Rust `schema::tests::zero_scenario_shocks_is_rejected`,
`schema::tests::too_many_scenario_shocks_is_rejected`,
`schema::tests::nonfinite_scenario_shock_is_rejected`. Python: **Open** —
no dedicated test for the count or per-item bound.

### SDH-LLR-054 — Bounded string-list fields
Statement: `evidence_ids`, `risks`, and `data_requests` shall each be
lists capped at 8 items, with each item truncated to a maximum length
(160/240/240 characters respectively) rather than rejected for being too
long.
Source: `python/smart_hedge/model_advisor.py:85-93`.
Verification: Test. Status: Implemented.
Implementation: Rust `schema::string_list`.
Verifying tests: Rust `schema::tests::too_many_list_items_is_rejected`,
`schema::tests::string_list_items_are_truncated_not_rejected_when_too_long`,
`schema::tests::non_string_list_item_is_rejected`. Python: **Open** — no
test exercises the truncation or the item-count cap.

### SDH-LLR-055 — Heuristic adviser makes no network call
Statement: `HeuristicAdvisor.assess` shall be a pure function of its
inputs (snapshot, features, core, contract) with no network access, no
file I/O beyond what those inputs already contain, and shall always
return a schema-valid `ModelAssessment`.
Source: `python/smart_hedge/model_advisor.py:130-207`.
Verification: Test, Inspection. Status: Implemented.
Implementation: Python `HeuristicAdvisor`; Rust
`smart_hedge_model_advisor::heuristic::HeuristicAdvisor`.
Verifying tests: Rust `heuristic::tests::*` (10 cases, isolate
`HeuristicAdvisor` directly with hand-built fixtures — regime
classification, gamma-driven urgency, confidence bounds, evidence-ID
capping, and the falsy-`or` volatility-fallback quirk (`SDH-LLR-055`
correction below)). Python: transitively via `test_engine.py`; **Open** —
no direct unit test isolates `HeuristicAdvisor` from the rest of the engine.

**Correction (2026-07-19, while porting):** Python's
`values.get("ewma_volatility") or values.get("realized_volatility")`
treats an *exact* `0.0` ewma value as falsy and falls through to
realized volatility, not just a missing/`None` value — an easy detail to
miss porting literally (`Option::or` alone does not replicate `or`'s
falsy-any-zero semantics). Replicated in Rust with an explicit
`.filter(|&v| v != 0.0)`; regression test:
`heuristic::tests::zero_ewma_volatility_falls_back_to_realized_volatility`.

### SDH-LLR-056 — OpenAI adviser fails fast on missing configuration
Statement: Constructing `OpenAIAdvisor` shall raise immediately (not on
first use) if no usable model name is configured or if
`OPENAI_API_KEY` is not set.
Source: `python/smart_hedge/model_advisor.py:214-222`.
Verification: Test. Status: Implemented.
Implementation: Python `OpenAIAdvisor.__init__`; Rust
`smart_hedge_model_advisor::openai::OpenAIAdvisor::new` (takes credentials
as explicit parameters rather than reading `std::env` directly, so the
immediate-failure check is testable without a real API key or network —
see that module's doc comment).
Verifying tests: Rust `openai::tests::the_packaged_default_model_name_placeholder_is_rejected`,
`openai::tests::missing_api_key_is_rejected`,
`openai::tests::an_empty_configured_name_with_no_env_fallback_is_rejected`.
Python: **Open** (would require monkeypatching `openai` or running without
network).

### SDH-LLR-057 — Fallback-to-heuristic is configuration-gated
Statement: When the active adviser raises, the engine shall fall back to
`HeuristicAdvisor` and record the fallback reason only when
`model.fallback_to_heuristic` is true; otherwise the original exception
shall propagate.
Source: `python/smart_hedge/engine.py:99-107`.
Verification: Test. Status: Implemented.
Implementation: Python `SmartHedgeEngine.recommendation`; Rust
`smart_hedge_engine::engine::SmartHedgeEngine::recommendation_at`.
Verifying tests: Rust `integration_tests::adviser_failure_falls_back_to_heuristic_when_enabled`,
`integration_tests::adviser_failure_propagates_when_fallback_disabled`
(both use a deliberately-failing `Advisor` stub — `AlwaysFailsAdvisor` —
injected via `SmartHedgeEngine::with_components`). Python: **Open** (no
test exercises either branch).

### SDH-LLR-060 — Evidence defaults to untrusted
Statement: `EvidenceItem.untrusted_text` shall default to `true`; only
evidence a source explicitly marks as trusted (e.g. the synthetic
provider's internally generated items, or a config-supplied `"untrusted_text":
false`) shall be treated as non-adversarial.
Source: `python/smart_hedge/models.py:56` (dataclass default);
`python/smart_hedge/data.py` (synthetic provider sets `untrusted_text=False`
on its own generated items; RSS items are always `untrusted_text=True`).
Verification: Test. Status: Implemented.
Implementation: Python `EvidenceItem` dataclass default; Rust
`smart_hedge_models::evidence::default_untrusted_text`.
Verifying tests: **Open** in both — no test asserts the default value
directly (Rust's `EvidenceItem` isn't exercised by any test yet since
nothing constructs one from partial JSON in the current test suite).

### SDH-LLR-061 — Model instructions explicitly flag evidence as untrusted
Statement: The instructions sent to the OpenAI adviser shall explicitly
state that evidence text is untrusted, may contain prompt injection, and
must never be followed as instructions.
Source: `python/smart_hedge/model_advisor.py:303-310`.
Verification: Inspection. Status: Implemented (Python only).

### SDH-LLR-062 — Evidence truncation before reaching the model
Statement: The number of evidence items and the character length of each
item's text sent to the model shall be capped by
`model.max_evidence_items`/`model.max_evidence_chars`.
Source: `python/smart_hedge/model_advisor.py:236-253`.
Verification: Test. Status: Implemented (Python only). **Open**: no test
exercises the truncation boundary.

## Audit and replay (traces to SDH-HLR-060)

### SDH-LLR-070 — Canonical JSON for hashing
Statement: The content hash of a decision payload shall be computed over
a canonical JSON serialization: sorted object keys, compact separators
(no extra whitespace), non-ASCII characters preserved (not escaped).
Source: `python/smart_hedge/store.py:46-47` (`canonical_json`).
Verification: Test. Status: Implemented.
Implementation: Python `canonical_json`; Rust `smart_hedge_store::canonical::canonical_json`
(relies on `serde_json::Value::Object` being `BTreeMap`-backed — see that
module's doc comment).
Verifying tests: Rust `canonical::tests::object_keys_are_sorted_regardless_of_insertion_order`,
`canonical::tests::separators_are_compact_with_no_extra_whitespace`,
`canonical::tests::nested_objects_are_also_sorted`. Python:
`tests/test_engine.py` (transitively, via replay hash-validity assertions).

### SDH-LLR-071 — SHA-256 content hash
Statement: The content hash shall be the SHA-256 hex digest of the
canonical JSON (SDH-LLR-070) of the full decision payload.
Source: `python/smart_hedge/store.py:49-51`.
Verification: Test. Status: Implemented.
Implementation: Python `hash_payload`; Rust `smart_hedge_store::canonical::hash_payload`
(built on `smart_hedge_models::sha256`, hand-rolled and verified against
official NIST/FIPS 180-4 test vectors — see that module).
Verifying tests: Rust `canonical::tests::hash_is_a_64_character_lowercase_hex_string`,
`canonical::tests::hash_differs_for_different_payloads`.

### SDH-LLR-072 — Replay independently reverifies the hash
Statement: Reading a stored decision shall recompute its content hash
from the stored JSON and report whether it matches the stored hash,
rather than trusting the stored hash unconditionally.
Source: `python/smart_hedge/store.py:75-87` (`get`).
Verification: Test. Status: Implemented.
Implementation: Python `DecisionStore.get`; Rust `smart_hedge_store::store::DecisionStore::get`.
Verifying tests: Rust `integration_tests::tampered_payload_is_detected_on_replay`
(directly corrupts a stored row via a raw SQL `UPDATE`, bypassing `append`
entirely, and asserts `get` reports `stored_content_hash_valid: false`).
Python: `tests/test_engine.py` (replay hash-validity assertion, per
`README.md` "SQLite records pass content-hash verification on replay").

**Correction (2026-07-19, found via `smart-hedge-cli`'s self-test
integration test):** `smart-hedge-store`'s hash-then-store,
parse-then-rehash round trip depends on `serde_json` reparsing a stored
float back to the *exact* bit pattern that produced it. Without the
`float_roundtrip` Cargo feature, `serde_json`'s float parser trades exact
round-tripping for speed, so a float like `0.9040000000000001` (not itself
the shortest round-trip representation of its bit pattern — the kind of
value ordinary floating-point arithmetic produces constantly) can reparse
to a nearby-but-different f64 and reserialize as the shorter `0.904`,
making `stored_content_hash_valid` false for a decision that was never
tampered with. Fixed by enabling `serde_json/float_roundtrip` workspace-wide
(`rust/Cargo.toml`); regression test:
`smart_hedge_store::canonical::tests::a_float_that_is_not_its_own_shortest_round_trip_still_round_trips_through_parsing`.
This was invisible to every test written before this session because none
of them stored a payload containing a float that wasn't already its own
shortest round-trip form — recorded here rather than silently patched, per
the methodology's requirement that corrections be traceable.

### SDH-LLR-073 — Stored hash lives beside, not inside, the hashed JSON
Statement: The content hash shall be stored in a separate database
column from the JSON payload, never injected into the JSON before
hashing (which would make the hash self-referential and unverifiable).
Source: `python/smart_hedge/store.py:53-73` (`append`); `engine.py:150-154`.
Verification: Inspection. Status: Implemented.
Implementation: Python `DecisionStore.append`; Rust `store::DecisionStore::append`
(the `content_hash` column is separate from `payload_json`; the hash is
computed from the payload before any hash is added to it).

## Credential and interface-surface boundaries (traces to SDH-HLR-110, SDH-HLR-140, SDH-HLR-150)

### SDH-LLR-080 — Audit record always asserts no secrets/no order endpoint
Statement: Every decision's `audit` block shall include
`secrets_sent_to_model: false` and `broker_order_endpoint_present: false`
unconditionally (not computed from a check that could pass silently if
the underlying code changed).
Source: `python/smart_hedge/engine.py:134-135`.
Rationale: Recovered as written — this is a self-asserted invariant, not
a runtime check. Its actual enforcement is structural (no code path exists
that could set it otherwise), not a validation the audit record performs
on itself. Worth flagging: if an order endpoint were ever added
elsewhere, nothing here would automatically flip this to `true` — it
relies on the author updating it, which is a real gap in a hard guarantee.
Verification: Inspection. Status: **Partial** — the assertion is present,
but nothing prevents it from silently going stale if HLR-010 were ever
violated elsewhere in the codebase.
Implementation: Python `SmartHedgeEngine.recommendation`/`health`; Rust
`smart_hedge_engine::engine::SmartHedgeEngine::recommendation_at`/`health`
(both hardcode `false` literals, same as Python — same structural caveat
applies to the Rust port).
Verifying tests: Rust `integration_tests::recommendation_and_health_report_the_synthetic_heuristic_path`
asserts both fields on a real end-to-end decision and on `health()`.

### SDH-LLR-081 — Alpaca provider is data-only
Statement: `AlpacaReadOnlyProvider` shall construct requests only against
the Alpaca market-data host and only for quote/bar GET endpoints; it
shall hold no code path capable of constructing an order-placement
request.
Source: `python/smart_hedge/data.py:123-211`.
Verification: Inspection. Status: Implemented.
Implementation: Python `AlpacaReadOnlyProvider`; Rust
`smart_hedge_data::alpaca::AlpacaReadOnlyProvider` (`get` is a private
method used only for the two documented quote/bar GET paths; there is no
POST/PUT/DELETE call anywhere in this module).
Verifying tests: Rust `alpaca::tests::*` cover the request-shaping logic
directly (`parse_bars`, `build_quote`); the "no order-capable code path"
guarantee itself is structural, verified by inspection in both languages,
the same as `SDH-LLR-080`.

### SDH-LLR-082 — MCP tool set contains no order-capable tool
Statement: The MCP server shall expose exactly `health`,
`get_market_recommendation`, `price_option`, `replay_decision`,
`list_recent_decisions`, `get_policy_snapshot` — no tool named or
equivalent to `place_order`/`submit_order`/`cancel_order`/credential
management.
Source: `python/smart_hedge/mcp_server.py:36-96`; `README.md` "MCP tools".
Verification: Test, Inspection. Status: Implemented.
Implementation: Python `create_server`; Rust
`smart_hedge_mcp::protocol::tool_definitions`.
Verifying tests: Rust `protocol::tests::tools_list_returns_exactly_the_six_expected_tools_and_no_order_tool`
(enumerates the real `tools/list` response and asserts both the exact
membership and the absence of `place_order`/`submit_order`/`cancel_order`);
`tests/cli_integration.rs::mcp_answers_initialize_and_tools_list_over_stdio`
(same assertion end-to-end over a real stdio subprocess). Python: **Open**
— no test introspects `FastMCP`'s registered tools.

### SDH-LLR-090 — Dashboard/MCP default to localhost/stdio
Statement: The dashboard shall default to host `127.0.0.1` port `8765`;
the MCP server shall default to the stdio transport.
Source: `python/smart_hedge/config.py:63` (dashboard defaults);
`python/smart_hedge/mcp_server.py:100-104` (`server.run()`, stdio).
Verification: Test, Inspection. Status: Implemented.
Implementation: Python; Rust `smart_hedge_config::defaults::default_config_json`
(dashboard section).
Verifying tests: Rust `loader::tests::loads_defaults_with_no_override_file`
(asserts `provider.kind`, not yet extended to assert dashboard
host/port — **Open**, easy addition).

## Feature extraction (traces to SDH-HLR-040, SDH-HLR-050)

### SDH-LLR-110 — Data-quality composition
Statement: `data_quality` shall be the mean of: (a) `1.0` if the quote
midpoint is positive else `0.0`, (b) `1.0` if the spread is finite else
`0.0`, (c) `min(1.0, bar_count / (long_window + 1))`, (d) `1.0 - min(1.0,
missing_count / 6.0)`, and, when any evidence exists, (e) the mean
evidence-item quality — the overall result clamped to `[0, 1]`.
Source: `python/smart_hedge/features.py:109-117`.
Verification: Test. Status: Implemented.
Implementation: Python `build_features`; Rust `smart_hedge_features::build::build_features`.
Verifying tests: Rust `integration_tests::data_quality_is_high_for_a_complete_snapshot`,
`integration_tests::data_quality_is_low_for_a_degenerate_snapshot`,
`integration_tests::evidence_quality_influences_data_quality`. Python:
transitively via `tests/test_engine.py`; no dedicated unit test of the
composition formula itself. Status: **Open** (Python).

### SDH-LLR-111 — Missing features are marked, not defaulted
Statement: `realized_volatility`, `ewma_volatility`, and the
short/long-window returns shall each be explicitly listed in `missing`
(and left absent from `values`, i.e. `None`) when insufficient bar
history exists to compute them, rather than silently substituted with
`0.0` or a placeholder.
Source: `python/smart_hedge/features.py:40-61`.
Rationale: A silently-defaulted `0.0` volatility would look like a real,
confident "no volatility" reading to a downstream consumer (the
heuristic adviser, the policy gate) instead of "unknown" — the missing
list exists precisely so nothing downstream can't tell the difference.
Verification: Test. Status: Implemented.
Implementation: Python `build_features`; Rust `build::build_features`.
Verifying tests: Rust `integration_tests::realized_volatility_is_missing_with_fewer_than_two_closes`,
`integration_tests::realized_volatility_is_present_with_enough_closes`. Python: **Open**.

### SDH-LLR-112 — Volume z-score requires 21 bars of history
Statement: `volume_zscore` shall be computed only when at least 21 bars
of volume history exist (20 prior bars plus the current one); otherwise
it shall be `None` with a `volume_zscore_unavailable` warning (not a
blocker — insufficient history for this one feature must not by itself
block a decision).
Source: `python/smart_hedge/features.py:66-73`.
Verification: Test. Status: Implemented.
Implementation: Python `build_features`; Rust `build::build_features`.
Verifying tests: Rust `integration_tests::volume_zscore_unavailable_with_fewer_than_21_bars`,
`integration_tests::volume_zscore_present_with_21_or_more_bars`. Python: **Open**.

### SDH-LLR-113 — Trend score requires a nonzero volatility floor
Statement: `trend_score` shall be computed only when `realized`
volatility exists and exceeds `1e-9`; otherwise it shall be `None`,
avoiding a division that would blow up toward infinity for a near-zero
volatility estimate.
Source: `python/smart_hedge/features.py:75-78`.
Verification: Test. Status: Implemented.
Implementation: Python `build_features`; Rust `build::build_features`.
Verifying tests: Rust `integration_tests::trend_score_is_none_when_realized_volatility_is_at_the_floor`,
`integration_tests::trend_score_is_present_with_a_real_trend_and_volatility`. Python: **Open**.

## Market data providers (traces to SDH-HLR-070, SDH-HLR-130)

### SDH-LLR-120 — Synthetic provider is always available and needs no account
Statement: The synthetic provider shall produce a complete `MarketSnapshot`
(quote, 180 bars, evidence) for any symbol using only a deterministic
pseudo-random walk seeded from the symbol and a time bucket — no network
access, no API key, no external account.
Source: `python/smart_hedge/data.py:41-121` (`SyntheticProvider`).
Verification: Test. Status: Implemented.
Implementation: Python `SyntheticProvider`; Rust
`smart_hedge_data::synthetic::SyntheticProvider`.
Verifying tests: Rust `synthetic::tests::produces_the_expected_bar_count`
and the rest of `synthetic::tests::*`.

### SDH-LLR-121 — Synthetic quote market state is always "open"
Statement: The synthetic provider's quote shall report `market_state:
"open"` unconditionally, since it is a research fixture, not a claim about
any real exchange's session state.
Source: `python/smart_hedge/data.py:90` (comment: "synthetic market is
intentionally always available").
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_data::synthetic::SyntheticProvider`.
Verifying tests: Rust `synthetic::tests::market_state_is_always_open`.

### SDH-LLR-122 — Synthetic seed is deterministic within a 5-second bucket
Statement: The synthetic provider's random seed shall be derived from the
symbol and `floor(now / 5 seconds)`, so repeated calls within the same
5-second window for the same symbol produce identical output (useful for
caching/dashboard polling), while calls in different windows differ.
Source: `python/smart_hedge/data.py:48-51`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_data::synthetic::derive_seed`.
Verifying tests: Rust `synthetic::tests::same_symbol_and_bucket_produces_identical_snapshots`.

### SDH-LLR-123 — Evidence-file items are filtered by symbol
Statement: Loading the user-supplied evidence file shall include an item
for a given symbol only if the item's `symbols` list contains that symbol
(case-insensitive) or the literal wildcard `"*"`; an item with an empty/
absent `symbols` list shall be included for every symbol (matching
Python's `row.get("symbols", [symbol])` default, which trivially always
matches when no explicit list is given).
Source: `python/smart_hedge/data.py:229-234`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_data::evidence_file::row_applies_to_symbol`.
Verifying tests: Rust `evidence_file::tests::symbol_match_is_case_insensitive`,
`evidence_file::tests::wildcard_symbol_always_applies`.

### SDH-LLR-124 — Evidence-file fields are bounded and defensively typed
Statement: Loading the evidence file shall clamp `quality` to `[0, 1]`,
truncate `title`/`source`/`text` to fixed maximum lengths (240/120/5000
characters respectively), default `untrusted_text` to `true`, and skip
(not error on) any row that isn't a JSON object — a malformed evidence
file must degrade gracefully, not crash the whole snapshot.
Source: `python/smart_hedge/data.py:214-248`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_data::evidence_file::load_evidence_file`.
Verifying tests: Rust `evidence_file::tests::title_is_truncated_to_240_characters`,
`evidence_file::tests::non_object_rows_are_skipped_not_erroring`.

### SDH-LLR-125 — Missing or unreadable evidence file is not an error
Statement: A configured evidence-file path that doesn't exist, or that
fails to parse as JSON, shall yield an empty evidence list rather than
raising — a research convenience file must be optional in practice, not
just in the config schema.
Source: `python/smart_hedge/data.py:218-224` (`except (OSError,
json.JSONDecodeError): return []`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_data::evidence_file::load_evidence_file`.
Verifying tests: Rust `evidence_file::tests::missing_file_returns_empty_not_an_error`,
`evidence_file::tests::invalid_json_returns_empty_not_an_error`.

### SDH-LLR-126 — Network-backed providers are deferred, not stubbed silently
Statement: `AlpacaReadOnlyProvider`, FRED evidence, and RSS evidence are
explicitly **not yet ported** to Rust as of this entry — this is a
recorded scope decision (needs an HTTP-client dependency choice), not an
oversight. Any Rust code claiming provider parity must not silently omit
these.
Source: conversation, 2026-07-19.
Verification: Inspection. Status: Open (deferred by design — see
`rust/README.md` "Known scope: deferred network providers").

**Correction (2026-07-19, later the same day): no longer deferred.** The
HTTP-client dependency decision this entry called for has been made and
implemented: `ureq` (built on `rustls`, a pure-Rust TLS implementation) —
a documented exception to "hand-roll instead of depend," same reasoning as
`smart-hedge-store`'s `rusqlite`, scoped only to `smart-hedge-data` and
`smart-hedge-model-advisor` (see those crates' `Cargo.toml`). All three
providers this entry names are now implemented and tested:
`AlpacaReadOnlyProvider` (`SDH-LLR-081`), FRED evidence, and RSS evidence
(the last via a hand-rolled, narrowly-scoped, DTD/entity-free XML
extractor — `smart_hedge_data::rss_xml` — chosen specifically *because* it
eliminates the XXE attack surface by never implementing entity expansion
at all, not despite the "prefer depending on complex parsers" rule but as
an application of the same reasoning that rule serves). `OpenAIAdvisor`
(`SDH-LLR-056`) uses the same `ureq` dependency. This entry is kept,
uncorrected in its original text, per the methodology's requirement that
corrections be recorded, not silently rewritten.

## Orchestration (traces to SDH-HLR-020, SDH-HLR-040, SDH-HLR-060, SDH-HLR-100, SDH-HLR-110)

### SDH-LLR-130 — Contract type fields are validated, not merely typed
Statement: Resolving a contract shall reject any `option_type` other than
`call`/`put` and any `exercise_style` other than `american`/`european`,
even though the config schema already types these as strings — a syntactically
valid but semantically invalid value (e.g. `"straddle"`) must be caught at
resolution time, not passed through to the C++ core.
Source: `python/smart_hedge/engine.py:76-79` (`contract_for`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::contract::resolve_contract`.
Verifying tests: Rust `contract::tests::invalid_option_type_is_rejected`,
`contract::tests::invalid_exercise_style_is_rejected`.

### SDH-LLR-131 — "ATM" strike shorthand resolves from the live quote
Statement: A configured `strike` of the literal string `"ATM"`
(case-insensitive) shall resolve to the rounded current quote midpoint at
recommendation time, not a static value — and the resolved strike shall
still be validated positive and finite afterward.
Source: `python/smart_hedge/engine.py:89-94`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::contract::resolve_contract`.
Verifying tests: Rust `contract::tests::atm_strike_resolves_to_rounded_midpoint`,
`contract::tests::nonpositive_resolved_strike_is_rejected`.

### SDH-LLR-132 — Explicit expiry date overrides static days-to-expiry
Statement: When a contract specifies an `expiry` date (ISO `YYYY-MM-DD`),
days-to-expiry shall be computed dynamically as the time remaining until
21:00 UTC on that date, overriding any configured static
`days_to_expiry`; when `expiry` is absent, the configured
`days_to_expiry` (or its default) is used unchanged. The result is
floored at `0.0`, never negative.
Source: `python/smart_hedge/engine.py:40-46` (`_days_to_expiry`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::contract::days_to_expiry_from_date`.
Verifying tests: Rust `contract::tests::expiry_date_overrides_static_days_to_expiry`,
`contract::tests::expiry_date_overrides_even_an_explicit_days_to_expiry_override`,
`contract::tests::days_to_expiry_from_date_is_floored_at_zero_for_a_past_date`.

### SDH-LLR-133 — Canonical hashing extends to arbitrary audit values
Statement: The engine shall compute `input_hash` and `model_output_hash`
using the same canonical-JSON-then-SHA-256 approach as the decision-store
content hash (SDH-LLR-070/-071), applied to the combined
contract/snapshot/features/core inputs and to the model assessment
output respectively — not a different, ad hoc hashing scheme for audit
fields versus storage.
Source: `python/smart_hedge/engine.py:25-27` (`_canonical_hash`), `118-129`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::hashing::canonical_hash`.
Verifying tests: Rust `hashing::tests::canonical_hash_is_deterministic_regardless_of_key_order`,
`hashing::tests::canonical_hash_differs_for_different_values`.

### SDH-LLR-134 — Core binary is hashed for audit, "missing" if absent
Statement: The audit record shall include a SHA-256 hash of the resolved
core binary file; if the binary does not exist (or isn't a regular
file), the hash field shall be the literal string `"missing"` rather
than raising or omitting the field.
Source: `python/smart_hedge/engine.py:30-37` (`_file_hash`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::hashing::file_hash`.
Verifying tests: Rust `hashing::tests::file_hash_of_a_missing_path_is_the_literal_missing`,
`hashing::tests::file_hash_of_a_real_file_is_its_sha256`.

### SDH-LLR-135 — Replay is explicitly tagged as network-free
Statement: A replayed decision's audit block shall include
`replay_mode: "stored_inputs_and_outputs_no_network"`, making the
network-free guarantee (SDH-HLR-060) visible in the replayed payload
itself, not just true by construction.
Source: `python/smart_hedge/engine.py:157-162` (`replay`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_engine::engine::SmartHedgeEngine::replay`.
Verifying tests: Rust `integration_tests::replay_returns_the_stored_decision_tagged_as_a_replay`;
`smart_hedge_cli` `tests/cli_integration.rs::recent_and_replay_see_a_decision_persisted_by_a_prior_process`
(end-to-end across two separate process invocations).

### SDH-LLR-136 — Health report never claims an order endpoint exists
Statement: The engine's health report shall always include
`broker_order_endpoint_present: false`, mirroring the same guarantee the
per-decision audit record makes (SDH-LLR-080), at the service-health
level too.
Source: `python/smart_hedge/engine.py:167-176` (`health`).
Verification: Inspection. Status: Implemented.
Implementation: Rust `smart_hedge_engine::engine::SmartHedgeEngine::health`.
Verifying tests: Rust `integration_tests::recommendation_and_health_report_the_synthetic_heuristic_path`.

## CLI (traces to SDH-HLR-060, SDH-HLR-140)

### SDH-LLR-140 — CLI argument parsing rejects unknown flags and missing values
Statement: The CLI shall reject an unrecognized flag for a given
subcommand, and a flag given without its required value, with a specific
error message — never silently ignoring it or treating it as a positional
argument.
Source: `python/smart_hedge/cli.py` (`argparse`'s built-in behavior, relied
on implicitly); Rust CLI (hand-rolled, since a zero-dependency parser
doesn't get this for free — see `smart_hedge_cli::args::FlagCursor`).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_cli::args::parse_args`.
Verifying tests: Rust `args::tests::once_rejects_an_unknown_flag`,
`args::tests::missing_value_for_a_flag_is_reported`;
`tests/cli_integration.rs::an_unrecognized_flag_exits_2_before_touching_the_network_or_store`.
Python: **Open** (covered only by `argparse`'s own untested-here defaults).

### SDH-LLR-141 — `serve`/`mcp` launch the real dashboard/MCP server
Statement: The Rust CLI's `serve` subcommand shall bind and run the real
HTTP dashboard (`--host`/`--port` overriding the configured
`dashboard.host`/`dashboard.port`), and `mcp` shall run the real MCP
stdio server — matching Python's `cmd_serve`/`cmd_mcp`.
Source: `python/smart_hedge/cli.py` `cmd_serve`/`cmd_mcp`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_cli::commands::cmd_serve`/`cmd_mcp`.
Verifying tests: Rust `tests/cli_integration.rs::serve_starts_a_real_http_server_and_answers_health`
(spawns the real binary, reads its "listening on" line to learn the
OS-assigned `--port 0` port, makes a real TCP request against it);
`tests/cli_integration.rs::mcp_answers_initialize_and_tools_list_over_stdio`
(spawns the real binary, drives a real `initialize`/`tools/list` exchange
over its stdin/stdout).

**Correction (2026-07-19, later the same day):** an earlier version of
this entry described `serve`/`mcp` as recognized-but-not-yet-implemented,
pending the HTTP-server/MCP-protocol decisions `SDH-LLR-126` called for.
Both are now implemented — see `SDH-LLR-150` through `-156` below for the
dashboard/MCP requirements themselves.

### SDH-LLR-142 — `self-test` validates paper-only and hash-integrity invariants end-to-end
Statement: The `self-test` command shall build/verify the deterministic
core, run the core binary's own `--self-test`, generate one real
recommendation, and assert `mode == "paper"`,
`policy.live_execution_allowed == false`,
`audit.broker_order_endpoint_present == false`, and that replaying the
just-created decision reports `stored_content_hash_valid == true` —
failing loudly (nonzero exit) if any assertion doesn't hold, rather than
printing a partial pass.
Source: `python/smart_hedge/cli.py` `cmd_self_test`.
Verification: Test. Status: Implemented.
Implementation: Python `cmd_self_test`; Rust
`smart_hedge_cli::commands::cmd_self_test`.
Verifying tests: Rust `tests/cli_integration.rs::self_test_passes_against_the_synthetic_heuristic_path`
— this is the test that found and drove the `float_roundtrip` correction
under `SDH-LLR-072`. Python: **Open** (no automated test runs `cli.py
self-test` itself; it's a manually-invoked smoke test in practice).

### SDH-LLR-143 — `loop` enforces a minimum 1-second interval
Statement: The `loop` command shall sleep for at least `1.0` second
between recommendations regardless of a smaller or negative `--interval`
value.
Source: `python/smart_hedge/cli.py:58` (`time.sleep(max(1.0,
args.interval))`).
Verification: Inspection. Status: Implemented.
Implementation: Python `cmd_loop`; Rust
`smart_hedge_cli::commands::cmd_loop` (`interval.max(1.0)`).
Status: **Open** — no automated test exercises the loop's actual sleep
timing in either language (would require killing a long-running process
mid-loop); the Rust logic is a direct one-line mirror of Python's
`max(1.0, ...)`.

## Dashboard and MCP server (traces to SDH-HLR-020, SDH-HLR-060, SDH-HLR-140, SDH-HLR-150)

### SDH-LLR-150 — Dashboard binds to localhost by default, overridable per-call
Statement: The dashboard shall bind to `dashboard.host`/`dashboard.port`
(default `127.0.0.1:8765`) unless the CLI's `--host`/`--port` flags
override them.
Source: `python/smart_hedge/dashboard.py` `create_app`;
`python/smart_hedge/config.py` (`dashboard` defaults);
`python/smart_hedge/cli.py` `cmd_serve`.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_dashboard::server::serve`. The server
itself is a hand-rolled minimal HTTP/1.1 implementation
(`smart_hedge_dashboard::http`) rather than a dependency — safe to
hand-roll specifically *because* it never needs TLS (localhost, matching
Python's own `uvicorn` default) and only ever parses requests this
process itself defines the shape of, unlike the *client* side
(`ureq`/`rustls` in `smart-hedge-data`), which parses arbitrary
third-party HTTPS responses and must not be hand-rolled.
Verifying tests: Rust `smart_hedge_dashboard`'s `integration_tests::*` (8
tests binding a real ephemeral port and making real TCP requests);
`smart_hedge_cli` `tests/cli_integration.rs::serve_starts_a_real_http_server_and_answers_health`.

### SDH-LLR-151 — Dashboard exposes exactly the documented read-only routes
Statement: The dashboard shall expose `GET /`, `/api/health`,
`/api/recommendation`, `/api/history`, `/api/replay/{decision_id}` — no
route accepts a non-`GET` method or mutates state; a non-`GET` request or
an unrecognized path is rejected (`405`/`404`), not silently ignored.
Source: `python/smart_hedge/dashboard.py` `create_app`'s route
registrations.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_dashboard::routes::handle`.
Verifying tests: Rust `integration_tests::an_unknown_route_returns_404`,
`integration_tests::a_non_get_method_returns_405`,
`integration_tests::index_page_returns_the_html_console`.

### SDH-LLR-152 — Recommendation caching honors the `fresh=true` bypass and TTL
Statement: `/api/recommendation` shall serve a cached value for the same
symbol within `dashboard.cache_seconds` unless `fresh=true` is given, in
which case it always recomputes and refreshes the cache.
Source: `python/smart_hedge/dashboard.py` `_Cache`, `recommendation` endpoint.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_dashboard::cache::Cache`.
Verifying tests: Rust `cache::tests::a_fresh_entry_is_returned`,
`cache::tests::an_entry_older_than_the_ttl_is_not_returned`,
`integration_tests::recommendation_endpoint_returns_a_paper_only_decision`
(exercises the real endpoint with `fresh=true`).

### SDH-LLR-153 — Invalid symbol query parameters are rejected with `422`
Statement: The `symbol` query parameter to `/api/recommendation` and
`/api/history` shall be validated as 1-12 characters of
`[A-Za-z0-9._-]`, matching FastAPI's `Query(..., min_length=1,
max_length=12, pattern=...)` constraint — a violation returns `422`
before the engine is ever called, rather than passing through and
failing some other way downstream.
Source: `python/smart_hedge/dashboard.py` (`Query` parameter definitions).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_dashboard::routes::valid_symbol`.
Verifying tests: Rust `routes::tests::valid_symbol_rejects_empty_and_overlong_and_bad_characters`,
`integration_tests::an_invalid_symbol_is_rejected_with_422`.

### SDH-LLR-154 — Replay-not-found maps to `404`, not a generic failure
Statement: `/api/replay/{decision_id}` for an unknown ID shall return
`404` with a detail message, matching Python's `except KeyError:
raise HTTPException(status_code=404, ...)`; any other replay failure
returns `500` (Python's default for an unhandled exception), never a
misleading `200`.
Source: `python/smart_hedge/dashboard.py` `replay` endpoint.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_dashboard::routes::route_replay`.
Verifying tests: Rust `integration_tests::replay_of_an_unknown_decision_returns_404`.

### SDH-LLR-155 — MCP tool failures are `isError` results, not JSON-RPC protocol errors
Statement: A `tools/call` failure — an unknown tool name, a missing
required argument, or an underlying engine error — shall be reported as
a normal MCP result with `isError: true` and the failure message as
`content`, not a JSON-RPC-level `error` envelope; the JSON-RPC-level
`error` field is reserved for genuine protocol problems (parse failure,
unrecognized top-level method).
Source: `python/smart_hedge/mcp_server.py` (relies on the `FastMCP`
framework converting an exception raised inside a `@mcp.tool()` function
into an error *result*, not a transport failure — not written explicitly
in this file, but the observable contract any MCP client depends on).
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_mcp::protocol::handle_line`.
Verifying tests: Rust `protocol::tests::tools_call_with_an_unknown_tool_name_is_an_error_result_not_a_protocol_error`,
`protocol::tests::tools_call_replay_decision_without_a_decision_id_is_a_tool_error`,
`protocol::tests::an_unknown_top_level_method_is_a_jsonrpc_protocol_error`
(the contrasting case — confirms the two error paths are actually
distinct, not just differently named).

### SDH-LLR-156 — `price_option` runs the deterministic core directly
Statement: The `price_option` MCP tool shall invoke the deterministic C++
core directly with the caller-supplied `spot`/`strike`/etc., without
calling the market-data provider, model adviser, policy gate, or decision
store — a pure pricing calculator, not a recommendation.
Source: `python/smart_hedge/mcp_server.py` `price_option`.
Verification: Test, Inspection. Status: Implemented.
Implementation: Rust `smart_hedge_mcp::tools::price_option`/`build_contract`.
Verifying tests: Rust `tools::tests::build_contract_uses_the_configured_base_for_a_known_symbol`,
`tools::tests::build_contract_defaults_reasonably_for_an_unconfigured_symbol`,
`tools::tests::build_contract_never_leaves_an_atm_strike_or_an_expiry_date`
(the contract-shaping logic; the core invocation itself reuses
`smart_hedge_core_bridge::run_core`, already verified by that crate's own
tests — verifies SDH-HLR-020, "no other component recomputes the core's
numbers," by construction: `price_option` calls the same `run_core`
`smart-hedge-engine` does, not a second implementation).

**Correction (2026-07-19):** `price_option`'s exact base-contract
resolution for a symbol with no configured contract entry isn't fully
visible from this crate (Python's `_engine().contract_for` implementation
wasn't re-read in this pass); this port's choice — a fresh `ContractConfig`
built from only `strike`/`implied_volatility` via the same
`#[serde(default = ...)]` schema defaults `SDH-LLR-025` already
established — is a reasonable, directly-testable, documented judgment
call for the same observable behavior (a pricing utility that works for
any symbol), not a verified byte-for-byte match.

## Dependency minimization (traces to SDH-HLR-160)

### SDH-LLR-100 — C++ core has no third-party include
Statement: `cpp/smart_dynamic_hedge.cpp` shall include no third-party
header — only the C++ standard library.
Source: `docs/THREAT_MODEL.md` "Supply-chain risk"; direct inspection of
`cpp/smart_dynamic_hedge.cpp` includes.
Verification: Inspection. Status: Implemented.

### SDH-LLR-101 — Rust workspace dependency ceiling
Statement: The `rust/` workspace's only third-party runtime dependencies
shall be `serde` and `serde_json`; every other candidate dependency
(`time`, `uuid`, `thiserror`, etc.) shall be hand-rolled or omitted.
Source: conversation, 2026-07-19; `rust/Cargo.toml` `[workspace.dependencies]`
comment.
Verification: Inspection (dependency manifest review — could be automated
via `cargo tree` in CI). Status: Implemented.
