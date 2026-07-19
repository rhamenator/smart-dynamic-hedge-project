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
`contracts`, `multiplier`, `current_shares`, `rate`, `dividend_yield`, and
`base_no_trade_band_shares` defaulted; `strike`, `days_to_expiry`, and
`implied_volatility` remain required.
Source: `python/smart_hedge/core_bridge.py:90-112` (`contract.get(key,
default)` calls); recovered behavior of `_deep_merge` on a symbol absent
from the base config.
Verification: Test. Status: Implemented.
Implementation: Rust `smart_hedge_config::types::ContractConfig`
(`#[serde(default = ...)]` per field). Python: implicit in
`core_bridge.run_core`'s per-field `.get()` calls — same net behavior,
recovered from reading the code, not from a written-down rule.
Verifying tests: Rust `loader::tests::a_brand_new_contract_symbol_gets_only_the_fields_it_specifies_plus_defaults`,
`loader::tests::a_new_contract_symbol_missing_a_required_field_fails_fast_at_load_time`.
Python: **Open**.

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
Verification: Test. Status: Implemented (Python only — not yet ported).
Implementation: Python `validate_assessment_payload`.
Verifying tests: `tests/test_model_schema.py::test_extra_trade_field_rejected`.

### SDH-LLR-051 — Regime enum enforcement
Statement: `regime` shall be one of exactly seven values: `calm`,
`trend_up`, `trend_down`, `volatile`, `jump_risk`, `illiquid`,
`uncertain`.
Source: `python/smart_hedge/model_advisor.py:11-19,104-106`.
Verification: Test. Status: Implemented (Python only).
Implementation: Python `ALLOWED_REGIMES`, `validate_assessment_payload`.
Verifying tests: **Open** — `test_model_schema.py` doesn't test an
invalid regime directly (only the valid-payload and extra-field/
out-of-range-band cases).

### SDH-LLR-052 — Numeric field bounds
Statement: `confidence`/`hedge_urgency` shall be finite numbers in
`[0, 1]`; `band_multiplier` shall be a finite number in `[0.5, 3.0]`;
values outside these ranges (or non-numeric, or boolean) shall be
rejected.
Source: `python/smart_hedge/model_advisor.py:76-82,118-120`.
Verification: Test. Status: Implemented (Python only).
Implementation: Python `_finite_number`.
Verifying tests: `test_model_schema.py::test_out_of_range_band_rejected`.

### SDH-LLR-053 — Scenario-shock bounds
Statement: `scenario_spot_shocks` shall contain 1 to 7 finite numbers,
each in `[-0.30, 0.30]`.
Source: `python/smart_hedge/model_advisor.py:107-110`.
Verification: Test. Status: Implemented (Python only). **Open**: no
dedicated test for the count or per-item bound (only the valid-payload
case exercises this field, with in-range values).

### SDH-LLR-054 — Bounded string-list fields
Statement: `evidence_ids`, `risks`, and `data_requests` shall each be
lists capped at 8 items, with each item truncated to a maximum length
(160/240/240 characters respectively) rather than rejected for being too
long.
Source: `python/smart_hedge/model_advisor.py:85-93`.
Verification: Test. Status: Implemented (Python only). **Open**: no test
exercises the truncation or the item-count cap.

### SDH-LLR-055 — Heuristic adviser makes no network call
Statement: `HeuristicAdvisor.assess` shall be a pure function of its
inputs (snapshot, features, core, contract) with no network access, no
file I/O beyond what those inputs already contain, and shall always
return a schema-valid `ModelAssessment`.
Source: `python/smart_hedge/model_advisor.py:130-207`.
Verification: Test, Inspection. Status: Implemented (Python only).
Verifying tests: transitively via `test_engine.py`; **Open** — no direct
unit test isolates `HeuristicAdvisor` from the rest of the engine.

### SDH-LLR-056 — OpenAI adviser fails fast on missing configuration
Statement: Constructing `OpenAIAdvisor` shall raise immediately (not on
first use) if no usable model name is configured or if
`OPENAI_API_KEY` is not set.
Source: `python/smart_hedge/model_advisor.py:214-222`.
Verification: Test. Status: Implemented (Python only). **Open**: no test
constructs `OpenAIAdvisor` and asserts the immediate failure (would
require monkeypatching `openai` or running without network — reasonable
to add without needing a real API key, since the check happens before
any request).

### SDH-LLR-057 — Fallback-to-heuristic is configuration-gated
Statement: When the active adviser raises, the engine shall fall back to
`HeuristicAdvisor` and record the fallback reason only when
`model.fallback_to_heuristic` is true; otherwise the original exception
shall propagate.
Source: `python/smart_hedge/engine.py:99-107`.
Verification: Test. Status: Implemented (Python only). **Open**: no test
exercises either branch of this behavior (would require an adviser stub
that always raises).

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

### SDH-LLR-081 — Alpaca provider is data-only
Statement: `AlpacaReadOnlyProvider` shall construct requests only against
the Alpaca market-data host and only for quote/bar GET endpoints; it
shall hold no code path capable of constructing an order-placement
request.
Source: `python/smart_hedge/data.py:123-211`.
Verification: Inspection. Status: Implemented (Python only).

### SDH-LLR-082 — MCP tool set contains no order-capable tool
Statement: The MCP server shall expose exactly `health`,
`get_market_recommendation`, `price_option`, `replay_decision`,
`list_recent_decisions`, `get_policy_snapshot` — no tool named or
equivalent to `place_order`/`submit_order`/`cancel_order`/credential
management.
Source: `python/smart_hedge/mcp_server.py:36-96`; `README.md` "MCP tools".
Verification: Test, Inspection. Status: Implemented (Python only, not yet
ported). **Open**: no test enumerates the tool list and asserts its exact
membership (would need to introspect the `FastMCP` server's registered
tools).

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
