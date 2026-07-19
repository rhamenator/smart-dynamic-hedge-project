use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::canonical::{canonical_json, hash_payload};
use crate::error::StoreError;
use crate::field_access::{nested_object, str_field};

/// Port of `smart_hedge.store.DecisionStore`. Opens a fresh connection per
/// operation (matching Python's `_connect()` context-manager pattern)
/// rather than holding one open — same concurrency behavior, not an
/// optimization opportunity left on the table by accident.
pub struct DecisionStore {
    path: PathBuf,
}

impl DecisionStore {
    /// Creates the parent directory if needed and initializes the schema.
    /// Verifies: SDH-HLR-060.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = DecisionStore { path };
        store.initialize()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn connect(&self) -> Result<Connection, StoreError> {
        let conn = Connection::open(&self.path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(conn)
    }

    fn initialize(&self) -> Result<(), StoreError> {
        let conn = self.connect()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS decisions (
                decision_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                symbol TEXT NOT NULL,
                action TEXT NOT NULL,
                advisor_kind TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                payload_json TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_decisions_created ON decisions(created_at DESC)", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_decisions_symbol ON decisions(symbol, created_at DESC)",
            [],
        )?;
        Ok(())
    }

    /// Port of `DecisionStore.append`. Returns the content hash.
    /// Verifies: SDH-LLR-070, SDH-LLR-071, SDH-LLR-073.
    pub fn append(&self, payload: &Value) -> Result<String, StoreError> {
        let body = canonical_json(payload);
        let digest = smart_hedge_models::sha256_hex(body.as_bytes());

        let decision_id = str_field(payload, "decision_id")?;
        let created_at = str_field(payload, "created_at")?;
        let symbol = str_field(payload, "symbol")?;
        let action = str_field(nested_object(payload, "policy")?, "action")?;
        let advisor_kind = str_field(nested_object(payload, "model_assessment")?, "advisor_kind")?;

        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO decisions(decision_id, created_at, symbol, action, advisor_kind, content_hash, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![decision_id, created_at, symbol, action, advisor_kind, digest, body],
        )?;
        Ok(digest)
    }

    /// Port of `DecisionStore.get`: reads a stored decision and
    /// independently reverifies its content hash rather than trusting the
    /// stored one blindly. Verifies: SDH-LLR-072.
    pub fn get(&self, decision_id: &str) -> Result<Option<Value>, StoreError> {
        let conn = self.connect()?;
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT payload_json, content_hash FROM decisions WHERE decision_id = ?1",
                params![decision_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((payload_json, stored_hash)) = row else {
            return Ok(None);
        };

        let mut payload: Value =
            serde_json::from_str(&payload_json).map_err(|e| StoreError::InvalidJson(e.to_string()))?;
        let actual = hash_payload(&payload);

        let root = payload
            .as_object_mut()
            .ok_or_else(|| StoreError::MalformedPayload("stored payload root is not an object".to_string()))?;
        let audit = root.entry("audit").or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(audit_map) = audit {
            audit_map.insert("stored_content_hash".to_string(), Value::String(stored_hash.clone()));
            audit_map.insert("stored_content_hash_valid".to_string(), Value::Bool(actual == stored_hash));
        }
        Ok(Some(payload))
    }

    /// Port of `DecisionStore.recent`. `limit` is clamped to `[1, 200]`,
    /// matching Python's `max(1, min(200, int(limit)))`.
    pub fn recent(&self, limit: i64, symbol: Option<&str>) -> Result<Vec<Value>, StoreError> {
        let clamped_limit = limit.clamp(1, 200);
        let conn = self.connect()?;

        let rows: Vec<String> = if let Some(sym) = symbol {
            let mut stmt = conn.prepare(
                "SELECT payload_json FROM decisions WHERE symbol = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?;
            stmt.query_map(params![sym.to_uppercase(), clamped_limit], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt =
                conn.prepare("SELECT payload_json FROM decisions ORDER BY created_at DESC LIMIT ?1")?;
            stmt.query_map(params![clamped_limit], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?
        };

        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(|e| StoreError::InvalidJson(e.to_string())))
            .collect()
    }
}
