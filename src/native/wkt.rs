//! Well-known types: calendar arithmetic and range rules for `google.protobuf.Timestamp`
//! and `google.protobuf.Duration`, independent of any host language.

/// `0001-01-01T00:00:00Z`, the smallest valid Timestamp.
pub const TIMESTAMP_MIN_SECONDS: i64 = -62_135_596_800;
/// `9999-12-31T23:59:59Z`, the largest valid Timestamp.
pub const TIMESTAMP_MAX_SECONDS: i64 = 253_402_300_799;
/// About ±10,000 years, the Duration range defined by `duration.proto`.
pub const DURATION_MAX_SECONDS: i64 = 315_576_000_000;

const MICROS_PER_SECOND: i128 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400_000_000;

/// A UTC calendar date and time of day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Days since 1970-01-01 for a proleptic Gregorian date (H. Hinnant's algorithm).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = i64::from((month + 9) % 12);
    let day_of_year = (153 * month_index + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Validate the `seconds`/`nanos` pair of a Timestamp.
pub fn check_timestamp(seconds: i64, nanos: i32) -> Result<(), String> {
    if !(TIMESTAMP_MIN_SECONDS..=TIMESTAMP_MAX_SECONDS).contains(&seconds) {
        return Err(format!("Timestamp seconds {seconds} out of range"));
    }
    if !(0..1_000_000_000).contains(&nanos) {
        return Err(format!("Timestamp nanos {nanos} out of range"));
    }
    Ok(())
}

/// UTC calendar fields of a valid Timestamp's seconds.
pub fn timestamp_to_civil(seconds: i64) -> Civil {
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year: year as i32,
        month: month as u8,
        day: day as u8,
        hour: (second_of_day / 3600) as u8,
        minute: (second_of_day % 3600 / 60) as u8,
        second: (second_of_day % 60) as u8,
    }
}

/// Timestamp `(seconds, nanos)` for UTC calendar fields, a microsecond and a UTC
/// offset (in microseconds, positive east of UTC).
pub fn civil_to_timestamp(civil: Civil, microsecond: u32, offset_micros: i64) -> (i64, i32) {
    let days = days_from_civil(i64::from(civil.year), civil.month.into(), civil.day.into());
    let seconds = days * 86_400
        + i64::from(civil.hour) * 3600
        + i64::from(civil.minute) * 60
        + i64::from(civil.second);
    let total = i128::from(seconds) * MICROS_PER_SECOND + i128::from(microsecond)
        - i128::from(offset_micros);
    let seconds = total.div_euclid(MICROS_PER_SECOND) as i64;
    let nanos = (total.rem_euclid(MICROS_PER_SECOND) * 1000) as i32;
    (seconds, nanos)
}

/// Validate the `seconds`/`nanos` pair of a Duration (same sign, bounded).
pub fn check_duration(seconds: i64, nanos: i32) -> Result<(), String> {
    if !(-DURATION_MAX_SECONDS..=DURATION_MAX_SECONDS).contains(&seconds) {
        return Err(format!("Duration seconds {seconds} out of range"));
    }
    if !(-999_999_999..=999_999_999).contains(&nanos) {
        return Err(format!("Duration nanos {nanos} out of range"));
    }
    if (seconds > 0 && nanos < 0) || (seconds < 0 && nanos > 0) {
        return Err("Duration seconds and nanos have different signs".into());
    }
    Ok(())
}

/// Split a Duration into `(days, seconds, microseconds)` with non-negative seconds and
/// microseconds, the normalized form of Python's `timedelta`. Sub-microsecond
/// precision is truncated toward zero.
pub fn duration_to_parts(seconds: i64, nanos: i32) -> (i64, i64, i64) {
    let total = seconds * 1_000_000 + i64::from(nanos / 1000);
    let days = total.div_euclid(MICROS_PER_DAY);
    let rest = total.rem_euclid(MICROS_PER_DAY);
    (days, rest / 1_000_000, rest % 1_000_000)
}

/// Duration `(seconds, nanos)` from timedelta parts. `seconds` and `nanos` share the
/// sign of the whole duration, as `duration.proto` requires.
pub fn duration_from_parts(days: i64, seconds: i64, microseconds: i64) -> (i64, i32) {
    let total = i128::from(days) * i128::from(MICROS_PER_DAY)
        + i128::from(seconds) * MICROS_PER_SECOND
        + i128::from(microseconds);
    // Truncating division keeps both parts on the same side of zero.
    let seconds = (total / MICROS_PER_SECOND) as i64;
    let nanos = ((total % MICROS_PER_SECOND) * 1000) as i32;
    (seconds, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trips_across_the_whole_range() {
        for days in (-719_162..=2_932_896).step_by(997) {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-719_162), (1, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn timestamp_bounds_map_to_calendar_bounds() {
        let min = timestamp_to_civil(TIMESTAMP_MIN_SECONDS);
        assert_eq!((min.year, min.month, min.day, min.hour), (1, 1, 1, 0));
        let max = timestamp_to_civil(TIMESTAMP_MAX_SECONDS);
        assert_eq!(
            (
                max.year, max.month, max.day, max.hour, max.minute, max.second
            ),
            (9999, 12, 31, 23, 59, 59)
        );
        assert!(check_timestamp(TIMESTAMP_MAX_SECONDS + 1, 0).is_err());
        assert!(check_timestamp(0, 1_000_000_000).is_err());
    }

    #[test]
    fn civil_to_timestamp_applies_offset_and_microseconds() {
        let civil = Civil {
            year: 2024,
            month: 1,
            day: 1,
            hour: 1,
            minute: 0,
            second: 0,
        };
        // 01:00 at UTC+01:00 is midnight UTC.
        assert_eq!(
            civil_to_timestamp(civil, 5, 3_600_000_000),
            (1_704_067_200, 5000)
        );
        // Before the epoch, nanos stay non-negative.
        let before = Civil {
            year: 1969,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59,
        };
        assert_eq!(civil_to_timestamp(before, 500_000, 0), (-1, 500_000_000));
    }

    #[test]
    fn durations_keep_one_sign() {
        assert_eq!(duration_from_parts(-1, 86_399, 500_000), (0, -500_000_000));
        assert_eq!(duration_to_parts(0, -500_000_000), (-1, 86_399, 500_000));
        assert_eq!(duration_from_parts(1, 1, 1), (86_401, 1000));
        assert!(check_duration(1, -1).is_err());
        assert!(check_duration(DURATION_MAX_SECONDS + 1, 0).is_err());
    }
}
