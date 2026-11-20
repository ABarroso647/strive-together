use chrono::{DateTime, Duration, NaiveDate, Utc};

pub fn week_end(week_start: DateTime<Utc>) -> DateTime<Utc> {
    week_start + Duration::weeks(1)
}

pub fn loa_end(loa_start: DateTime<Utc>, weeks: i64) -> DateTime<Utc> {
    (0..weeks).fold(loa_start, |end, _| week_end(end))
}

pub fn is_rollover_day(week_start: DateTime<Utc>, date: NaiveDate) -> bool {
    week_end(week_start).date_naive() == date
}

pub fn covers_week(
    loa_start: DateTime<Utc>,
    loa_end: DateTime<Utc>,
    week_start: DateTime<Utc>,
) -> bool {
    loa_start <= week_start && week_start < loa_end
}

pub fn covered_weeks(
    loa_start: DateTime<Utc>,
    loa_end: DateTime<Utc>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut weeks = Vec::new();
    let mut week_start = loa_start;
    while covers_week(loa_start, loa_end, week_start) {
        weeks.push((week_start, week_end(week_start)));
        week_start = week_end(week_start);
    }
    weeks
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn only_the_sunday_ending_the_week_is_its_rollover_day() {
        let week = Utc.with_ymd_and_hms(2026, 11, 8, 12, 0, 0).unwrap();
        assert!(is_rollover_day(week, date(2026, 11, 15)));
        assert!(!is_rollover_day(week, date(2026, 11, 14)));
        assert!(!is_rollover_day(week, date(2026, 11, 8)));
        assert!(!is_rollover_day(week, date(2026, 11, 16)));
    }

    #[test]
    fn midnight_rollover_has_no_split_sunday() {
        let week = Utc.with_ymd_and_hms(2026, 11, 15, 0, 0, 0).unwrap();
        assert!(!is_rollover_day(week, date(2026, 11, 15)));
    }
}
