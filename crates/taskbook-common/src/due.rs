//! Due-date parsing, formatting and status classification.
//!
//! Due dates are stored as epoch milliseconds at local midnight of the due
//! day, consistent with the `_timestamp` convention used elsewhere.

use chrono::{DateTime, Local, NaiveDate, TimeZone};

/// Classification of a due date relative to today (local time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueStatus {
    Overdue,
    DueToday,
    Upcoming,
}

/// Parse a due-date string into local-midnight epoch millis.
///
/// Accepts `YYYY-MM-DD`, `today`, `tomorrow` (case-insensitive).
pub fn parse_due_date(raw: &str) -> Option<i64> {
    let s = raw.trim().to_lowercase();
    let date = match s.as_str() {
        "today" => Local::now().date_naive(),
        "tomorrow" => Local::now().date_naive().succ_opt()?,
        _ => NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()?,
    };
    date_to_millis(date)
}

fn date_to_millis(date: NaiveDate) -> Option<i64> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

/// Format a due-date millis value as `YYYY-MM-DD` in local time.
pub fn format_due_date(millis: i64) -> String {
    DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Classify a due date against the current local date.
pub fn due_status(due_millis: i64) -> DueStatus {
    let due =
        DateTime::from_timestamp_millis(due_millis).map(|dt| dt.with_timezone(&Local).date_naive());
    let today = Local::now().date_naive();
    match due {
        Some(d) if d < today => DueStatus::Overdue,
        Some(d) if d == today => DueStatus::DueToday,
        _ => DueStatus::Upcoming,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_date_round_trips() {
        let millis = parse_due_date("2026-07-01").unwrap();
        assert_eq!(format_due_date(millis), "2026-07-01");
    }

    #[test]
    fn parse_today_and_tomorrow() {
        let today = Local::now().date_naive();
        let t = parse_due_date("today").unwrap();
        assert_eq!(format_due_date(t), today.format("%Y-%m-%d").to_string());
        let tm = parse_due_date("Tomorrow").unwrap();
        assert_eq!(
            format_due_date(tm),
            today.succ_opt().unwrap().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert_eq!(parse_due_date("next week"), None);
        assert_eq!(parse_due_date("2026-13-99"), None);
        assert_eq!(parse_due_date(""), None);
    }

    #[test]
    fn status_classification() {
        let today = parse_due_date("today").unwrap();
        assert_eq!(due_status(today), DueStatus::DueToday);
        let tomorrow = parse_due_date("tomorrow").unwrap();
        assert_eq!(due_status(tomorrow), DueStatus::Upcoming);
        let yesterday = today - 24 * 60 * 60 * 1000;
        assert_eq!(due_status(yesterday), DueStatus::Overdue);
    }
}
