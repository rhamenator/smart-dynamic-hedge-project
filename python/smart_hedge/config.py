from __future__ import annotations

import copy
import json
import os
from pathlib import Path
from typing import Any


DEFAULT_CONFIG: dict[str, Any] = {
    "mode": "paper",
    "provider": {
        "kind": "synthetic",
        "alpaca": {
            "data_base_url": "https://data.alpaca.markets",
            "feed": "iex",
            "bar_timeframe": "1Min",
            "bar_limit": 180,
            "timeout_seconds": 8.0,
        },
        "evidence_file": "data/evidence.example.json",
        "fred": {
            "enabled": False,
            "series": ["VIXCLS", "DGS2", "DGS10"],
            "timeout_seconds": 8.0,
        },
        "rss": {"enabled": False, "feeds": [], "max_items_per_feed": 3},
    },
    "model": {
        "kind": "heuristic",
        "name": "configure-with-OPENAI_MODEL",
        "timeout_seconds": 20.0,
        "max_evidence_items": 20,
        "max_evidence_chars": 1200,
        "fallback_to_heuristic": True,
    },
    "core": {
        "binary": "",
        "tree_steps": 600,
        "auto_build": True,
        "timeout_seconds": 12.0,
    },
    "features": {
        "bars_per_year": 98280.0,
        "ewma_lambda": 0.94,
        "short_window": 20,
        "long_window": 90,
    },
    "policy": {
        "paper_only": True,
        "max_quote_age_seconds": 45.0,
        "max_spread_bps": 35.0,
        "min_data_quality": 0.65,
        "min_model_confidence_for_band_change": 0.55,
        "min_band_multiplier": 0.50,
        "max_band_multiplier": 3.00,
        "max_abs_trade_shares": 500.0,
        "max_preview_notional": 50_000.0,
        "allow_fractional_shares": True,
        "require_market_open_for_preview": True,
    },
    "storage": {"sqlite_path": ".smart_hedge/decisions.sqlite3"},
    "dashboard": {"host": "127.0.0.1", "port": 8765, "cache_seconds": 5.0},
    "contracts": {
        "SPY": {
            "option_type": "put",
            "exercise_style": "american",
            "strike": 100.0,
            "days_to_expiry": 30.0,
            "contracts": 1,
            "multiplier": 100.0,
            "current_shares": 0.0,
            "rate": 0.045,
            "dividend_yield": 0.012,
            "implied_volatility": 0.20,
            "base_no_trade_band_shares": 2.0,
        }
    },
}


def _deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(base)
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(result.get(key), dict):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = copy.deepcopy(value)
    return result


def project_root() -> Path:
    return Path(__file__).resolve().parents[2]


def load_config(path: str | os.PathLike[str] | None = None) -> dict[str, Any]:
    selected = path or os.getenv("SMART_HEDGE_CONFIG")
    config = copy.deepcopy(DEFAULT_CONFIG)
    if selected:
        config_path = Path(selected).expanduser().resolve()
        with config_path.open("r", encoding="utf-8") as handle:
            user_config = json.load(handle)
        if not isinstance(user_config, dict):
            raise ValueError("configuration root must be a JSON object")
        config = _deep_merge(config, user_config)
        config["_config_path"] = str(config_path)
        config["_config_dir"] = str(config_path.parent)
    else:
        config["_config_dir"] = str(project_root())

    if os.getenv("SMART_HEDGE_PROVIDER"):
        config["provider"]["kind"] = os.environ["SMART_HEDGE_PROVIDER"]
    if os.getenv("SMART_HEDGE_MODEL_KIND"):
        config["model"]["kind"] = os.environ["SMART_HEDGE_MODEL_KIND"]
    if os.getenv("OPENAI_MODEL"):
        config["model"]["name"] = os.environ["OPENAI_MODEL"]
    if os.getenv("SMART_HEDGE_CORE"):
        config["core"]["binary"] = os.environ["SMART_HEDGE_CORE"]
    if os.getenv("SMART_HEDGE_DB"):
        config["storage"]["sqlite_path"] = os.environ["SMART_HEDGE_DB"]

    # Hard stop: this research project deliberately has no live mode.
    if str(config.get("mode", "paper")).lower() != "paper":
        raise ValueError("only mode='paper' is implemented")
    if not bool(config["policy"].get("paper_only", True)):
        raise ValueError("policy.paper_only must remain true")
    return config


def resolve_project_path(config: dict[str, Any], raw: str) -> Path:
    path = Path(raw).expanduser()
    if path.is_absolute():
        return path
    base = Path(config.get("_config_dir", project_root()))
    return (base / path).resolve()
