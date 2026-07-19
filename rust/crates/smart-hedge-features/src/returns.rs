/// Log returns between consecutive closes: `ln(closes[i] / closes[i-1])`
/// for `i` in `1..closes.len()`.
pub fn log_returns(closes: &[f64]) -> Vec<f64> {
    (1..closes.len()).map(|i| (closes[i] / closes[i - 1]).ln()).collect()
}

/// Port of `horizon_return`: the fractional change from `window` bars ago
/// to the most recent close, or `None` if there isn't enough history
/// (`closes.len() <= window`, matching Python's `<=`, not `<`).
pub fn horizon_return(closes: &[f64], window: usize) -> Option<f64> {
    if closes.len() <= window {
        return None;
    }
    let last = *closes.last()?;
    let earlier = closes[closes.len() - 1 - window];
    Some(last / earlier - 1.0)
}

/// Port of the drawdown-from-rolling-peak calculation: the peak close
/// over the last `long_window` bars (or all of them if fewer exist —
/// Python's `closes[-long_window:]` doesn't error when `long_window` is
/// larger than `len(closes)`, it just returns the whole slice), and the
/// most recent close's fractional distance below it.
pub fn drawdown_from_rolling_peak(closes: &[f64], long_window: usize) -> Option<f64> {
    if closes.is_empty() {
        return None;
    }
    let start = closes.len().saturating_sub(long_window);
    let peak = closes[start..].iter().cloned().fold(f64::MIN, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let last = *closes.last()?;
    Some(last / peak - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_returns_of_fewer_than_two_closes_is_empty() {
        assert!(log_returns(&[]).is_empty());
        assert!(log_returns(&[100.0]).is_empty());
    }

    #[test]
    fn log_returns_matches_known_value() {
        let r = log_returns(&[100.0, 110.0]);
        assert_eq!(r.len(), 1);
        assert!((r[0] - (110.0_f64 / 100.0).ln()).abs() < 1e-12);
    }

    #[test]
    fn horizon_return_none_when_not_enough_history() {
        // len == window: Python's `<=` means this is still "not enough".
        assert_eq!(horizon_return(&[1.0, 2.0], 2), None);
        assert_eq!(horizon_return(&[1.0, 2.0], 5), None);
    }

    #[test]
    fn horizon_return_matches_known_value() {
        let closes = vec![100.0, 105.0, 110.0];
        // window=2: closes[-1]/closes[-1-2] - 1 = 110/100 - 1 = 0.10
        let result = horizon_return(&closes, 2).unwrap();
        assert!((result - 0.10).abs() < 1e-12);
    }

    #[test]
    fn drawdown_is_zero_at_a_new_high() {
        let closes = vec![90.0, 95.0, 100.0];
        let result = drawdown_from_rolling_peak(&closes, 90).unwrap();
        assert!((result - 0.0).abs() < 1e-12);
    }

    #[test]
    fn drawdown_reflects_decline_from_peak() {
        let closes = vec![100.0, 90.0];
        // peak=100, last=90: 90/100 - 1 = -0.10
        let result = drawdown_from_rolling_peak(&closes, 90).unwrap();
        assert!((result - (-0.10)).abs() < 1e-12);
    }

    #[test]
    fn drawdown_window_larger_than_history_uses_whole_history_without_panicking() {
        let closes = vec![50.0, 60.0, 55.0];
        // long_window (1000) far exceeds available history (3 closes);
        // Python's `closes[-1000:]` just returns the whole list.
        let result = drawdown_from_rolling_peak(&closes, 1000).unwrap();
        assert!((result - (55.0 / 60.0 - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn drawdown_of_empty_closes_is_none() {
        assert_eq!(drawdown_from_rolling_peak(&[], 90), None);
    }
}
