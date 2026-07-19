from __future__ import annotations

import hashlib
import json
import sqlite3
from pathlib import Path
from typing import Any


class DecisionStore:
    def __init__(self, path: str | Path):
        self.path = Path(path).expanduser().resolve()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._initialize()

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.path, timeout=10.0)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA journal_mode=WAL")
        connection.execute("PRAGMA foreign_keys=ON")
        return connection

    def _initialize(self) -> None:
        with self._connect() as db:
            db.execute(
                """
                CREATE TABLE IF NOT EXISTS decisions (
                    decision_id TEXT PRIMARY KEY,
                    created_at TEXT NOT NULL,
                    symbol TEXT NOT NULL,
                    action TEXT NOT NULL,
                    advisor_kind TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                )
                """
            )
            db.execute(
                "CREATE INDEX IF NOT EXISTS idx_decisions_created ON decisions(created_at DESC)"
            )
            db.execute(
                "CREATE INDEX IF NOT EXISTS idx_decisions_symbol ON decisions(symbol, created_at DESC)"
            )

    @staticmethod
    def canonical_json(payload: dict[str, Any]) -> str:
        return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)

    @staticmethod
    def hash_payload(payload: dict[str, Any]) -> str:
        return hashlib.sha256(DecisionStore.canonical_json(payload).encode("utf-8")).hexdigest()

    def append(self, payload: dict[str, Any]) -> str:
        body = self.canonical_json(payload)
        digest = hashlib.sha256(body.encode("utf-8")).hexdigest()
        with self._connect() as db:
            db.execute(
                """
                INSERT INTO decisions(
                    decision_id, created_at, symbol, action, advisor_kind, content_hash, payload_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    payload["decision_id"],
                    payload["created_at"],
                    payload["symbol"],
                    payload["policy"]["action"],
                    payload["model_assessment"]["advisor_kind"],
                    digest,
                    body,
                ),
            )
        return digest

    def get(self, decision_id: str) -> dict[str, Any] | None:
        with self._connect() as db:
            row = db.execute(
                "SELECT payload_json, content_hash FROM decisions WHERE decision_id = ?",
                (decision_id,),
            ).fetchone()
        if row is None:
            return None
        payload = json.loads(row["payload_json"])
        actual = self.hash_payload(payload)
        payload.setdefault("audit", {})["stored_content_hash"] = row["content_hash"]
        payload["audit"]["stored_content_hash_valid"] = actual == row["content_hash"]
        return payload

    def recent(self, limit: int = 20, symbol: str | None = None) -> list[dict[str, Any]]:
        limit = max(1, min(200, int(limit)))
        with self._connect() as db:
            if symbol:
                rows = db.execute(
                    """
                    SELECT payload_json FROM decisions
                    WHERE symbol = ? ORDER BY created_at DESC LIMIT ?
                    """,
                    (symbol.upper(), limit),
                ).fetchall()
            else:
                rows = db.execute(
                    "SELECT payload_json FROM decisions ORDER BY created_at DESC LIMIT ?",
                    (limit,),
                ).fetchall()
        return [json.loads(row["payload_json"]) for row in rows]
