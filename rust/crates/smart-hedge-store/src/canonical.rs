use serde_json::Value;
use smart_hedge_models::sha256_hex;

/// Port of `smart_hedge.store.DecisionStore.canonical_json`: sorted object
/// keys, compact separators, non-ASCII preserved rather than
/// `\uXXXX`-escaped.
///
/// This relies on `serde_json::Value`'s `Object` variant being backed by a
/// `BTreeMap` (true as long as the `preserve_order` feature is not enabled
/// anywhere in the dependency graph — see the workspace `Cargo.toml`,
/// which does not enable it), so `serde_json::to_string` already emits
/// object keys in sorted order and uses compact, non-escaping-non-ASCII
/// output by default. `sort-keys::tests` locks this in with a direct test
/// rather than leaving it as an unverified assumption about a dependency's
/// default configuration.
///
/// Verifies: SDH-LLR-070.
pub fn canonical_json(payload: &Value) -> String {
    serde_json::to_string(payload).expect("serde_json::Value serialization is infallible")
}

/// Port of `DecisionStore.hash_payload`: SHA-256 hex digest of the
/// canonical JSON serialization. Verifies: SDH-LLR-071.
///
/// Note: this is **not** guaranteed to match the hash Python's
/// `hash_payload` would produce for the same logical payload — float
/// formatting and other serialization details can differ between
/// `json.dumps` and `serde_json`. That's fine: a Rust-created record's
/// hash only ever needs to be re-verifiable by this same Rust
/// implementation on replay (SDH-LLR-072), not cross-checked byte-for-byte
/// against a Python-created record.
pub fn hash_payload(payload: &Value) -> String {
    sha256_hex(canonical_json(payload).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_keys_are_sorted_regardless_of_insertion_order() {
        let payload = json!({"z": 1, "a": 2, "m": 3});
        assert_eq!(canonical_json(&payload), r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn separators_are_compact_with_no_extra_whitespace() {
        let payload = json!({"a": [1, 2], "b": "x"});
        let out = canonical_json(&payload);
        assert!(!out.contains(' '), "expected no whitespace, got {out:?}");
    }

    #[test]
    fn nested_objects_are_also_sorted() {
        let payload = json!({"outer": {"z": 1, "a": 2}});
        assert_eq!(canonical_json(&payload), r#"{"outer":{"a":2,"z":1}}"#);
    }

    #[test]
    fn hash_is_deterministic_for_the_same_logical_payload_regardless_of_construction_order() {
        let a = json!({"x": 1, "y": 2});
        let b = json!({"y": 2, "x": 1});
        assert_eq!(hash_payload(&a), hash_payload(&b));
    }

    #[test]
    fn hash_differs_for_different_payloads() {
        let a = json!({"x": 1});
        let b = json!({"x": 2});
        assert_ne!(hash_payload(&a), hash_payload(&b));
    }

    #[test]
    fn hash_is_a_64_character_lowercase_hex_string() {
        let hash = hash_payload(&json!({"a": 1}));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
