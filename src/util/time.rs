use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};

/// Returns the next Sunday at `rollover_hour` UTC.
/// If today is Sunday and we haven't passed rollover_hour yet, returns today at that hour.
/// Otherwise returns the following Sunday.
pub fn get_period_end_time(rollover_hour: u32) -> DateTime<Utc> {
    let now = Utc::now();
    let days_from_sunday = now.weekday().num_days_from_sunday() as i64;
    let days_offset = if days_from_sunday == 0 { 0 } else { 7 - days_from_sunday };

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

    #[test]
    fn test_period_bounds() {
        let (start, end) = get_weekly_period_bounds_with_hour(12);

        // Start should be a Sunday
        assert_eq!(start.weekday().num_days_from_sunday(), 0);

        // End should be 7.5 days after start
        let duration = end - start;
        assert_eq!(duration.num_hours(), 7 * 24 + 12);
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
