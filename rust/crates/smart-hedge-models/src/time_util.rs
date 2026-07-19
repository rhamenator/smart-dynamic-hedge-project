//! UTC timestamp helpers matching the exact leniency of Python's
//! `smart_hedge.models.utc_now_iso` / `smart_hedge.policy._parse_time`.
//!
//! This intentionally duplicates (in spirit, not in code) the hand-rolled
//! timestamp approach in `market-intelligence-mcp`'s
//! `market_intelligence_core::utc_timestamp` rather than depending on it
//! across repositories — the two have different leniency requirements
//! (this one must accept naive/offset-less timestamps and treat them as
//! UTC, matching Python's `datetime.fromisoformat` + manual UTC fallback;
//! that one is strict RFC 3339 UTC-only by the shared schema's design). If
//! a third copy of this logic is ever needed, that's the signal to factor
//! a shared crate into `market-system-contracts` instead of a fourth copy.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC instant: seconds since the Unix epoch plus a nanosecond remainder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimestampUtc {
    secs: i64,
    nanos: u32,
}

impl TimestampUtc {
    pub fn now() -> Self {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        TimestampUtc { secs: dur.as_secs() as i64, nanos: dur.subsec_nanos() }
    }

    pub fn unix_seconds(&self) -> i64 {
        self.secs
    }

    /// Seconds from `self` to `other` (`other - self`), as a signed f64 so
    /// callers can detect a timestamp in the future (negative) the same way
    /// Python's `total_seconds()` does, without saturating.
    pub fn seconds_until(&self, other: &Self) -> f64 {
        (other.secs - self.secs) as f64 + (other.nanos as f64 - self.nanos as f64) / 1e9
    }

    /// Formats as `YYYY-MM-DDTHH:MM:SS.ffffffZ` (always 6-digit
    /// microseconds, always UTC) — matches
    /// `datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")` in
    /// the common case where microseconds are nonzero. Python omits the
    /// fractional part entirely when microseconds happen to be exactly
    /// zero; this always includes it, which is a deliberate, documented
    /// deviation (harmless for every consumer in this codebase, all of
    /// which parse rather than string-compare timestamps).
    pub fn to_iso_string(&self) -> String {
        let micros = self.nanos / 1000;
        let days = self.secs.div_euclid(86_400);
        let day_secs = self.secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        let hour = day_secs / 3600;
        let minute = (day_secs % 3600) / 60;
        let second = day_secs % 60;
        format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}.{micros:06}Z")
    }

    /// Parses a timestamp with the same leniency as Python's
    /// `smart_hedge.policy._parse_time`: accepts a trailing `Z` (replaced
    /// with `+00:00`), an explicit `+HH:MM`/`-HH:MM` offset, or no offset at
    /// all (treated as already-UTC/naive, per the Python fallback
    /// `parsed.replace(tzinfo=timezone.utc)`). Returns `None` rather than
    /// erroring on any malformed input, matching
    /// `except (TypeError, ValueError): return None`.
    pub fn parse_flexible(input: &str) -> Option<Self> {
        let s = input.replace('Z', "+00:00");
        let bytes = s.as_bytes();
        if bytes.len() < 19 {
            return None;
        }

        fn digit(b: u8) -> Option<i64> {
            // `then_some` evaluates its argument eagerly even when the
            // condition is false, so `b - b'0'` would underflow-panic for
            // any non-digit byte below b'0' if written that way — use the
            // lazy `then` closure form instead.
            b.is_ascii_digit().then(|| (b - b'0') as i64)
        }
        fn two(b0: u8, b1: u8) -> Option<i64> {
            Some(digit(b0)? * 10 + digit(b1)?)
        }

        let year = digit(bytes[0])? * 1000 + digit(bytes[1])? * 100 + digit(bytes[2])? * 10 + digit(bytes[3])?;
        if bytes[4] != b'-' {
            return None;
        }
        let month = two(bytes[5], bytes[6])?;
        if bytes[7] != b'-' {
            return None;
        }
        let day = two(bytes[8], bytes[9])?;
        match bytes[10] {
            b'T' | b't' | b' ' => {}
            _ => return None,
        }
        let hour = two(bytes[11], bytes[12])?;
        if bytes[13] != b':' {
            return None;
        }
        let minute = two(bytes[14], bytes[15])?;
        if bytes[16] != b':' {
            return None;
        }
        let second = two(bytes[17], bytes[18])?;

        if !(1..=12).contains(&month) || day < 1 || day > last_day_of_month(year, month) {
            return None;
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }

        let mut idx = 19usize;
        let mut nanos: u32 = 0;
        if idx < bytes.len() && bytes[idx] == b'.' {
            idx += 1;
            let frac_start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            let frac_len = idx - frac_start;
            if frac_len == 0 || frac_len > 9 {
                return None;
            }
            let mut value: u32 = 0;
            for &b in &bytes[frac_start..idx] {
                value = value * 10 + (b - b'0') as u32;
            }
            for _ in 0..(9 - frac_len) {
                value *= 10;
            }
            nanos = value;
        }

        let mut offset_secs: i64 = 0;
        if idx < bytes.len() {
            let sign = match bytes[idx] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if bytes.len() < idx + 6 || bytes[idx + 3] != b':' {
                return None;
            }
            let oh = two(bytes[idx + 1], bytes[idx + 2])?;
            let om = two(bytes[idx + 4], bytes[idx + 5])?;
            if oh > 23 || om > 59 {
                return None;
            }
            offset_secs = sign * (oh * 3600 + om * 60);
            idx += 6;
        }
        if idx != bytes.len() {
            return None;
        }

        let days = days_from_civil(year, month, day);
        let local_secs = days.checked_mul(86_400)?.checked_add(hour * 3600 + minute * 60 + second)?;
        let utc_secs = local_secs.checked_sub(offset_secs)?;
        Some(TimestampUtc { secs: utc_secs, nanos })
    }
}

