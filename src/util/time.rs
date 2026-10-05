use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};

/// Returns the next Sunday at `rollover_hour` UTC.
/// If today is Sunday and we haven't passed rollover_hour yet, returns today at that hour.
/// Otherwise returns the following Sunday.
pub fn get_period_end_time(rollover_hour: u32) -> DateTime<Utc> {
    period_end_after(Utc::now(), rollover_hour)
}

fn period_end_after(now: DateTime<Utc>, rollover_hour: u32) -> DateTime<Utc> {
    let days_from_sunday = now.weekday().num_days_from_sunday() as i64;
    let days_offset = if days_from_sunday == 0 {
        0
    } else {
        7 - days_from_sunday
    };

    let candidate = Utc.from_utc_datetime(
        &(now + Duration::days(days_offset))
            .date_naive()
            .and_hms_opt(rollover_hour, 0, 0)
            .unwrap(),
    );

    if candidate <= now {
        candidate + Duration::days(7)
    } else {
        candidate
    }
}

/// Returns the start of a period given its end time (exactly 7 days before).
pub fn get_period_start_time(end_time: &DateTime<Utc>) -> DateTime<Utc> {
    *end_time - Duration::days(7)
}

/// Get the start and end times for the current weekly period using a configurable rollover hour.
pub fn get_weekly_period_bounds_with_hour(rollover_hour: u32) -> (DateTime<Utc>, DateTime<Utc>) {
    let end = get_period_end_time(rollover_hour);
    let start = get_period_start_time(&end);
    (start, end)
}

/// Format a datetime for storage in SQLite
pub fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Parse a datetime from SQLite storage
pub fn parse_datetime(s: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    // 2025-06-01 and 2025-06-08 are Sundays
    #[test]
    fn period_end_midweek_is_coming_sunday() {
        assert_eq!(
            period_end_after(utc(2025, 6, 4, 9, 0), 12),
            utc(2025, 6, 8, 12, 0)
        );
    }

    #[test]
    fn period_end_sunday_before_rollover_is_today() {
        assert_eq!(
            period_end_after(utc(2025, 6, 1, 11, 59), 12),
            utc(2025, 6, 1, 12, 0)
        );
    }

    #[test]
    fn period_end_sunday_at_or_after_rollover_is_next_sunday() {
        assert_eq!(
            period_end_after(utc(2025, 6, 1, 12, 0), 12),
            utc(2025, 6, 8, 12, 0)
        );
        assert_eq!(
            period_end_after(utc(2025, 6, 1, 18, 0), 12),
            utc(2025, 6, 8, 12, 0)
        );
    }

    #[test]
    fn period_end_saturday_is_next_day() {
        assert_eq!(
            period_end_after(utc(2025, 6, 7, 23, 0), 12),
            utc(2025, 6, 8, 12, 0)
        );
    }

    #[test]
    fn test_datetime_roundtrip() {
        let now = Utc::now();
        let formatted = format_datetime(&now);
        let parsed = parse_datetime(&formatted).unwrap();

        // Should be equal to the second
        assert_eq!(now.timestamp(), parsed.timestamp());
    }
}
