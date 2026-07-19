# Smart Dynamic Hedge

A paper-only, replayable dynamic-hedging research lab. It combines a deterministic
C++ option/Greeks engine with Python data adapters, feature extraction, an optional
LLM regime adviser, a non-model policy gate, a local browser dashboard, an MCP
server, and a SQLite audit log.

The project deliberately **does not contain a broker order endpoint**. It can read
market data and display a hypothetical stock-hedge preview, but it cannot place or
simulate-fill an order and it never updates `current_shares` automatically.

> Educational/research software, not financial advice. A hedge model can lose
> money through model error, jumps, liquidity, transaction costs, dividends,
> early exercise, stale data, and operational mistakes.

## What “smart” means here

The model is not the calculator and is not the trader.

* **C++ is authoritative** for option value, American/European exercise treatment,
  Greeks, signed position delta, and the stock hedge target.
* **Data adapters** normalize quotes, bars, macro observations, event flags, and
  untrusted news text into timestamped evidence with provenance.
* **The adviser** may classify a regime, report uncertainty, choose scenario
  shocks, and propose a bounded multiplier for the no-trade band.
* **The policy gate** independently checks freshness, spread, data quality,
  evidence citations, position size, notional, market state, and paper-only mode.
* **The decision log** records inputs, outputs, hashes, model identity, fallback
  state, and the exact policy outcome for offline replay.

The LLM schema has no field for `buy_shares`, `sell_shares`, `target_delta`, or
`approve_order`. Extra fields are rejected. Even a valid model result can only
change the no-trade band within configured limits, and low-confidence results
cannot change it at all.

## Architecture

