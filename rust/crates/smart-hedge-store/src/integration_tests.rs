use serde_json::json;

use crate::error::StoreError;
use crate::store::DecisionStore;

fn temp_store(name: &str) -> (DecisionStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("smart-hedge-store-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("decisions.sqlite3");
    let store = DecisionStore::new(&db_path).unwrap();
    (store, dir)
}

fn sample_payload(decision_id: &str, symbol: &str, action: &str) -> serde_json::Value {
    json!({
        "decision_id": decision_id,
        "created_at": "2026-07-19T00:00:00Z",
        "symbol": symbol,
        "policy": {"action": action},
        "model_assessment": {"advisor_kind": "heuristic"},
    })
}

/// SDH-HLR-060 / SDH-LLR-070 / SDH-LLR-071: appending stores a payload
/// with a content hash, and reading it back reverifies (not merely
/// repeats) that hash.
#[test]
fn append_then_get_round_trips_with_a_valid_hash() {
    let (store, dir) = temp_store("roundtrip");
    let payload = sample_payload("d1", "SPY", "hold_inside_effective_band");
    let hash = store.append(&payload).unwrap();
    assert_eq!(hash.len(), 64);

    let fetched = store.get("d1").unwrap().unwrap();
    assert_eq!(fetched["audit"]["stored_content_hash"], hash);
    assert_eq!(fetched["audit"]["stored_content_hash_valid"], true);
    assert_eq!(fetched["decision_id"], "d1");

    std::fs::remove_dir_all(&dir).ok();
}

/// SDH-HLR-060: replaying an unknown decision ID reports absence rather
/// than erroring.
#[test]
fn get_of_unknown_decision_id_is_none() {
    let (store, dir) = temp_store("unknown");
    assert!(store.get("never-existed").unwrap().is_none());
    std::fs::remove_dir_all(&dir).ok();
}

/// SDH-LLR-072: if the stored row is tampered with directly (bypassing
/// `append`), `get` must detect the mismatch rather than trust the
/// stored hash.
#[test]
fn tampered_payload_is_detected_on_replay() {
    let (store, dir) = temp_store("tamper");
    let payload = sample_payload("d1", "SPY", "hold_inside_effective_band");
    store.append(&payload).unwrap();

    // Directly corrupt the stored JSON, bypassing `append` entirely —
    // simulating accidental (or malicious) database mutation.
    let conn = rusqlite::Connection::open(store.path()).unwrap();
    conn.execute(
        "UPDATE decisions SET payload_json = ?1 WHERE decision_id = 'd1'",
        [r#"{"decision_id":"d1","created_at":"2026-07-19T00:00:00Z","symbol":"SPY","policy":{"action":"TAMPERED"},"model_assessment":{"advisor_kind":"heuristic"}}"#],
    )
    .unwrap();

    let fetched = store.get("d1").unwrap().unwrap();
    assert_eq!(fetched["audit"]["stored_content_hash_valid"], false);

    std::fs::remove_dir_all(&dir).ok();
}

/// `recent` clamps its limit to `[1, 200]`, matching Python's
/// `max(1, min(200, int(limit)))`.
#[test]
fn recent_clamps_limit_to_the_valid_range() {
    let (store, dir) = temp_store("clamp");
    for i in 0..5 {
        store.append(&sample_payload(&format!("d{i}"), "SPY", "hold_inside_effective_band")).unwrap();
    }
    assert_eq!(store.recent(0, None).unwrap().len(), 1); // clamped up to 1
    assert_eq!(store.recent(3, None).unwrap().len(), 3);
    assert_eq!(store.recent(1000, None).unwrap().len(), 5); // fewer rows than the 200 cap
    std::fs::remove_dir_all(&dir).ok();
}

/// `recent` filters by symbol, case-insensitively (normalized to
/// uppercase, matching Python's `symbol.upper()`).
#[test]
fn recent_filters_by_symbol_case_insensitively() {
    let (store, dir) = temp_store("symbolfilter");
    store.append(&sample_payload("d1", "SPY", "hold_inside_effective_band")).unwrap();
    store.append(&sample_payload("d2", "QQQ", "hold_inside_effective_band")).unwrap();

    let spy_only = store.recent(20, Some("spy")).unwrap();
    assert_eq!(spy_only.len(), 1);
    assert_eq!(spy_only[0]["symbol"], "SPY");

    std::fs::remove_dir_all(&dir).ok();
}

/// SDH-LLR-070/-071: a malformed payload (missing a field `append`
/// indexes directly) is rejected rather than partially inserted.
#[test]
fn append_rejects_a_payload_missing_a_required_field() {
    let (store, dir) = temp_store("malformed");
    let payload = json!({"created_at": "2026-07-19T00:00:00Z", "symbol": "SPY"}); // no decision_id
    let result = store.append(&payload);
    assert!(matches!(result, Err(StoreError::MalformedPayload(_))));
    std::fs::remove_dir_all(&dir).ok();
}

/// Reopening the same database path (a fresh `DecisionStore::new` call)
/// must not fail or duplicate schema objects — `CREATE TABLE/INDEX IF NOT
/// EXISTS` must actually be idempotent across process restarts, not just
/// within one connection's lifetime.
#[test]
fn reopening_an_existing_database_is_idempotent() {
    let (store, dir) = temp_store("reopen");
    store.append(&sample_payload("d1", "SPY", "hold_inside_effective_band")).unwrap();
    drop(store);

    let db_path = dir.join("decisions.sqlite3");
    let reopened = DecisionStore::new(&db_path).unwrap();
    assert!(reopened.get("d1").unwrap().is_some());

    std::fs::remove_dir_all(&dir).ok();
}
