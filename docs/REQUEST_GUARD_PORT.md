# Reusing `request-guard-mcp` safely

The existing `request-guard-mcp` repository is useful infrastructure, but its
current domain is request/bot/abuse classification. Its classifier verdict must
not be repurposed as an order approval.

## Reuse unchanged or nearly unchanged

* Tokio/Axum runtime and WebSocket JSON-RPC transport.
* Bearer authentication at connection establishment.
* Request-size and batch-size limits.
* Global semaphore/backpressure and per-tool timeouts.
* Structured tracing and Prometheus metrics.
* Redis cache and PostgreSQL audit adapters.
* Health/readiness endpoints, Docker, and Kubernetes patterns.

## Replace or add domain components

Create a separate trading-research tool registry:

* `get_market_snapshot`
* `price_option`
* `compute_hedge`
* `get_market_recommendation`
* `get_policy_snapshot`
* `replay_decision`
* `list_recent_decisions`

Do not include order tools in this paper-only service.

Replace generic abuse scoring with a `HedgePolicyEngine` whose input is a typed
recommendation record and whose output contains:

* `paper_preview_approved`
* `live_execution_allowed` fixed to false
* blocker codes
* effective no-trade band
* immutable limits snapshot

## Authentication is not authorization

A valid bearer token only identifies an allowed client. It does not establish that
a requested financial action is safe. Even a future paper-order simulator should
apply domain policy after authentication and before any state mutation.

## Suggested deployment split

```text
MCP client
   │
   ▼
request guard: auth / limits / timeout / metrics / audit
   │
   ▼
smart-hedge tools: read-only data / C++ calculation / recommendation
   │
   ├── no broker order dependency
   └── local or append-only decision store
```

This repository's own MCP server (`smart-hedge mcp`, part of the Rust
`smart-hedge-mcp` crate) is a hand-rolled stdio JSON-RPC transport — a
smaller runnable surface than `request-guard-mcp`'s Tokio/Axum WebSocket
server. Adopting `request-guard-mcp`'s remote transport becomes worthwhile
when remote multi-client operation, backpressure, Prometheus, and durable
audit are needed; it is not needed for the current local-only stdio use
case.
