use serde_json::Value;

use crate::error::StoreError;

/// Reads a required string field from a JSON object, matching Python's
/// `payload["field"]` (a `KeyError`/`TypeError` there becomes a
/// `StoreError::MalformedPayload` here — both fail the operation rather
/// than silently substituting a default).
pub fn str_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, StoreError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::MalformedPayload(format!("missing or non-string field '{key}'")))
}

/// Reads a required nested object field (e.g. `payload["policy"]`),
/// erroring if it's absent or not an object.
pub fn nested_object<'a>(value: &'a Value, key: &str) -> Result<&'a Value, StoreError> {
    match value.get(key) {
        Some(v) if v.is_object() => Ok(v),
        _ => Err(StoreError::MalformedPayload(format!("missing or non-object field '{key}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn str_field_reads_a_present_string() {
        let v = json!({"a": "hello"});
        assert_eq!(str_field(&v, "a").unwrap(), "hello");
    }

    #[test]
    fn str_field_errors_on_missing_key() {
        let v = json!({});
        assert!(matches!(str_field(&v, "a"), Err(StoreError::MalformedPayload(_))));
    }

    #[test]
    fn str_field_errors_on_wrong_type() {
        let v = json!({"a": 5});
        assert!(matches!(str_field(&v, "a"), Err(StoreError::MalformedPayload(_))));
    }

    #[test]
    fn nested_object_errors_on_missing_key() {
        let v = json!({});
        assert!(matches!(nested_object(&v, "policy"), Err(StoreError::MalformedPayload(_))));
    }

    #[test]
    fn nested_object_errors_when_not_an_object() {
        let v = json!({"policy": "not-an-object"});
        assert!(matches!(nested_object(&v, "policy"), Err(StoreError::MalformedPayload(_))));
    }

    #[test]
    fn nested_object_returns_the_value_when_present() {
        let v = json!({"policy": {"action": "hold"}});
        let result = nested_object(&v, "policy").unwrap();
        assert_eq!(str_field(result, "action").unwrap(), "hold");
    }
}
