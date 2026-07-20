# Threat model

## Protected properties

* No live order can be generated or transmitted.
* API credentials are not sent to the model or persisted in decisions.
* Deterministic prices, Greeks, and hedge targets cannot be overwritten by model
  text or evidence text.
* Every displayed decision has provenance and an integrity hash.
* Invalid, stale, illiquid, over-limit, or ungrounded results fail closed to
  `observe_blocked`.

## Threats and controls

### Prompt injection in news, filings, or RSS

Evidence text is labeled untrusted, truncated, and placed under a system-level
instruction to ignore embedded commands. The output schema excludes tool calls
and order fields. Cited evidence IDs are checked against the input set. This
reduces risk but does not make arbitrary web text trustworthy.

### Hallucinated facts or citations

Only supplied evidence IDs are accepted. Unknown IDs block the preview. Numerical
pricing and Greeks come from C++, not the model.

### Stale or conflicting market data

The policy checks quote age and spread. A production adapter should also reconcile
multiple venues, sequence numbers, crossed markets, exchange status, and clock
skew. This prototype has one quote source at a time.

### Credential leakage

Connectors read secrets from environment variables. Only derived market data enters
model input and audit JSON. Do not put credentials in the config or evidence file.

### Accidental live-trading evolution

There is no order route, method, tool, URL, or broker client. Policy always emits
`live_execution_allowed=false`. Adding execution should be a separate repository
and review, not an innocuous extension of a data adapter.

### Model/API outage

The default is heuristic. OpenAI failures can either fall back with an audited
reason or fail closed, controlled by configuration.

### Denial of service and cost runaway

The local dashboard uses a short cache and auto-refresh is opt-in. A remote MCP
transport needs authentication, request limits, timeouts, concurrency controls,
rate limits, and budget controls. The current stdio server assumes a trusted local
client.

### Supply-chain risk

The C++ core uses only the standard library. The Rust workspace keeps its
third-party dependencies deliberately minimal (`serde`/`serde_json`,
`rusqlite`, `ureq`/`rustls`) — see `rust/README.md` "Dependency and testing
policy" — and these should be kept current and scanned before deployment.
Do not expose the dashboard to an untrusted network.

### Tampering with decision history

SQLite rows include a SHA-256 content hash verified during replay. This detects
accidental mutation but is not a tamper-proof ledger: an attacker with database
write access can change both JSON and hash. Stronger deployment would use append-
only remote storage, signed records, and restricted credentials.
