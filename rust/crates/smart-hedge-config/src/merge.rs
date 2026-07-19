use serde_json::Value;

/// Port of Python's `config._deep_merge`: recursively merges `over` onto
/// `base` in place. An object key present in both merges recursively; any
/// other value (including one side being non-object) is fully replaced by
/// `over`'s value — matching Python's `isinstance(value, dict) and
/// isinstance(result.get(key), dict)` branch exactly.
pub fn deep_merge(base: &mut Value, over: &Value) {
    let Value::Object(over_map) = over else {
        // Python's `_deep_merge` is only ever called with the top-level
        // config as `override`, which is validated to be an object before
        // this runs (see `loader::load_config`); a non-object here is a
        // caller bug, not user input, so this is intentionally a silent
        // no-op rather than a panic — there is nothing sensible to merge.
        return;
    };
    let Value::Object(base_map) = base else {
        *base = over.clone();
        return;
    };
    for (key, over_value) in over_map {
        match base_map.get_mut(key) {
            Some(base_value) if base_value.is_object() && over_value.is_object() => {
                deep_merge(base_value, over_value);
            }
            _ => {
                base_map.insert(key.clone(), over_value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merges_nested_objects_recursively() {
        let mut base = json!({"a": {"x": 1, "y": 2}, "b": 3});
        let over = json!({"a": {"y": 20}});
        deep_merge(&mut base, &over);
        assert_eq!(base, json!({"a": {"x": 1, "y": 20}, "b": 3}));
    }

    #[test]
    fn override_replaces_non_object_values_entirely() {
        let mut base = json!({"a": [1, 2, 3]});
        let over = json!({"a": [9]});
        deep_merge(&mut base, &over);
        assert_eq!(base, json!({"a": [9]}));
    }

    #[test]
    fn override_replaces_object_with_non_object() {
        let mut base = json!({"a": {"x": 1}});
        let over = json!({"a": "now a string"});
        deep_merge(&mut base, &over);
        assert_eq!(base, json!({"a": "now a string"}));
    }

    #[test]
    fn override_adds_new_keys() {
        let mut base = json!({"a": 1});
        let over = json!({"b": 2});
        deep_merge(&mut base, &over);
        assert_eq!(base, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn empty_override_leaves_base_unchanged() {
        let mut base = json!({"a": {"x": 1}});
        let original = base.clone();
        deep_merge(&mut base, &json!({}));
        assert_eq!(base, original);
    }

    #[test]
    fn contracts_map_merges_per_symbol() {
        let mut base = json!({"contracts": {"SPY": {"strike": 100.0, "contracts": 1}}});
        let over = json!({"contracts": {"SPY": {"strike": 105.0}, "QQQ": {"strike": 50.0}}});
        deep_merge(&mut base, &over);
        assert_eq!(
            base,
            json!({"contracts": {"SPY": {"strike": 105.0, "contracts": 1}, "QQQ": {"strike": 50.0}}})
        );
    }
}