```text
                         read-only connectors
                    quote / bars / macro / RSS
                                 │
                                 ▼
                    normalized evidence bundle
                   timestamps + source + quality
                                 │
                    ┌────────────┴────────────┐
                    ▼                         ▼
             feature extraction       C++ deterministic core
             vol/trend/liquidity       value/Greeks/target shares
                    │                         │
                    └────────────┬────────────┘
                                 ▼
                        constrained adviser
                  heuristic (default) or OpenAI API
                                 │
                                 ▼
                       non-model policy gate
                    freshness/spread/limits/paper
                                 │
                    ┌────────────┴────────────┐
                    ▼                         ▼
             SQLite decision log       dashboard / MCP tools
                    │
                    ▼
               no-network replay
```

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and
[`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) for the detailed contracts.

## Included components

### Deterministic C++17 core

`cpp/smart_dynamic_hedge.cpp` has no third-party dependency and supports:

* European Black–Scholes pricing.
* American Cox–Ross–Rubinstein binomial pricing.
* Continuous dividend yield and financing rate.
* Calls and puts; signed long/short contract counts.
* Delta, gamma, vega, theta, and rho.
* OCC-style configurable multiplier, defaulting to 100.
* Current stock position, target stock position, and no-trade band.
* Machine-readable JSON output and numerical self-tests.

The core does not access the network or a model.

### Data inputs

The first version implements:

* `synthetic`: free, local, changing test data; always available.
* `alpaca-readonly`: current equity quotes and bars from Alpaca's market-data
  service. It contains data URLs only—no paper or live order URL.
* `FRED`: optional macro-series observations.
* `RSS/Atom`: optional headlines/summaries, marked as untrusted text.
* `evidence_file`: user-supplied JSON for event calendars, option-chain metrics,
  cross-asset signals, filings, analyst estimates, or experimental features.

The adapter interface is intentionally small. SEC filings, a real options chain,
corporate actions, dividend calendars, borrow costs, futures, crypto, and richer
news can be added without changing the C++ or policy contracts.

### Advisers

`heuristic` is the free default. It is transparent and suitable for plumbing and
policy tests.

`openai` sends a bounded, redacted evidence packet to the OpenAI Responses API and
requests a strict JSON-schema result. API keys are used by the Python client and
are never inserted into model input or the audit record. The model name is
configuration, not code, because ChatGPT product labels and API model identifiers
need not be the same.

### MCP tools

The local stdio MCP server exposes only:

* `health`
* `get_market_recommendation`
* `price_option`
* `replay_decision`
* `list_recent_decisions`
* `get_policy_snapshot`

There is intentionally no `place_order`, `submit_order`, `cancel_order`, or
credential-management tool.

## Zero-cost quick start

Requirements:

* CMake 3.16+ and a C++17 compiler, or just `g++`/`clang++`.
* Python 3.11+.

Build and run without installing any Python package:

```bash
cd smart_dynamic_hedge
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j
ctest --test-dir build --output-on-failure

PYTHONPATH=python \
SMART_HEDGE_CORE=build/smart_dynamic_hedge \
python -m smart_hedge.cli --config config.example.json once --symbol SPY
```

Or:

```bash
make test
make demo
```

The first recommendation uses synthetic data and the deterministic heuristic, so
it needs no market-data account and makes no paid model call.

## Browser dashboard

Install the optional dependencies:

```bash
python -m venv .venv
. .venv/bin/activate                    # Windows: .venv\Scripts\activate
python -m pip install -e '.[dashboard]'
make build
smart-hedge --config config.example.json serve
```

Open `http://127.0.0.1:8765`. The dashboard displays the quote, model value,
Greeks, deterministic target, effective band, paper preview, regime explanation,
blockers, limits, hashes, and complete decision JSON.

Auto-refresh is opt-in. That matters when an API-backed model is enabled because
every uncached refresh may cost money.

## OpenAI adviser

Install the model extra and select a currently supported API model:

```bash
python -m pip install -e '.[model]'
export SMART_HEDGE_MODEL_KIND=openai
export OPENAI_API_KEY='...'
export OPENAI_MODEL='your-supported-api-model-id'
smart-hedge --config config.example.json once --symbol SPY
```

The call uses the Responses API with a strict schema. On an API error, invalid
schema, timeout, or absent output, the configured default is to fall back to the
local heuristic and write the reason to the audit record. Set
`model.fallback_to_heuristic` to `false` to fail closed instead.

The model receives:

* quote midpoint, spread, timestamp, market state, and source;
* computed volatility, momentum, trend, drawdown, volume, and data-quality fields;
* deterministic value/Greeks/hedge output;
* a bounded set of timestamped evidence items.

It does **not** receive API secrets or authority to change the core calculation.
News and RSS content are explicitly labeled untrusted and delimited as data.

## Read-only market data with Alpaca

Create a configuration from `config.alpaca-readonly.example.json`, then set:

```bash
export ALPACA_API_KEY_ID='...'
export ALPACA_API_SECRET_KEY='...'
smart-hedge --config config.alpaca-readonly.example.json once --symbol SPY
```

Only Alpaca's market-data host is present in the adapter. Available feeds,
real-time entitlements, exchange coverage, and historical limits depend on the
account and data plan. Check the provider's current documentation rather than
assuming a free feed is consolidated or suitable for production.

The configured option strike, expiry, implied volatility, dividend yield, and
current shares remain user inputs in this version. The adapter does not yet pull
or select a listed-option contract. That omission is intentional: silently
mixing a live stock quote with a guessed option contract would be worse than
requiring an explicit contract definition.

## Macro, news, events, and custom evidence

FRED:

```json
"fred": {
  "enabled": true,
  "series": ["VIXCLS", "DGS2", "DGS10"]
}
```

Then set `FRED_API_KEY`.

RSS/Atom:

```json
"rss": {
  "enabled": true,
  "feeds": ["https://example.com/feed.xml"],
  "max_items_per_feed": 3
}
```

Only use feeds whose terms permit automated retrieval. Feed text is untrusted and
should not be treated as a reliable numerical signal without a validated parser.

Custom evidence follows `data/evidence.example.json`:

```json
{
  "evidence_id": "earnings-2026-08-01",
  "symbols": ["XYZ"],
  "kind": "event",
  "title": "Earnings after close",
  "timestamp": "2026-08-01T20:00:00Z",
  "source": "your-calendar-adapter",
  "value": true,
  "text": "Scheduled event; no directional claim.",
  "quality": 0.9,
  "untrusted_text": false
}
```

Every model citation must match an input `evidence_id`; invented IDs block the
paper preview.

## MCP server

Install the MCP extra and build the C++ binary:

```bash
python -m pip install -e '.[mcp]'
make build
smart-hedge --config config.example.json mcp
```

The default is stdio, minimizing network exposure. Adapt
`mcp-config.example.json` for an MCP-capable local client.

For a remote transport, add TLS, authentication, request-size limits, per-tool
timeouts, concurrency backpressure, secret isolation, rate limits, and immutable
audit storage. The repository `request-guard-mcp` is a useful source of those
infrastructure patterns, but its existing bot/abuse classification tools are not
an order-authorization policy. See
[`docs/REQUEST_GUARD_PORT.md`](docs/REQUEST_GUARD_PORT.md).

## Direct C++ use

```bash
./build/smart_dynamic_hedge \
  --spot 100 \
  --strike 100 \
  --rate 0.045 \
  --dividend-yield 0.012 \
  --vol 0.20 \
  --days 30 \
  --type put \
  --style american \
  --contracts 1 \
  --multiplier 100 \
  --current-shares 0 \
  --no-trade-band 2 \
  --json
```

Signed contract convention:

* `+1`: long one option contract.
* `-1`: short one option contract.

The stock target is always:

```text
target_stock_shares = -(contracts × multiplier × model_delta)
```

## Tests

```bash
make test
```

The test suite checks:

* a known Black–Scholes price and delta;
* American put value is not below its European comparison;
* short-call hedge sign;
* model schema rejects an injected `buy_shares` field;
* low model confidence cannot change the band;
* stale data and invented evidence citations block previews;
* no decision allows live execution;
* SQLite records pass content-hash verification on replay.

## What this does not prove

A plausible dashboard is not evidence of an edge. Before treating any feature or
model output as useful, a serious experiment would need:

1. Point-in-time data with publication timestamps and no look-ahead leakage.
2. Walk-forward evaluation, untouched holdout periods, and regime stratification.
3. Bid/ask, market impact, fees, borrow, financing, dividends, assignment, and
   corporate-action handling.
4. A historical option surface—not a single fixed volatility—and realistic
   exercise/assignment behavior.
5. Model-version, prompt, feature, and source pinning for exact reproducibility.
6. Baselines that test whether the LLM adds anything beyond delta, gamma,
   volatility, and simple event rules.
7. Adversarial tests for stale feeds, conflicting prices, prompt injection,
   malformed model output, and dependency outages.

A model that consumes more information can become more confidently wrong. The
correct research question is not “does the explanation sound intelligent?” but
“does the bounded adviser improve a pre-registered out-of-sample metric after all
costs, without increasing tail risk?”

## Project status

This is a functional research scaffold, not a production hedge system. The most
important missing pieces are a point-in-time option-chain adapter, exchange-grade
calendar/corporate-action handling, dividend forecasts, paper fill simulation,
portfolio-level aggregation, and a proper walk-forward backtester.

## License

This project is licensed under the GNU General Public License, version 3 (or,
at your option, any later version). See [LICENSE](LICENSE).

Project-specific legal notices are documented in [NOTICE](NOTICE) and
[LEGAL_NOTICE.md](LEGAL_NOTICE.md). These include attribution preservation,
origin-marking, trademark boundaries, and financial-research disclaimers that
are intended to be compatible with GPLv3 section 7.
