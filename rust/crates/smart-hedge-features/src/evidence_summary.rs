use std::collections::BTreeMap;

use serde_json::Value;
use smart_hedge_models::EvidenceItem;

/// Sanitizes an evidence title into a feature-map key: lowercase, every
/// non-alphanumeric character replaced with `_`, truncated to 64
/// *characters* (not bytes — matters for multi-byte UTF-8 titles), then
/// prefixed with `evidence_`. Matches Python's
/// `"evidence_" + "".join(ch if ch.isalnum() else "_" for ch in
/// item.title.lower())[:64]` — note the slice applies to the sanitized
/// string *before* the prefix is added, not to the whole key.
pub fn evidence_feature_key(title: &str) -> String {
    let sanitized: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(64)
        .collect();
    format!("evidence_{sanitized}")
}

/// Extracts (a) a map of `evidence_<sanitized-title>` → numeric value for
/// every evidence item whose `value` is a JSON number (explicitly
/// excluding JSON booleans, which are numeric-like but not numeric), and
/// (b) whether any item is a `kind == "event"` with `value == true`
/// (strict boolean `true`, not merely truthy).
///
/// When two items sanitize to the same key, the later item's value wins —
/// matching Python dict-assignment semantics (`evidence_numeric[key] =
/// ...` in a loop simply overwrites).
pub fn summarize(evidence: &[EvidenceItem]) -> (BTreeMap<String, f64>, bool) {
    let mut numeric = BTreeMap::new();
    let mut event_risk = false;
    for item in evidence {
        if item.kind == "event" && item.value == Value::Bool(true) {
            event_risk = true;
        }
        if let Value::Number(n) = &item.value
            && let Some(v) = n.as_f64()
        {
            numeric.insert(evidence_feature_key(&item.title), v);
        }
    }
    (numeric, event_risk)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &str, title: &str, value: Value) -> EvidenceItem {
        EvidenceItem {
            evidence_id: "id".to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            timestamp: "2026-07-19T00:00:00Z".to_string(),
            source: "test".to_string(),
            value,
            text: String::new(),
            quality: 0.5,
            untrusted_text: true,
        }
    }

    #[test]
    fn key_sanitizes_non_alphanumeric_and_lowercases() {
        assert_eq!(evidence_feature_key("Earnings: Q2 2026!"), "evidence_earnings__q2_2026_");
    }

    #[test]
    fn key_truncates_the_sanitized_body_to_64_chars_before_prefixing() {
        let long_title = "a".repeat(100);
        let key = evidence_feature_key(&long_title);
        // "evidence_" (9 chars) + 64 sanitized chars = 73, not 9 + 100.
        assert_eq!(key.len(), 9 + 64);
    }

    #[test]
    fn numeric_value_is_extracted() {
        let (numeric, _) = summarize(&[item("option_metric", "Realized Vol", Value::from(0.25))]);
        assert_eq!(numeric.get("evidence_realized_vol"), Some(&0.25));
    }

    #[test]
    fn boolean_value_is_not_treated_as_numeric() {
        let (numeric, _) = summarize(&[item("event", "Flag", Value::Bool(true))]);
        assert!(numeric.is_empty());
    }

    #[test]
    fn event_kind_with_true_value_sets_event_risk() {
        let (_, event_risk) = summarize(&[item("event", "Something happened", Value::Bool(true))]);
        assert!(event_risk);
    }

    #[test]
    fn event_kind_with_false_value_does_not_set_event_risk() {
        let (_, event_risk) = summarize(&[item("event", "Nothing happened", Value::Bool(false))]);
        assert!(!event_risk);
    }

    #[test]
    fn non_event_kind_never_sets_event_risk_even_with_true_value() {
        let (_, event_risk) = summarize(&[item("news", "Not an event", Value::Bool(true))]);
        assert!(!event_risk);
    }

    #[test]
    fn later_item_with_colliding_key_overwrites_earlier_one() {
        let (numeric, _) = summarize(&[
            item("option_metric", "Vol", Value::from(0.10)),
            item("option_metric", "Vol", Value::from(0.20)),
        ]);
        assert_eq!(numeric.get("evidence_vol"), Some(&0.20));
    }
}
