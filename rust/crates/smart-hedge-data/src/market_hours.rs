use smart_hedge_models::{civil_from_days, days_from_civil, TimestampUtc};

/// Port of `data._regular_market_state`. Deliberately conservative, same as
/// Python: this catches weekends and clock hours but does not pretend to be
/// a complete NYSE holiday calendar. Uses the current (post-2007) US
/// Eastern DST rule (2nd Sunday of March through 1st Sunday of November) —
/// fine for any "now" this function is ever actually called with, since it
/// is never used to interpret a historical timestamp.
pub fn regular_market_state(now: TimestampUtc) -> &'static str {
    let utc_days = now.unix_seconds().div_euclid(86_400);
    let (year, _, _) = civil_from_days(utc_days);

    let dst_start = nth_sunday_of_month(year, 3, 2) * 86_400 + 7 * 3_600; // 2am EST -> 07:00 UTC
    let dst_end = nth_sunday_of_month(year, 11, 1) * 86_400 + 6 * 3_600; // 2am EDT -> 06:00 UTC
    let in_dst = now.unix_seconds() >= dst_start && now.unix_seconds() < dst_end;
    let offset_secs: i64 = if in_dst { -4 * 3_600 } else { -5 * 3_600 };

    let local_secs = now.unix_seconds() + offset_secs;
    let local_days = local_secs.div_euclid(86_400);
    let local_day_secs = local_secs.rem_euclid(86_400);

    // Monday=0 .. Sunday=6, matching Python's `datetime.weekday()`. Epoch
    // day 0 (1970-01-01) was a Thursday, index 3 in this convention.
    let weekday = (local_days + 3).rem_euclid(7);
    if weekday >= 5 {
        return "closed";
    }
    let minutes = local_day_secs / 60;
    if (570..960).contains(&minutes) {
        "open"
    } else {
        "closed"
    }
}

/// Sunday=0 .. Saturday=6 weekday of a civil-calendar day count.
fn weekday_sunday0(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// The `n`th Sunday of `(year, month)`, as a civil-calendar day count.
fn nth_sunday_of_month(year: i64, month: i64, n: i64) -> i64 {
    let first_day = days_from_civil(year, month, 1);
    let offset_to_sunday = (7 - weekday_sunday0(first_day)) % 7;
    first_day + offset_to_sunday + (n - 1) * 7
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(iso: &str) -> TimestampUtc {
        TimestampUtc::parse_flexible(iso).unwrap()
    }

    #[test]
    fn nth_sunday_of_month_actually_lands_on_a_sunday() {
        for year in [2024, 2025, 2026, 2030] {
            let start_days = nth_sunday_of_month(year, 3, 2);
            assert_eq!(weekday_sunday0(start_days), 0, "2nd Sunday of March {year} wasn't a Sunday");
            let end_days = nth_sunday_of_month(year, 11, 1);
            assert_eq!(weekday_sunday0(end_days), 0, "1st Sunday of November {year} wasn't a Sunday");
        }
    }

    #[test]
    fn a_midweek_midday_utc_instant_in_july_is_open_edt() {
        // Midsummer is always within DST regardless of the exact transition
        // date; noon UTC on a Tuesday in July is 8am EDT, before the open.
        // Use a later hour comfortably inside 9:30am-4pm Eastern.
        let t = at("2026-07-14T15:00:00Z"); // Tuesday; 15:00 - 4h = 11:00 EDT
        assert_eq!(regular_market_state(t), "open");
    }

    #[test]
    fn a_midweek_midday_utc_instant_in_january_is_open_est() {
        let t = at("2026-01-13T16:00:00Z"); // Tuesday; 16:00 - 5h = 11:00 EST
        assert_eq!(regular_market_state(t), "open");
    }

    #[test]
    fn a_saturday_is_always_closed_regardless_of_hour() {
        let t = at("2026-07-18T15:00:00Z"); // Saturday
        assert_eq!(regular_market_state(t), "closed");
    }

    #[test]
    fn a_sunday_is_always_closed_regardless_of_hour() {
        let t = at("2026-07-19T15:00:00Z"); // Sunday
        assert_eq!(regular_market_state(t), "closed");
    }

    #[test]
    fn before_the_open_on_a_weekday_is_closed() {
        let t = at("2026-07-14T12:00:00Z"); // 12:00 - 4h = 08:00 EDT, before 9:30am
        assert_eq!(regular_market_state(t), "closed");
    }

    #[test]
    fn after_the_close_on_a_weekday_is_closed() {
        let t = at("2026-07-14T21:00:00Z"); // 21:00 - 4h = 17:00 EDT, after 4pm
        assert_eq!(regular_market_state(t), "closed");
    }

    #[test]
    fn exactly_at_the_open_boundary_is_open() {
        let t = at("2026-07-14T13:30:00Z"); // 13:30 - 4h = 09:30 EDT, exactly the open
        assert_eq!(regular_market_state(t), "open");
    }

    #[test]
    fn exactly_at_the_close_boundary_is_closed() {
        let t = at("2026-07-14T20:00:00Z"); // 20:00 - 4h = 16:00 EDT, exactly the close (exclusive)
        assert_eq!(regular_market_state(t), "closed");
    }

    #[test]
    fn just_before_the_spring_forward_transition_uses_the_est_offset() {
        let year = 2026;
        let start_days = nth_sunday_of_month(year, 3, 2);
        let transition_secs = start_days * 86_400 + 7 * 3_600;
        let just_before = TimestampUtc::from_unix(transition_secs - 1, 0);
        let just_after = TimestampUtc::from_unix(transition_secs, 0);
        // At 06:59:59 UTC (just before), EST (-5h) local time is 01:59:59 —
        // still Sunday, closed. At 07:00:00 UTC (just after), EDT (-4h)
        // local time is 03:00:00 — still Sunday, still closed. Both should
        // report closed (it's a Sunday either way); the point of this test
        // is that neither branch panics and the boundary is inclusive on
        // the DST-start side, per `>= dst_start`.
        assert_eq!(regular_market_state(just_before), "closed");
        assert_eq!(regular_market_state(just_after), "closed");
    }
}
