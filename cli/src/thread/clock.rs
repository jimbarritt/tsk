use std::time::{SystemTime, UNIX_EPOCH};

pub fn utc_now() -> String {
    format_utc(now_secs())
}

/// Seconds since the Unix epoch.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The inverse of [`format_utc`]: seconds since the epoch for a `YYYY-MM-DDTHH:MM:SSZ`
/// timestamp. `None` for any other text, a date that does not exist, or a time before
/// 1970.
pub fn parse_utc(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    let shaped = b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if !shaped {
        return None;
    }
    let number = |from: usize, to: usize| text[from..to].parse::<u64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(year as i64, month, day)?;
    u64::try_from(days)
        .ok()
        .map(|d| d * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Days since 1970-01-01 for a proleptic Gregorian date; `None` when the day does not
/// exist in that month.
fn days_from_civil(year: i64, month: u64, day: u64) -> Option<i64> {
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * m + 2) / 5 + day.checked_sub(1)?;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    // Reject days past the end of the month by round-tripping through the formatter.
    let (ry, rm, rd) = civil_from_days(days);
    (ry == year && rm == month && rd == day).then_some(days)
}

pub fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe as i64 + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_the_epoch() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn formats_a_leap_day() {
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn formats_seconds_minutes_and_hours() {
        assert_eq!(format_utc(1_790_000_000), "2026-09-21T14:13:20Z");
        assert_eq!(format_utc(4_102_444_799), "2099-12-31T23:59:59Z");
    }

    #[test]
    fn parse_utc_reads_what_format_utc_writes() {
        for secs in [0, 951_782_400, 1_790_000_000, 4_102_444_799] {
            assert_eq!(parse_utc(&format_utc(secs)), Some(secs));
        }
    }

    #[test]
    fn parse_utc_refuses_other_text_and_dates_that_do_not_exist() {
        for bad in [
            "",
            "2026-10-19",
            "2026-10-19 10:00:00Z",
            "2026-10-19T10:00:00",
            "2026-02-30T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-10-19T24:00:00Z",
            "1969-12-31T23:59:59Z",
            "20x6-10-19T10:00:00Z",
        ] {
            assert_eq!(parse_utc(bad), None, "{:?}", bad);
        }
    }

    #[test]
    fn utc_now_has_the_second_precision_shape() {
        let now = utc_now();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z'));
        assert_eq!(&now[10..11], "T");
    }
}
