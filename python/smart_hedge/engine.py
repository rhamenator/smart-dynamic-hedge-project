from __future__ import annotations

import copy
import hashlib
import json
import math
import uuid
from dataclasses import asdict
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any

from .config import load_config, project_root, resolve_project_path
from .core_bridge import resolve_binary, run_core
from .data import MarketDataProvider, build_provider
from .features import build_features
from .model_advisor import Advisor, HeuristicAdvisor, build_advisor
from .models import ModelAssessment, Recommendation, to_dict, utc_now_iso
from .policy import POLICY_VERSION, evaluate_policy
from .store import DecisionStore

ENGINE_VERSION = "smart-orchestrator-v0.2.0"


def _canonical_hash(value: Any) -> str:
    body = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def _file_hash(path: Path) -> str:
    if not path.exists() or not path.is_file():
        return "missing"
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _days_to_expiry(contract: dict[str, Any]) -> float:
    if contract.get("expiry"):
        expiry = date.fromisoformat(str(contract["expiry"]))
        now = datetime.now(timezone.utc)
        close_utc = datetime(expiry.year, expiry.month, expiry.day, 21, 0, tzinfo=timezone.utc)
        return max(0.0, (close_utc - now).total_seconds() / 86400.0)
    return max(0.0, float(contract.get("days_to_expiry", 30.0)))


class SmartHedgeEngine:
    def __init__(
        self,
        config: dict[str, Any] | None = None,
        provider: MarketDataProvider | None = None,
        advisor: Advisor | None = None,
    ):
        self.config = config or load_config()
        self.provider = provider or build_provider(self.config)
        self.advisor = advisor or build_advisor(self.config)
        db_path = resolve_project_path(
            self.config, str(self.config.get("storage", {}).get("sqlite_path", ".smart_hedge/decisions.sqlite3"))
        )
        self.store = DecisionStore(db_path)

    def contract_for(self, symbol: str, overrides: dict[str, Any] | None = None) -> dict[str, Any]:
        normalized = symbol.upper()
        configured = self.config.get("contracts", {})
        if normalized not in configured:
            raise KeyError(
                f"no contract configured for {normalized}; add contracts.{normalized} to the config"
            )
        contract = copy.deepcopy(configured[normalized])
        if overrides:
            contract.update(copy.deepcopy(overrides))
        contract["days_to_expiry"] = _days_to_expiry(contract)
        contract.pop("expiry", None)
        if str(contract.get("option_type", "call")) not in {"call", "put"}:
            raise ValueError("option_type must be call or put")
        if str(contract.get("exercise_style", "american")) not in {"american", "european"}:
            raise ValueError("exercise_style must be american or european")
        return contract

    def recommendation(
        self, symbol: str, contract_overrides: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        normalized = symbol.upper()
        snapshot = self.provider.snapshot(normalized)
        contract = self.contract_for(normalized, contract_overrides)
        # Optional ATM shorthand is resolved deterministically from the quote.
        strike = contract.get("strike")
        if isinstance(strike, str) and strike.upper() == "ATM":
            contract["strike"] = round(snapshot.quote.midpoint)
        contract["strike"] = float(contract["strike"])
        if not math.isfinite(contract["strike"]) or contract["strike"] <= 0:
            raise ValueError("strike must be positive")

        features = build_features(snapshot, self.config)
        core = run_core(self.config, contract, snapshot.quote.midpoint)

        fallback_reason = ""
        try:
            assessment = self.advisor.assess(snapshot, features, core, contract)
        except Exception as exc:
            if not bool(self.config.get("model", {}).get("fallback_to_heuristic", True)):
                raise
            fallback_reason = f"{type(exc).__name__}: {str(exc)[:300]}"
            assessment = HeuristicAdvisor().assess(snapshot, features, core, contract)
            assessment.fallback_reason = fallback_reason

        policy = evaluate_policy(self.config, snapshot, features, core, assessment)
        decision_id = str(uuid.uuid4())
        created_at = utc_now_iso()

        snapshot_dict = asdict(snapshot)
        features_dict = asdict(features)
        assessment_dict = asdict(assessment)
        policy_dict = asdict(policy)
        core_binary = resolve_binary(self.config)
        audit = {
            "engine_version": ENGINE_VERSION,
            "policy_version": POLICY_VERSION,
            "input_hash": _canonical_hash(
                {
                    "contract": contract,
                    "snapshot": snapshot_dict,
                    "features": features_dict,
                    "core": core,
                }
            ),
            "model_output_hash": _canonical_hash(assessment_dict),
            "core_binary_path": str(core_binary),
            "core_binary_sha256": _file_hash(core_binary),
            "fallback_used": bool(fallback_reason),
            "fallback_reason": fallback_reason,
            "secrets_sent_to_model": False,
            "broker_order_endpoint_present": False,
        }
        recommendation = Recommendation(
            decision_id=decision_id,
            created_at=created_at,
            mode="paper",
            symbol=normalized,
            contract=contract,
            snapshot=snapshot,
            features=features,
            deterministic_core=core,
            model_assessment=assessment,
            policy=policy,
            audit=audit,
        )
        payload = asdict(recommendation)
        content_hash = self.store.append(payload)
        # The hash is stored beside the immutable JSON row. It is not inserted
        # into the row itself because that would make the hash self-referential.
        payload["audit"]["decision_store_content_hash"] = content_hash
        return payload

    def replay(self, decision_id: str) -> dict[str, Any]:
        payload = self.store.get(decision_id)
        if payload is None:
            raise KeyError(f"decision not found: {decision_id}")
        payload.setdefault("audit", {})["replay_mode"] = "stored_inputs_and_outputs_no_network"
        return payload

    def recent(self, limit: int = 20, symbol: str | None = None) -> list[dict[str, Any]]:
        return self.store.recent(limit=limit, symbol=symbol)

    def health(self) -> dict[str, Any]:
        return {
            "status": "ok",
            "mode": "paper",
            "engine_version": ENGINE_VERSION,
            "provider": type(self.provider).__name__,
            "advisor": type(self.advisor).__name__,
            "database": str(self.store.path),
            "broker_order_endpoint_present": False,
        }
