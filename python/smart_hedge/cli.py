from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from .config import load_config
from .core_bridge import build_core, ensure_core
from .engine import SmartHedgeEngine


def _overrides(args: argparse.Namespace) -> dict[str, Any]:
    mapping = {
        "strike": args.strike,
        "implied_volatility": args.vol,
        "days_to_expiry": args.days,
        "current_shares": args.current_shares,
        "contracts": args.contracts,
    }
    return {key: value for key, value in mapping.items() if value is not None}


def _config(args: argparse.Namespace) -> dict[str, Any]:
    return load_config(args.config)


def cmd_build(args: argparse.Namespace) -> int:
    path = build_core(_config(args))
    print(path)
    return 0


def cmd_once(args: argparse.Namespace) -> int:
    engine = SmartHedgeEngine(_config(args))
    result = engine.recommendation(args.symbol.upper(), _overrides(args))
    print(json.dumps(result, indent=2, ensure_ascii=False))
    return 0


def cmd_loop(args: argparse.Namespace) -> int:
    engine = SmartHedgeEngine(_config(args))
    try:
        while True:
            value = engine.recommendation(args.symbol.upper(), _overrides(args))
            p = value["policy"]
            q = value["snapshot"]["quote"]
            m = value["model_assessment"]
            print(
                f"{value['created_at']} {value['symbol']} mid={(q['bid'] + q['ask']) / 2:.4f} "
                f"regime={m['regime']} action={p['action']} "
                f"preview={p['paper_trade_preview_shares']:.3f} blockers={p['blocking_reasons']}"
            )
            time.sleep(max(1.0, args.interval))
    except KeyboardInterrupt:
        return 0


def cmd_replay(args: argparse.Namespace) -> int:
    value = SmartHedgeEngine(_config(args)).replay(args.decision_id)
    print(json.dumps(value, indent=2, ensure_ascii=False))
    return 0


def cmd_recent(args: argparse.Namespace) -> int:
    values = SmartHedgeEngine(_config(args)).recent(args.limit, args.symbol or None)
    print(json.dumps(values, indent=2, ensure_ascii=False))
    return 0


def cmd_serve(args: argparse.Namespace) -> int:
    try:
        import uvicorn
    except ImportError as exc:
        raise RuntimeError("install dashboard dependencies: pip install -e '.[dashboard]'") from exc
    from .dashboard import create_app

    config = _config(args)
    app = create_app(args.config)
    dashboard = config.get("dashboard", {})
    uvicorn.run(
        app,
        host=args.host or str(dashboard.get("host", "127.0.0.1")),
        port=args.port or int(dashboard.get("port", 8765)),
        log_level="info",
    )
    return 0


def cmd_mcp(args: argparse.Namespace) -> int:
    if args.config:
        os.environ["SMART_HEDGE_CONFIG"] = str(Path(args.config).expanduser().resolve())
    from .mcp_server import main

    main()
    return 0


def cmd_self_test(args: argparse.Namespace) -> int:
    config = _config(args)
    binary = ensure_core(config)
    subprocess.run([str(binary), "--self-test"], check=True)
    engine = SmartHedgeEngine(config)
    value = engine.recommendation(args.symbol.upper())
    assert value["mode"] == "paper"
    assert value["policy"]["live_execution_allowed"] is False
    assert value["audit"]["broker_order_endpoint_present"] is False
    replay = engine.replay(value["decision_id"])
    assert replay["audit"]["stored_content_hash_valid"] is True
    print("python integration self-test: PASS")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="smart-hedge",
        description="Paper-only smart dynamic hedge debugger",
    )
    parser.add_argument("--config", help="JSON configuration path")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("build-core", help="compile the deterministic C++ core")

    for name, help_text in (
        ("once", "generate one recommendation"),
        ("loop", "generate repeated recommendations"),
    ):
        item = sub.add_parser(name, help=help_text)
        item.add_argument("--symbol", default="SPY")
        item.add_argument("--strike", type=float)
        item.add_argument("--vol", type=float)
        item.add_argument("--days", type=float)
        item.add_argument("--current-shares", type=float)
        item.add_argument("--contracts", type=int)
        if name == "loop":
            item.add_argument("--interval", type=float, default=15.0)

    replay = sub.add_parser("replay", help="read one decision without network/model calls")
    replay.add_argument("decision_id")

    recent = sub.add_parser("recent", help="show recent decisions")
    recent.add_argument("--limit", type=int, default=20)
    recent.add_argument("--symbol", default="")

    serve = sub.add_parser("serve", help="start the local browser dashboard")
    serve.add_argument("--host")
    serve.add_argument("--port", type=int)

    sub.add_parser("mcp", help="start the local stdio MCP server")
    self_test = sub.add_parser("self-test", help="run C++ and Python smoke tests")
    self_test.add_argument("--symbol", default="SPY")
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    handlers = {
        "build-core": cmd_build,
        "once": cmd_once,
        "loop": cmd_loop,
        "replay": cmd_replay,
        "recent": cmd_recent,
        "serve": cmd_serve,
        "mcp": cmd_mcp,
        "self-test": cmd_self_test,
    }
    try:
        raise SystemExit(handlers[args.command](args))
    except Exception as exc:
        print(f"error: {type(exc).__name__}: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc


if __name__ == "__main__":
    main()
