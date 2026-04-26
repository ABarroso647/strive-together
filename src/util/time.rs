use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};

/// Get the start and end times for the current weekly period.
/// Week runs from Sunday 00:00 UTC to Saturday 23:59 UTC (end is next Sunday 12:00 for buffer).
pub fn get_weekly_period_bounds() -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();

    // weekday(): Monday=0 ... Sunday=6
    // We want Sunday as start of week
    // Days since last Sunday: (weekday + 1) % 7
    let days_since_sunday = (now.weekday().num_days_from_monday() as i64 + 1) % 7;

    let start = (now - Duration::days(days_since_sunday))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let start = Utc.from_utc_datetime(&start);

    // End is 7 days + 12 hours after start (buffer for late entries)
    let end = start + Duration::days(7) + Duration::hours(12);

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
        let (start, end) = get_weekly_period_bounds();

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