impl fmt::Display for TimestampUtc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso_string())
    }
}

fn is_leap_year(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn last_day_of_month(y: i64, m: i64) -> i64 {
    const DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if m == 2 && is_leap_year(y) { 29 } else { DAYS[(m - 1) as usize] }
}

/// Howard Hinnant's public-domain civil-calendar algorithm — see
/// `market-intelligence-mcp`'s `utc_timestamp` module for the derivation
/// and reference link; duplicated here per the module-level doc comment.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_z_suffix() {
        let t = TimestampUtc::parse_flexible("2026-07-19T14:30:00Z").unwrap();
        assert_eq!(t.unix_seconds(), TimestampUtc::parse_flexible("2026-07-19T14:30:00+00:00").unwrap().unix_seconds());
    }

    #[test]
    fn naive_timestamp_is_treated_as_utc() {
        let with_z = TimestampUtc::parse_flexible("2026-07-19T14:30:00Z").unwrap();
        let naive = TimestampUtc::parse_flexible("2026-07-19T14:30:00").unwrap();
        assert_eq!(with_z, naive);
    }

    #[test]
    fn nonzero_offset_converts_to_utc() {
        // 14:30 +05:00 is 09:30 UTC.
        let offset = TimestampUtc::parse_flexible("2026-07-19T14:30:00+05:00").unwrap();
        let utc = TimestampUtc::parse_flexible("2026-07-19T09:30:00Z").unwrap();
        assert_eq!(offset, utc);
    }

    #[test]
    fn negative_offset_converts_to_utc() {
        let offset = TimestampUtc::parse_flexible("2026-07-19T09:30:00-05:00").unwrap();
        let utc = TimestampUtc::parse_flexible("2026-07-19T14:30:00Z").unwrap();
        assert_eq!(offset, utc);
    }

    #[test]
    fn fractional_seconds_are_parsed() {
        let t = TimestampUtc::parse_flexible("2026-07-19T14:30:00.500000Z").unwrap();
        let t2 = TimestampUtc::parse_flexible("2026-07-19T14:30:00Z").unwrap();
        assert!(t.seconds_until(&t2).abs() < 1.0);
        assert_eq!(t.seconds_until(&t), 0.0);
    }

    #[test]
    fn rejects_malformed_input_without_panicking() {
        for bad in ["", "not-a-timestamp", "2026-07-19", "2026-13-01T00:00:00Z", "2026-02-30T00:00:00Z"] {
            assert!(TimestampUtc::parse_flexible(bad).is_none(), "expected None for {bad:?}");
        }
    }

    #[test]
    fn to_iso_string_round_trips() {
        let original = "2026-07-19T14:30:00.123456Z";
        let parsed = TimestampUtc::parse_flexible(original).unwrap();
        let rendered = parsed.to_iso_string();
        let reparsed = TimestampUtc::parse_flexible(&rendered).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn seconds_until_is_positive_for_the_future_and_negative_for_the_past() {
        let earlier = TimestampUtc::parse_flexible("2026-07-19T00:00:00Z").unwrap();
        let later = TimestampUtc::parse_flexible("2026-07-19T00:01:00Z").unwrap();
        assert_eq!(earlier.seconds_until(&later), 60.0);
        assert_eq!(later.seconds_until(&earlier), -60.0);
    }

    /// Dependency-free fuzz-smoke test: random mutations of a valid
    /// timestamp must never panic the parser, only ever return `None` or a
    /// valid `TimestampUtc`.
    #[test]
    fn fuzz_smoke_mutated_timestamps_never_panic() {
        struct XorShift64(u64);
        impl XorShift64 {
            fn next(&mut self) -> u64 {
                let mut x = self.0;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.0 = x;
                x
            }
        }
        let mut rng = XorShift64(0xBF58476D1CE4E5B9);
        let seed = "2026-07-19T14:30:00.123456+05:00";
        for _ in 0..20_000 {
            let mut chars: Vec<char> = seed.chars().collect();
            let mutations = 1 + (rng.next() % 6) as usize;
            for _ in 0..mutations {
                let idx = (rng.next() as usize) % chars.len();
                chars[idx] = (b'!' + (rng.next() % 90) as u8) as char;
            }
            let mutated: String = chars.into_iter().collect();
            let _ = TimestampUtc::parse_flexible(&mutated);
        }
    }
}
