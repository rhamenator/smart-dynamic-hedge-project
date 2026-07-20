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
- the Rust port, which as of this pass covers the entire Python package's
  observable behavior (`rust/crates/smart-hedge-{models,config,policy,
  core-bridge,features,store,model-advisor,data,engine,cli,dashboard,mcp}`)
  — see `rust/README.md` for per-crate status and what's still open
  (mainly: direct tests for `SDH-LLR-031`/`-034`, and the live-network
  paths of the Alpaca/FRED/RSS/OpenAI integrations, which automated tests
  cannot exercise without real credentials).

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

## Cutover note (2026-07-19, after this recovery pass)

The "Current recovery scope" and "Sources used" sections above describe
this repository as it existed *during* the recovery pass, when the Python
package (`python/smart_hedge/`) and its `tests/` suite were still present
alongside the Rust port. The cutover from Python to Rust — anticipated but
not yet decided at the time of that pass — has since happened: `python/`
and `tests/` were removed from the active tree (recoverable via git
history), and the `smart-hedge` binary built from `rust/` is now the sole
running implementation. This note is appended rather than rewriting the
sections above, per this project's own DO-178-recovery-methodology rule
that corrections must be documented, not silently applied. Every LLR that
traced to a Python source file traces equally to that file's historical
git-log content; no requirement's substance changed because of the
cutover, only which file on disk currently satisfies it (see
`TRACEABILITY.md` and `LLR.md` for the equivalent per-requirement notes).
