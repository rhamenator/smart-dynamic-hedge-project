# High-level requirements — `smart-dynamic-hedge`

See `README.md` in this directory for scope, sources, and the shared
methodology. Statement format, fields, and ID scheme are defined in
`market-system-contracts`'s `docs/REQUIREMENTS_METHODOLOGY.md`.

---

### SDH-HLR-010 — No live execution from this repository

Statement: This repository shall never generate or transmit a live
(real-money) order, and shall provide no order-placement, order-cancel,
or broker-credential-management capability of any kind.

Source: `docs/THREAT_MODEL.md` "Protected properties" ("No live order can
be generated or transmitted") and "Accidental live-trading evolution";
`NOTICE`; `LEGAL_NOTICE.md`.

Rationale: Live execution and broker credentials belong exclusively to the
separate `trade-guard-mcp` repository, so a bug or feature creep in this
repository's data/model-adjacent code can never become a live order path.
See `docs/ROADMAP.md` "V2 multi-repository expansion".

Verification: Test, Inspection.

Status: Implemented.

---

### SDH-HLR-020 — Deterministic core is authoritative

Statement: All option pricing, Greeks, and stock hedge-target
calculations shall be computed by the deterministic C++ core, and no
other component (model adviser, policy gate, dashboard, CLI, or MCP tool)
shall be able to override, recompute, or substitute its own value for
those outputs.

Source: `docs/ARCHITECTURE.md` "Trust hierarchy" and "Why a subprocess
boundary"; `docs/THREAT_MODEL.md` "Protected properties".

Rationale: A model that consumes more information can become more
confidently wrong (README.md "What this does not prove") — keeping
pricing/Greeks in a narrow, deterministic, model-free component means a
persuasive-sounding but wrong model output can never corrupt the number
that actually matters.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-030 — Paper-only by default, hard-enforced

Statement: The system shall run in paper (observe/preview) mode by
default and shall refuse to start in any configuration that requests
live-mode or disables the paper-only policy flag.

Source: `python/smart_hedge/config.py` (`load_config` hard stops);
`NOTICE`.

Rationale: A configuration error should fail loudly at startup, not
silently permit a live-trading code path that doesn't otherwise exist in
this repository.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-040 — Independent non-model policy gate

Statement: Every recommendation shall pass through a policy gate, wholly
independent of any model adviser, that validates quote freshness, spread,
market state, feature/data quality, evidence-citation integrity, model
confidence and band bounds, and configured trade/notional limits before a
paper-trade preview is approved.

Source: `docs/ARCHITECTURE.md` "Decision lifecycle" step 5;
`docs/THREAT_MODEL.md` "Protected properties"; `python/smart_hedge/policy.py`.

Rationale: A model's job is to interpret, not to gate itself — the
policy engine is the deterministic, testable choke point that decides
what actually happens.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-050 — Fail-closed on invalid, stale, or ungrounded input

Statement: The system shall fail closed — blocking the paper-trade
preview via `action = "observe_blocked"` — whenever the quote is invalid,
stale, has excessive spread, the feature/data quality is below the
configured minimum, the model cites an evidence ID it wasn't given, a
deterministic core value is non-finite, the proposed trade or its
notional exceeds configured limits, or (when configured) the market is
closed and the trade would move outside the effective no-trade band.

Source: `docs/THREAT_MODEL.md` "Protected properties"; `python/smart_hedge/policy.py`.

Rationale: The default posture for any ambiguous, stale, or
out-of-bounds state must be "do nothing," never a best-effort guess.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-060 — Immutable, hash-verified decision audit trail

Statement: Every decision shall be recorded immutably with a
content-integrity hash sufficient to detect accidental mutation, and any
stored decision shall be replayable using only its stored inputs and
outputs, without any network or model call.

Source: `docs/ARCHITECTURE.md` "Decision lifecycle" steps 6–7;
`docs/THREAT_MODEL.md` "Tampering with decision history"; `python/smart_hedge/store.py`.

Rationale: A debugging/audit trail that can silently drift from what
actually happened is worse than no audit trail — replay must prove, not
merely claim, that nothing changed.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-070 — Untrusted evidence text cannot become an instruction

Statement: All externally retrieved evidence text (news, filings, RSS,
social content) shall be treated as untrusted data, kept separate from
any model system/developer instruction, and shall never be capable of
altering tool selection, policy, or execution mode regardless of its
content.

Source: `docs/THREAT_MODEL.md` "Prompt injection in news, filings, or
RSS"; `python/smart_hedge/model_advisor.py` (`hard_boundary` payload,
system instructions).

Rationale: Arbitrary retrieved text is an attacker-influenceable input
the moment any adapter fetches it from the open web or a user-supplied
file; the system must assume it can contain adversarial instructions.

Verification: Test, Inspection.

Status: Implemented.

---

### SDH-HLR-080 — Model adviser output cannot become an order

Statement: The model adviser's output schema shall have no field capable
of directly specifying an order side/type, a share or contract quantity,
a target delta, an option price or Greek override, a risk-limit change,
or an execution approval; any payload containing an unrecognized or
forbidden field shall be rejected outright, not partially accepted.

