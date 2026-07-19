use serde::{Deserialize, Serialize};

/// Port of `smart_hedge.models.Quote`. `midpoint`/`spread_bps` are computed
/// properties in Python; kept as methods here rather than stored fields for
/// the same reason — they must always reflect `bid`/`ask`/`last`, never a
/// stale cached value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    pub last: f64,
    pub timestamp: String,
    pub source: String,
    #[serde(default = "default_market_state")]
    pub market_state: String,
}

fn default_market_state() -> String {
    "unknown".to_string()
}

impl Quote {
    pub fn new(
        symbol: impl Into<String>,
        bid: f64,
        ask: f64,
        last: f64,
        timestamp: impl Into<String>,
        source: impl Into<String>,
        market_state: impl Into<String>,
    ) -> Self {
        Quote {
            symbol: symbol.into(),
            bid,
            ask,
            last,
            timestamp: timestamp.into(),
            source: source.into(),
            market_state: market_state.into(),
        }
    }

    /// `(bid + ask) / 2` when the quote looks sane, else falls back to
    /// `last` — matches Python's `Quote.midpoint` exactly, including the
    /// `bid > 0` and `ask >= bid` guards.
    pub fn midpoint(&self) -> f64 {
        if self.bid > 0.0 && self.ask >= self.bid {
            0.5 * (self.bid + self.ask)
        } else {
            self.last
        }
    }

    /// Spread in basis points of the midpoint; `f64::INFINITY` when the
    /// midpoint is non-positive or the book is crossed — matches Python's
    /// `Quote.spread_bps` (`float("inf")` in the same cases).
    pub fn spread_bps(&self) -> f64 {
        let mid = self.midpoint();
        if mid <= 0.0 || self.ask < self.bid {
            f64::INFINITY
        } else {
            (self.ask - self.bid) / mid * 10_000.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoint_averages_bid_and_ask() {
        let q = Quote::new("TEST", 99.0, 101.0, 100.0, "t", "src", "open");
        assert_eq!(q.midpoint(), 100.0);
    }

    #[test]
    fn midpoint_falls_back_to_last_when_bid_nonpositive() {
        let q = Quote::new("TEST", 0.0, 101.0, 100.0, "t", "src", "open");
        assert_eq!(q.midpoint(), 100.0);
    }

    #[test]
    fn midpoint_falls_back_to_last_when_crossed() {
        let q = Quote::new("TEST", 101.0, 99.0, 100.0, "t", "src", "open");
        assert_eq!(q.midpoint(), 100.0);
    }

    #[test]
    fn spread_bps_is_infinite_when_crossed() {
        let q = Quote::new("TEST", 101.0, 99.0, 100.0, "t", "src", "open");
        assert!(q.spread_bps().is_infinite());
    }

    #[test]
    fn spread_bps_is_infinite_when_midpoint_nonpositive() {
        let q = Quote::new("TEST", 0.0, 0.0, 0.0, "t", "src", "open");
        assert!(q.spread_bps().is_infinite());
    }

    #[test]
    fn spread_bps_matches_known_value() {
        let q = Quote::new("TEST", 99.99, 100.01, 100.0, "t", "src", "open");
        // (100.01 - 99.99) / 100.0 * 10_000 = 2.0
        assert!((q.spread_bps() - 2.0).abs() < 1e-9);
    }
}
