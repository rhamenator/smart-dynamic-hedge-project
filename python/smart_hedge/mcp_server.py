from __future__ import annotations

import json
import os
from functools import lru_cache
from typing import Any

from .config import load_config
from .core_bridge import run_core
from .engine import SmartHedgeEngine


@lru_cache(maxsize=1)
def _engine() -> SmartHedgeEngine:
    return SmartHedgeEngine(load_config(os.getenv("SMART_HEDGE_CONFIG")))


def _json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False)


def create_server():
    try:
        from mcp.server.fastmcp import FastMCP
    except ImportError as exc:
        raise RuntimeError("install MCP dependencies: pip install -e '.[mcp]'") from exc

    mcp = FastMCP(
        "smart-dynamic-hedge",
        instructions=(
            "Paper-only hedge research tools. Deterministic pricing and policy output are authoritative. "
            "There is intentionally no order-placement tool. Never represent a preview as an executed trade."
        ),
    )

    @mcp.tool()
    def health() -> str:
        """Return service health and prove that no broker-order endpoint is present."""
        return _json(_engine().health())

    @mcp.tool()
    def get_market_recommendation(symbol: str = "SPY") -> str:
        """Collect evidence and create one replayable paper hedge recommendation."""
        return _json(_engine().recommendation(symbol.upper()))

    @mcp.tool()
    def price_option(
        symbol: str = "SPY",
        spot: float = 100.0,
        strike: float = 100.0,
        implied_volatility: float = 0.20,
        days_to_expiry: float = 30.0,
        option_type: str = "put",
        exercise_style: str = "american",
        contracts: int = 1,
        current_shares: float = 0.0,
    ) -> str:
        """Run deterministic C++ price/Greeks/hedge math without market-data retrieval."""
        config = _engine().config
        base = _engine().contract_for(symbol.upper())
        base.update(
            {
                "strike": strike,
                "implied_volatility": implied_volatility,
                "days_to_expiry": days_to_expiry,
                "option_type": option_type,
                "exercise_style": exercise_style,
                "contracts": contracts,
                "current_shares": current_shares,
            }
        )
        return _json(run_core(config, base, spot))

    @mcp.tool()
    def replay_decision(decision_id: str) -> str:
        """Read a stored decision without accessing markets or calling a model."""
        return _json(_engine().replay(decision_id))

    @mcp.tool()
    def list_recent_decisions(limit: int = 10, symbol: str = "") -> str:
        """List recent paper decisions from the local SQLite audit log."""
        return _json(_engine().recent(limit=limit, symbol=symbol or None))

    @mcp.tool()
    def get_policy_snapshot() -> str:
        """Show non-model policy limits applied to every recommendation."""
        cfg = _engine().config
        return _json(
            {
                "mode": cfg["mode"],
                "policy": cfg["policy"],
                "broker_order_endpoint_present": False,
                "live_execution_allowed": False,
            }
        )

    return mcp


def main() -> None:
    server = create_server()
    # Stdio is the least exposed default and works with local MCP clients. A
    # network transport should be put behind authentication and a separate guard.
    server.run()


if __name__ == "__main__":
    main()