Source: `docs/ARCHITECTURE.md` "Model contract"; `README.md` ("no field
for `buy_shares`... extra fields are rejected"); `python/smart_hedge/model_advisor.py`
(`ASSESSMENT_SCHEMA`, `validate_assessment_payload`).

Rationale: "Forbidden by absence" is a stronger guarantee than "forbidden
by validation rule" — the schema should make the disallowed action
inexpressible, not merely rejected after the fact.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-090 — Evidence-citation integrity

Statement: The system shall verify that every evidence ID a model adviser
cites was actually present in the evidence supplied to it for that
decision, and shall block the preview if any citation is unknown.

Source: `docs/THREAT_MODEL.md` "Hallucinated facts or citations";
`python/smart_hedge/policy.py` (`MODEL_CITED_UNKNOWN_EVIDENCE`).

Rationale: An LLM can fabricate a plausible-looking evidence ID; only a
mechanical set-membership check, not the model's own claim, can catch
that.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-100 — Model-adviser is a swappable, bounded component

Statement: The active model adviser shall be selectable via configuration
alone (no code change), shall default to a fully deterministic heuristic
requiring no paid service, and shall be able to fall back to that
heuristic automatically on adviser failure when configured to do so.

Source: `docs/ARCHITECTURE.md`; `README.md` ("OpenAI adviser... on API
error... fallback to the local heuristic"); `python/smart_hedge/model_advisor.py`
(`build_advisor`, `HeuristicAdvisor`, `OpenAIAdvisor`).

Rationale: The system must keep working, for free, with no external
account, and a paid/optional model must never be a single point of
failure for the whole pipeline.

Verification: Test.

Status: Implemented.

---

### SDH-HLR-110 — No credential ever reaches the model, dashboard, or audit record

Statement: Model API keys, market-data provider credentials, and any
other secret shall never be included in a model prompt, a dashboard
response, an MCP tool response, or a stored audit/decision record; only
derived, non-secret market data and evidence shall reach those surfaces.

Source: `docs/THREAT_MODEL.md` "Credential leakage"; `python/smart_hedge/engine.py`
(`audit["secrets_sent_to_model"] = False`).

Rationale: Every one of these surfaces is either logged, displayed, or
sent to a third party (the model provider) — a credential reaching any of
them is a leak, not a convenience.

Verification: Test, Inspection.

Status: Implemented.

---

### SDH-HLR-120 — Multiplatform operation

Statement: The system shall build and run correctly on Windows, Linux,
and macOS, including correctly locating, building, and invoking its
native C++ core with the platform-appropriate binary name and build-tool
discovery order.

Source: `python/smart_hedge/core_bridge.py` (`.exe` suffix handling,
Windows multi-config generator fallback, `cmake`/`g++`/`clang++`
discovery); conversation, 2026-07-19 ("I intend for the system to be
multiplatform, as is the case with many of my repositories").

Rationale: Explicit user requirement; also a natural consequence of
targeting both C++ (via CMake or a direct compiler) and Rust, neither of
which should assume a single OS.

Verification: Test, Analysis (cross-platform logic reviewed even where a
specific OS isn't available to execute the test on).

Status: Implemented (Rust port); the Python original has the same
behavior but no dedicated cross-platform test (see `SDH-LLR-041`, `SDH-LLR-042`).

---

### SDH-HLR-130 — Zero-cost operation

Statement: The system shall provide a fully functional synthetic data
mode and heuristic adviser requiring no paid service, external account,
or API key.

Source: `README.md` "Zero-cost quick start"; `python/smart_hedge/data.py`
(`SyntheticProvider`); `python/smart_hedge/model_advisor.py`
(`HeuristicAdvisor`).

Rationale: A researcher or reviewer must be able to run and inspect the
entire pipeline without an account, a payment, or a network call.

Verification: Test, Demonstration.

Status: Implemented.

---

### SDH-HLR-140 — No order-capable interface surface

Statement: None of the system's operator/API surfaces (CLI, browser
dashboard, MCP stdio server) shall expose a tool, endpoint, or command
capable of placing, modifying, or canceling an order, or of managing
broker credentials.

Source: `README.md` "MCP tools"; `python/smart_hedge/mcp_server.py`;
`python/smart_hedge/dashboard.py`; `python/smart_hedge/cli.py`.

Rationale: This is `SDH-HLR-010` restated at the interface-surface level
— the guarantee has to hold at every entry point, not just "somewhere in
the pipeline."

Verification: Test, Inspection.

Status: Implemented.

---

### SDH-HLR-150 — Local-only surfaces by default

Statement: The browser dashboard and MCP server shall bind to localhost
by default and shall require no external network exposure for normal
operation.

Source: `docs/THREAT_MODEL.md` "Denial of service and cost runaway"
("The current stdio server assumes a trusted local client");
`python/smart_hedge/config.py` (`dashboard.host` default `127.0.0.1`);
`python/smart_hedge/mcp_server.py` (stdio transport).

Rationale: A localhost-only default means an operator has to take an
affirmative, documented step to expose either surface to a network,
rather than discovering later that it was reachable by default.

Verification: Test, Inspection.

Status: Implemented.

---

### SDH-HLR-160 — No third-party runtime dependency in the deterministic core

Statement: The deterministic pricing/Greeks/hedge core shall depend on no
third-party library at runtime.

Source: `docs/THREAT_MODEL.md` "Supply-chain risk" ("The C++ core uses
only the standard library"); conversation, 2026-07-19 (dependency-
minimization policy extended to the whole system).

Rationale: The component computing the one value everything else defers
to (`SDH-HLR-020`) should have the smallest possible supply-chain attack
surface; this requirement is also why the Rust port
(`smart-hedge-core-bridge`, etc.) keeps only `serde`/`serde_json` and
hand-rolls everything else reasonably hand-rollable.

Verification: Inspection (dependency manifest review).

Status: Implemented.
