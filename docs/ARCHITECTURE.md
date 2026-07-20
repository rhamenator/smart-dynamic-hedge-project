# Architecture and decision contract

## Trust hierarchy

The system has an explicit ordering of authority:

1. Configuration and non-model policy limits.
2. Timestamped market/evidence inputs.
3. Deterministic C++ value, Greeks, and hedge target.
4. Constrained adviser assessment.
5. Dashboard/MCP presentation.

A lower layer cannot override a higher one. In particular, the adviser cannot
write target shares or approve an action.

## Decision lifecycle

1. A read-only provider returns a `MarketSnapshot` with a quote, bars, and
   `EvidenceItem` objects.
2. Feature extraction computes volatility, EWMA volatility, short/long returns,
   drawdown, volume z-score, trend score, event flags, and a quality score.
3. The C++ process is invoked with explicit contract and market inputs. It returns
   value, Greeks, stock target, raw trade, and gamma scenario information.
4. The adviser receives a bounded copy. The default adviser is deterministic. The
   optional OpenAI adviser uses strict schema output.
5. Policy validates quote age, spread, market state, feature quality, evidence
   citations, confidence, model band bounds, share limit, and notional limit.
6. The decision is stored in SQLite before being returned to a caller.
7. Replay reads the stored JSON and verifies its content hash; it does not call a
   market API or model.

## Why a subprocess boundary

The C++ executable is intentionally a narrow deterministic service. The Rust
orchestration layer can be restarted, instrumented, and extended without
moving pricing logic into an LLM or web framework. A command-line JSON
boundary is slower than shared memory but
is adequate for human-scale debugging and gives each invocation explicit inputs.
A production low-latency design could expose the same functions through a stable
C ABI or a local gRPC service while retaining the trust boundary.

## Model contract

Allowed fields:

* regime
* confidence
* hedge urgency
* band multiplier in `[0.5, 3.0]`
* summary
* input evidence IDs
* risks
* bounded scenario shocks
* missing-data requests

Forbidden by absence:

* order side or type
* shares or contracts
* target delta
* option price or Greek overrides
* risk-limit changes
* execution approval
* API calls or credentials

The policy also verifies that cited IDs were present in the model input.

## Current position handling

`current_shares` is configuration/input. The system never changes it after a
preview. This prevents a debugging loop from drifting into a shadow execution
system. A future paper-fill simulator should be a separate component with an
explicit resettable account, deterministic fill model, and no broker credential.

## Cross-asset extension

Normalize each asset into a common decision envelope:

* identity and venue;
* price and timestamp;
* liquidity and spread;
* volatility and carry/funding;
* calendar/session state;
* news/macro/event evidence;
* current exposure;
* deterministic risk model;
* execution venue capability.

Keep asset-specific mechanics behind the envelope. Equity options need contract
multipliers, exercise style, dividends, and assignment. Futures need contract
value, variation margin, and expiry rolls. FX needs base/quote funding. Crypto
needs venue fragmentation and continuous sessions. A single generic “price” field
is not enough.
