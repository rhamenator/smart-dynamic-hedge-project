# Requirements (recovered)

DO-178-inspired requirements recovery for this repository — see
[`market-system-contracts`'s `docs/REQUIREMENTS_METHODOLOGY.md`](https://github.com/rhamenator/market-system-contracts/blob/main/docs/REQUIREMENTS_METHODOLOGY.md)
for the scheme, ID format, and rationale. Prefix for this repository: `SDH`.

## Files

- [`HLR.md`](HLR.md) — high-level, system-behavior requirements.
- [`LLR.md`](LLR.md) — low-level, implementation-adjacent requirements, each
  tracing to an HLR.
- [`TRACEABILITY.md`](TRACEABILITY.md) — the matrix: LLR → implementation →
  verifying test → status.

## Current recovery scope (as of 2026-07-19)

This pass covers the code that exists in this repository today:

- the C++ deterministic core (`cpp/smart_dynamic_hedge.cpp`);
- the full Python package (`python/smart_hedge/`);
- the Rust port completed so far (`rust/crates/smart-hedge-{models,config,policy,core-bridge}`).

It does **not** yet cover `market-intelligence-mcp` or `trade-guard-mcp` —
those repositories have not had a recovery pass. It also does not cover
the V2 international/multi-asset expansion described in `docs/ROADMAP.md`
"V2 multi-repository expansion", since that work doesn't exist as running
code yet; requirements get recovered from what exists, not from what's
planned. New requirements for that expansion should be written
requirements-first as that work actually happens, not recovered.

## Sources used for this recovery pass

- `docs/ARCHITECTURE.md` (trust hierarchy, decision lifecycle, model
  contract, current-position handling)
- `docs/THREAT_MODEL.md` (protected properties, threats and controls)
- `README.md` ("What this does not prove", component descriptions)
- `NOTICE` / `LEGAL_NOTICE.md` (paper-only/live-trading boundary)
- Direct reading of every Python module, the C++ core, and the Rust port
- `tests/test_policy.py`, `tests/test_model_schema.py`, `tests/test_engine.py`
  (existing Python test suite)
- This project's conversation history (architectural decisions made in
  discussion rather than written down elsewhere) — cited as
  `Source: conversation, 2026-07-19` where that's the only source.

## Honesty note

A requirement marked `Status: Open` in `LLR.md`/`TRACEABILITY.md` means a
real behavior exists in the code with no (or an incomplete) automated test
proving it — this recovery pass surfaces those gaps rather than papering
over them. Closing them is future work, tracked the same way any other gap
is.
