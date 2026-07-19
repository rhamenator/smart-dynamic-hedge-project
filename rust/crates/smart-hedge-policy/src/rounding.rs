/// Rounds to the nearest integer using round-half-to-even ("banker's
/// rounding"), matching Python's built-in `round()` for floats exactly —
/// Rust's `f64::round()` rounds half away from zero instead, which would
/// silently diverge from the Python behavior this crate is a parity port
/// of whenever `policy.allow_fractional_shares` is `false` and a preview
/// trade lands exactly on a half-share boundary (a real, not merely
/// theoretical, case: 0.5, 1.5, 2.5, ... are all exactly representable in
/// binary floating point).
pub fn round_half_to_even(x: f64) -> f64 {
    let floor = x.floor();
    let diff = x - floor;
    if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else {
        // Exactly 0.5: round to whichever neighbor is even. `%` on
        // negative integers in Rust can yield a negative remainder, so use
        // `rem_euclid` to get a correct 0/1 parity check for both signs.
        if (floor as i64).rem_euclid(2) == 0 {
            floor
        } else {
            floor + 1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_round_for_known_half_boundary_cases() {
        // Verified against CPython's `round()`: round(0.5)==0, round(1.5)==2,
        // round(2.5)==2, round(-0.5)==0, round(-1.5)==-2, round(-2.5)==-2.
        assert_eq!(round_half_to_even(0.5), 0.0);
        assert_eq!(round_half_to_even(1.5), 2.0);
        assert_eq!(round_half_to_even(2.5), 2.0);
        assert_eq!(round_half_to_even(3.5), 4.0);
        assert_eq!(round_half_to_even(-0.5), 0.0);
        assert_eq!(round_half_to_even(-1.5), -2.0);
        assert_eq!(round_half_to_even(-2.5), -2.0);
        assert_eq!(round_half_to_even(-3.5), -4.0);
    }

    #[test]
    fn non_half_values_round_normally() {
        assert_eq!(round_half_to_even(2.3), 2.0);
        assert_eq!(round_half_to_even(2.7), 3.0);
        assert_eq!(round_half_to_even(-2.3), -2.0);
        assert_eq!(round_half_to_even(-2.7), -3.0);
    }

    #[test]
    fn integers_are_unchanged() {
        for x in [-3.0, -1.0, 0.0, 1.0, 4.0] {
            assert_eq!(round_half_to_even(x), x);
        }
    }
}
