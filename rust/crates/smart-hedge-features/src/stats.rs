//! Hand-rolled statistics helpers matching Python's `statistics` module
//! functions this crate is a parity port of — no dependency added for
//! this, consistent with the workspace's dependency-minimization policy.

/// Matches `statistics.fmean`: arithmetic mean, `None` for an empty slice.
pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Matches `statistics.stdev`: *sample* standard deviation (Bessel's
/// correction, `n - 1` denominator), requiring at least 2 values.
pub fn sample_stdev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let m = mean(values)?;
    let variance =
        values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() as f64 - 1.0);
    Some(variance.sqrt())
}

/// Port of `_ewma_variance`: an exponentially weighted moving variance
/// seeded by the first squared return, recursed over the rest.
pub fn ewma_variance(returns: &[f64], decay: f64) -> Option<f64> {
    let (first, rest) = returns.split_first()?;
    let mut variance = first * first;
    for value in rest {
        variance = decay * variance + (1.0 - decay) * value * value;
    }
    Some(variance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_of_empty_is_none() {
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn mean_matches_known_value() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), Some(2.0));
    }

    #[test]
    fn sample_stdev_requires_at_least_two_values() {
        assert_eq!(sample_stdev(&[]), None);
        assert_eq!(sample_stdev(&[5.0]), None);
    }

    #[test]
    fn sample_stdev_matches_known_value() {
        // Population {2, 4, 4, 4, 5, 5, 7, 9} has sample stdev 2.13809...
        // (a commonly cited worked example); use a simpler exact case:
        // {0, 2}: mean=1, variance=(1+1)/1=2, stdev=sqrt(2).
        let result = sample_stdev(&[0.0, 2.0]).unwrap();
        assert!((result - std::f64::consts::SQRT_2).abs() < 1e-12);
    }

    #[test]
    fn ewma_variance_of_empty_is_none() {
        assert_eq!(ewma_variance(&[], 0.94), None);
    }

    #[test]
    fn ewma_variance_of_single_return_is_its_square() {
        assert_eq!(ewma_variance(&[0.02], 0.94), Some(0.0004));
    }

    #[test]
    fn ewma_variance_recurses_correctly() {
        let decay = 0.5;
        let v0: f64 = 0.02 * 0.02;
        let v1 = decay * v0 + (1.0 - decay) * 0.03 * 0.03;
        assert_eq!(ewma_variance(&[0.02, 0.03], decay), Some(v1));
    }
}
