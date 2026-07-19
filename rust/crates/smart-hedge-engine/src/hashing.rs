use std::path::Path;

use serde_json::Value;
use smart_hedge_models::sha256_hex;

/// Port of `engine._canonical_hash`. Reuses the same canonical-JSON
/// approach as `smart_hedge_store::canonical_json` — see that crate's doc
/// comment for why `serde_json::to_string` already produces sorted,
/// compact output without needing a custom serializer. Verifies:
/// SDH-LLR-133.
pub fn canonical_hash(value: &Value) -> String {
    let body = serde_json::to_string(value).expect("serde_json::Value serialization is infallible");
    sha256_hex(body.as_bytes())
}

/// Port of `engine._file_hash`: SHA-256 hex digest of a file's bytes, or
/// the literal string `"missing"` if the path doesn't exist or isn't a
/// regular file. Verifies: SDH-LLR-134.
pub fn file_hash(path: &Path) -> String {
    if !path.is_file() {
        return "missing".to_string();
    }
    match std::fs::read(path) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(_) => "missing".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_hash_is_deterministic_regardless_of_key_order() {
        let a = json!({"x": 1, "y": 2});
        let b = json!({"y": 2, "x": 1});
        assert_eq!(canonical_hash(&a), canonical_hash(&b));
    }

    #[test]
    fn canonical_hash_differs_for_different_values() {
        assert_ne!(canonical_hash(&json!({"x": 1})), canonical_hash(&json!({"x": 2})));
    }

    #[test]
    fn file_hash_of_a_missing_path_is_the_literal_missing() {
        assert_eq!(file_hash(Path::new("/definitely/does/not/exist")), "missing");
    }

    #[test]
    fn file_hash_of_a_directory_is_missing_not_an_error() {
        assert_eq!(file_hash(std::env::temp_dir().as_path()), "missing");
    }

    #[test]
    fn file_hash_of_a_real_file_is_its_sha256() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-engine-filehash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.bin");
        std::fs::write(&path, b"hello").unwrap();
        assert_eq!(file_hash(&path), sha256_hex(b"hello"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
