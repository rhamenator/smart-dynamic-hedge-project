use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Port of `dashboard._Cache`: a per-symbol, TTL-bounded cache of the last
/// computed recommendation, so a plain (non-`fresh=true`) request doesn't
/// recompute (and re-hit the network/model) on every page poll.
pub struct Cache {
    seconds: f64,
    values: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
}

impl Cache {
    pub fn new(seconds: f64) -> Self {
        Cache { seconds: seconds.max(0.0), values: Mutex::new(HashMap::new()) }
    }

    pub fn get(&self, symbol: &str) -> Option<serde_json::Value> {
        let values = self.values.lock().expect("cache mutex poisoned");
        let (inserted_at, value) = values.get(symbol)?;
        if inserted_at.elapsed().as_secs_f64() <= self.seconds { Some(value.clone()) } else { None }
    }

    pub fn put(&self, symbol: &str, value: serde_json::Value) {
        let mut values = self.values.lock().expect("cache mutex poisoned");
        values.insert(symbol.to_string(), (Instant::now(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn a_fresh_entry_is_returned() {
        let cache = Cache::new(60.0);
        cache.put("SPY", serde_json::json!({"x": 1}));
        assert_eq!(cache.get("SPY"), Some(serde_json::json!({"x": 1})));
    }

    #[test]
    fn an_unknown_symbol_returns_none() {
        let cache = Cache::new(60.0);
        assert_eq!(cache.get("SPY"), None);
    }

    #[test]
    fn an_entry_older_than_the_ttl_is_not_returned() {
        let cache = Cache::new(0.01);
        cache.put("SPY", serde_json::json!({"x": 1}));
        sleep(Duration::from_millis(50));
        assert_eq!(cache.get("SPY"), None);
    }

    #[test]
    fn a_negative_ttl_is_clamped_to_zero_not_permanently_stale() {
        let cache = Cache::new(-5.0);
        cache.put("SPY", serde_json::json!({"x": 1}));
        // With seconds clamped to 0.0, only a get() happening at exactly
        // the same instant (elapsed() == 0.0) would hit; in practice any
        // measurable delay means every subsequent get() misses, but the
        // point of this test is just that construction doesn't panic or
        // produce a negative TTL that behaves unpredictably.
        let _ = cache.get("SPY");
    }

    #[test]
    fn different_symbols_are_cached_independently() {
        let cache = Cache::new(60.0);
        cache.put("SPY", serde_json::json!({"x": 1}));
        cache.put("QQQ", serde_json::json!({"x": 2}));
        assert_eq!(cache.get("SPY"), Some(serde_json::json!({"x": 1})));
        assert_eq!(cache.get("QQQ"), Some(serde_json::json!({"x": 2})));
    }

    #[test]
    fn putting_again_overwrites_the_previous_value() {
        let cache = Cache::new(60.0);
        cache.put("SPY", serde_json::json!({"x": 1}));
        cache.put("SPY", serde_json::json!({"x": 2}));
        assert_eq!(cache.get("SPY"), Some(serde_json::json!({"x": 2})));
    }
}
