use std::path::Path;

use serde_json::Value;
use smart_hedge_config::{resolve_project_path, LoadedConfig};
use smart_hedge_models::{EvidenceItem, TimestampUtc};

/// A JSON value coerced to a display string the way Python's `str(x)`
/// would for the common cases this file format actually uses (string,
/// number, bool). Not a byte-perfect match for every possible JSON type
/// (e.g. Python's `str(None)` is `"None"`, not `"null"`) — evidence-file
/// authors are expected to supply strings for these fields; this is a
/// defensive fallback for a malformed file, not a correctness-critical
/// path. Verifies: SDH-LLR-124 (bounded, defensively typed fields).
fn stringify(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `row.get(key, default)` semantics: the default applies only when the
/// key is absent, not when it's present-but-falsy.
fn get_or_default(row: &serde_json::Map<String, Value>, key: &str, default: &str) -> String {
    row.get(key).map(stringify).unwrap_or_else(|| default.to_string())
}

/// `row.get(key) or fallback()` semantics: the fallback applies when the
/// key is absent *or* present with a falsy value (missing, `null`, or an
/// empty string) — matching Python's truthiness-based `or`, not just
/// presence.
fn get_or_falsy_fallback(row: &serde_json::Map<String, Value>, key: &str, fallback: impl FnOnce() -> String) -> String {
    match row.get(key) {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(v) if !matches!(v, Value::Null) && !matches!(v, Value::String(_)) => stringify(v),
        _ => fallback(),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Whether an evidence row's `symbols` field permits it to apply to
/// `symbol` — see `requirements/LLR.md` SDH-LLR-123 for the exact
/// (slightly non-obvious) three cases this replicates: an absent
/// `symbols` key, an explicitly empty list, and a list containing the
/// symbol or the `"*"` wildcard all count as "applies"; only a
/// *non-empty* list naming other symbols (and not `"*"`) excludes it.
fn row_applies_to_symbol(row: &serde_json::Map<String, Value>, symbol: &str) -> bool {
    let Some(Value::Array(applies)) = row.get("symbols") else {
        return true; // absent -> Python's `row.get("symbols", [symbol])` default trivially matches
    };
    if applies.is_empty() {
        return true; // present-but-empty is falsy in Python -> the `if applies and ...` guard short-circuits
    }
    let symbol_upper = symbol.to_uppercase();
    let has_wildcard = applies.iter().any(|v| v.as_str() == Some("*"));
    let has_symbol = applies.iter().any(|v| stringify(v).to_uppercase() == symbol_upper);
    has_wildcard || has_symbol
}

/// Port of `load_evidence_file`. Returns an empty list (never an error)
/// when the path is unconfigured, missing, or unparseable — verifies
/// SDH-LLR-125.
pub fn load_evidence_file(loaded: &LoadedConfig, symbol: &str) -> Vec<EvidenceItem> {
    let raw_path = loaded.config.provider.evidence_file.trim();
    if raw_path.is_empty() {
        return vec![];
    }
    let path = resolve_project_path(&loaded.config_dir, raw_path);
    if !path.exists() {
        return vec![];
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    let Ok(payload) = serde_json::from_str::<Value>(&text) else {
        return vec![];
    };

    let rows: &Vec<Value> = match &payload {
        Value::Object(map) => match map.get("evidence") {
            Some(Value::Array(arr)) => arr,
            _ => return vec![],
        },
        Value::Array(arr) => arr,
        _ => return vec![],
    };

    let file_name = path_file_name(&path);
    let mut output = Vec::new();
    for (index, row_value) in rows.iter().enumerate() {
        let Value::Object(row) = row_value else {
            continue;
        };
        if !row_applies_to_symbol(row, symbol) {
            continue;
        }
        let evidence_id = get_or_falsy_fallback(row, "evidence_id", || format!("file-{index}"));
        let timestamp = get_or_falsy_fallback(row, "timestamp", || TimestampUtc::now().to_iso_string());
        let quality = row
            .get("quality")
            .and_then(Value::as_f64)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        let untrusted_text = match row.get("untrusted_text") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Null) | None => true,
            Some(other) => !matches!(other, Value::Number(n) if n.as_f64() == Some(0.0)),
        };

        output.push(EvidenceItem {
            evidence_id,
            kind: get_or_default(row, "kind", "external"),
            title: truncate_chars(&get_or_default(row, "title", "Untitled evidence"), 240),
            timestamp,
            source: truncate_chars(&get_or_default(row, "source", &format!("file:{file_name}")), 120),
            value: row.get("value").cloned().unwrap_or(Value::Null),
            text: truncate_chars(&get_or_default(row, "text", ""), 5000),
            quality,
            untrusted_text,
        });
    }
    output
}

fn path_file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_hedge_config::EnvOverrides;
    use std::path::PathBuf;

    fn write_evidence_file(dir: &Path, contents: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("evidence.json");
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn loaded_config_with_evidence_file(evidence_path: &Path) -> LoadedConfig {
        let config_dir = evidence_path.parent().unwrap();
        let config_json = format!(
            r#"{{"provider": {{"evidence_file": "{}"}}}}"#,
            evidence_path.file_name().unwrap().to_string_lossy()
        );
        let config_path = config_dir.join("config.json");
        std::fs::write(&config_path, config_json).unwrap();
        smart_hedge_config::load_config(Some(&config_path), &EnvOverrides::default(), config_dir).unwrap()
    }

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("smart-hedge-data-test-{name}-{}", std::process::id()))
    }

    #[test]
    fn unconfigured_evidence_file_returns_empty() {
        let dir = temp_dir("unconfigured");
        std::fs::create_dir_all(&dir).unwrap();
        let loaded =
            smart_hedge_config::load_config(None, &EnvOverrides::default(), &dir).unwrap();
        assert!(load_evidence_file(&loaded, "SPY").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_returns_empty_not_an_error() {
        let dir = temp_dir("missing");
        std::fs::create_dir_all(&dir).unwrap();
        let evidence_path = dir.join("does-not-exist.json");
        let loaded = loaded_config_with_evidence_file(&evidence_path);
        assert!(load_evidence_file(&loaded, "SPY").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_json_returns_empty_not_an_error() {
        let dir = temp_dir("invalidjson");
        let path = write_evidence_file(&dir, "{not valid json");
        let loaded = loaded_config_with_evidence_file(&path);
        assert!(load_evidence_file(&loaded, "SPY").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn absent_symbols_field_always_applies() {
        let dir = temp_dir("absentsymbols");
        let path = write_evidence_file(
            &dir,
            r#"{"evidence": [{"evidence_id": "e1", "title": "General note"}]}"#,
        );
        let loaded = loaded_config_with_evidence_file(&path);
        assert_eq!(load_evidence_file(&loaded, "SPY").len(), 1);
        assert_eq!(load_evidence_file(&loaded, "QQQ").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_symbols_list_always_applies() {
        let dir = temp_dir("emptysymbols");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"symbols": [], "title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        assert_eq!(load_evidence_file(&loaded, "SPY").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_matching_symbols_list_excludes_the_item() {
        let dir = temp_dir("nonmatching");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"symbols": ["AAPL"], "title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        assert!(load_evidence_file(&loaded, "SPY").is_empty());
        assert_eq!(load_evidence_file(&loaded, "AAPL").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wildcard_symbol_always_applies() {
        let dir = temp_dir("wildcard");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"symbols": ["*"], "title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        assert_eq!(load_evidence_file(&loaded, "SPY").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn symbol_match_is_case_insensitive() {
        let dir = temp_dir("caseinsensitive");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"symbols": ["spy"], "title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        assert_eq!(load_evidence_file(&loaded, "SPY").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn quality_is_clamped_to_zero_one() {
        let dir = temp_dir("qualityclamp");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"title": "x", "quality": 5.0}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert_eq!(items[0].quality, 1.0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_quality_defaults_to_half() {
        let dir = temp_dir("qualitydefault");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert_eq!(items[0].quality, 0.5);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn untrusted_text_defaults_to_true() {
        let dir = temp_dir("untrusteddefault");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"title": "x"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert!(items[0].untrusted_text);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explicit_untrusted_text_false_is_honored() {
        let dir = temp_dir("untrustedfalse");
        let path = write_evidence_file(&dir, r#"{"evidence": [{"title": "x", "untrusted_text": false}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert!(!items[0].untrusted_text);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn title_is_truncated_to_240_characters() {
        let dir = temp_dir("titletrunc");
        let long_title = "x".repeat(500);
        let path = write_evidence_file(&dir, &format!(r#"{{"evidence": [{{"title": "{long_title}"}}]}}"#));
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert_eq!(items[0].title.chars().count(), 240);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_object_rows_are_skipped_not_erroring() {
        let dir = temp_dir("nonobjectrow");
        let path = write_evidence_file(&dir, r#"{"evidence": ["not-an-object", {"title": "valid"}]}"#);
        let loaded = loaded_config_with_evidence_file(&path);
        let items = load_evidence_file(&loaded, "SPY");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "valid");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn bare_array_payload_is_also_accepted() {
        let dir = temp_dir("barearray");
        let path = write_evidence_file(&dir, r#"[{"title": "x"}]"#);
        let loaded = loaded_config_with_evidence_file(&path);
        assert_eq!(load_evidence_file(&loaded, "SPY").len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
