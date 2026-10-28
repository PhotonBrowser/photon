//! Grouping visits by local calendar day, for the history page.

const SECONDS_PER_DAY: i64 = 86_400;
const WEEKDAYS: [&str; 7] = [
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The local day `unix_seconds` falls on, counted from 1 January 1970.
pub fn local_day(unix_seconds: u64, utc_offset_seconds: i64) -> i64 {
    (unix_seconds as i64 + utc_offset_seconds).div_euclid(SECONDS_PER_DAY)
}

/// The local time of day, as 24-hour "HH:MM".
pub fn time_of_day(unix_seconds: u64, utc_offset_seconds: i64) -> String {
    let seconds = (unix_seconds as i64 + utc_offset_seconds).rem_euclid(SECONDS_PER_DAY);
    format!("{:02}:{:02}", seconds / 3_600, seconds % 3_600 / 60)
}

/// A heading for visits on `day`: "Today", "Yesterday", or the weekday and
/// date, with the year when it is not `today`'s.
pub fn day_heading(day: i64, today: i64) -> String {
    match today - day {
        0 => return "Today".into(),
        1 => return "Yesterday".into(),
        _ => {}
    }
    let weekday = WEEKDAYS[day.rem_euclid(7) as usize];
    let (year, month, date) = civil_from_days(day);
    let (this_year, _, _) = civil_from_days(today);
    let month = MONTHS[(month - 1) as usize];
    if year == this_year {
        format!("{weekday}, {date} {month}")
    } else {
        format!("{weekday}, {date} {month} {year}")
    }
}

/// The proleptic Gregorian date of a day count, after Howard Hinnant's
/// `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let date = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_recent_days_and_dates() {
        // 2026-10-28 is a Wednesday.
        let today = local_day(1_793_145_600, 0);
        assert_eq!(day_heading(today, today), "Today");
        assert_eq!(day_heading(today - 1, today), "Yesterday");
        assert_eq!(day_heading(today - 2, today), "Monday, 26 October");
        assert_eq!(day_heading(today - 365, today), "Tuesday, 28 October 2025");
    }

    #[test]
    fn applies_the_local_offset() {
        // 23:30 UTC is the next day an hour east.
        let late = 1_793_145_600 + 23 * 3_600 + 1_800;
        assert_eq!(local_day(late, 3_600), local_day(late, 0) + 1);
        assert_eq!(local_day(late, -3_600), local_day(late, 0));
        assert_eq!(time_of_day(late, 0), "23:30");
        assert_eq!(time_of_day(late, 3_600), "00:30");
    }
}
