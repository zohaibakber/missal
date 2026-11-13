use std::fmt;

use crate::text::{is_js_whitespace, js_trim};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: i64,
    month: u8,
    day: u8,
}

const MAX_JS_DAYS: i64 = 100_000_000;
const MS_PER_DAY: f64 = 86_400_000.0;

impl CalendarDate {
    pub fn new(year: i64, month: u8, day: u8) -> Option<CalendarDate> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month))
            .then_some(CalendarDate { year, month, day })
            .filter(|date| date.days().abs() <= MAX_JS_DAYS)
    }

    pub fn year(self) -> i64 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    fn days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    fn from_days(days: i64) -> Option<CalendarDate> {
        let (year, month, day) = civil_from_days(days);
        CalendarDate::new(year, month, day)
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let year = if self.year > 0 { self.year } else { 1 - self.year };
        write!(f, "{:02}-{:02}-{year:04}", self.day, self.month)
    }
}

pub fn parse_date(value: &str) -> Option<CalendarDate> {
    let trimmed = js_trim(value);
    if trimmed.is_empty() {
        return None;
    }
    parse_numeric(trimmed, '-')
        .or_else(|| parse_numeric(trimmed, '.'))
        .or_else(|| parse_long(trimmed))
        .or_else(|| parse_iso(trimmed))
}

pub fn format_date(value: &str) -> String {
    match parse_date(value) {
        Some(date) => date.to_string(),
        None => js_trim(value).to_owned(),
    }
}

fn digits(input: &str, max: usize) -> Option<(i64, &str)> {
    let count = input
        .bytes()
        .take(max)
        .take_while(u8::is_ascii_digit)
        .count();
    (count > 0).then(|| (input[..count].parse().unwrap_or(i64::MAX), &input[count..]))
}

fn finish(rest: &str, year: i64, month: i64, day: i64) -> Option<CalendarDate> {
    if !rest.chars().all(is_js_whitespace) || year <= 0 || !(1..=12).contains(&month) {
        return None;
    }
    CalendarDate::new(year, month as u8, u8::try_from(day).ok()?)
}

fn parse_numeric(input: &str, separator: char) -> Option<CalendarDate> {
    let (day, rest) = digits(input, 2)?;
    let rest = rest.strip_prefix(separator)?;
    let (month, rest) = digits(rest, 2)?;
    let rest = rest.strip_prefix(separator)?;
    let (year, rest) = digits(rest, 4)?;
    finish(rest, year, month, day)
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];
const ANY_MONTH_PREFIXES: [&str; 12] = [
    "ja", "f", "mar", "ap", "may", "jun", "jul", "au", "s", "o", "n", "d",
];
const NARROW_MONTHS: [char; 12] = ['j', 'f', 'm', 'a', 'm', 'j', 'j', 'a', 's', 'o', 'n', 'd'];

fn starts_with_ignore_case(input: &str, prefix: &str) -> bool {
    input
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn month_name(input: &str) -> Option<(i64, &str)> {
    let matched = MONTHS
        .iter()
        .find(|name| starts_with_ignore_case(input, name))
        .map(|name| name.len())
        .or_else(|| {
            MONTHS
                .iter()
                .find(|name| starts_with_ignore_case(input, &name[..3]))
                .map(|_| 3)
        });
    if let Some(length) = matched {
        let word = &input[..length];
        let month = ANY_MONTH_PREFIXES
            .iter()
            .position(|prefix| starts_with_ignore_case(word, prefix))?;
        return Some((month as i64 + 1, &input[length..]));
    }
    let first = input.chars().next()?.to_ascii_lowercase();
    let month = NARROW_MONTHS.iter().position(|&letter| letter == first)?;
    Some((month as i64 + 1, &input[1..]))
}

fn day_of_month(input: &str) -> Option<(i64, &str)> {
    let bytes = input.as_bytes();
    let first = *bytes.first().filter(|byte| byte.is_ascii_digit())?;
    let second = bytes.get(1).copied();
    let length = match (first, second) {
        (b'3', Some(b'0' | b'1')) => 2,
        (b'0'..=b'2', Some(next)) if next.is_ascii_digit() => 2,
        _ => 1,
    };
    Some((input[..length].parse().ok()?, &input[length..]))
}

fn parse_long(input: &str) -> Option<CalendarDate> {
    let (month, rest) = month_name(input)?;
    let rest = rest.strip_prefix(' ')?;
    let (day, rest) = day_of_month(rest)?;
    let rest = rest.strip_prefix(", ")?;
    let (year, rest) = digits(rest, 4)?;
    finish(rest, year, month, day)
}

fn parse_iso(input: &str) -> Option<CalendarDate> {
    let parts: Vec<&str> = input.split(['T', ' ']).collect();
    if parts.len() > 2 || parts[0].contains(':') {
        return None;
    }
    let (date, time) = match parts[0].find(['Z', 'z']) {
        Some(end) => (&input[..end], Some(&input[end..])),
        None => (parts[0], parts.get(1).copied()),
    };
    let (time, zone) = match time.filter(|time| !time.is_empty()) {
        None => ("", None),
        Some(time) => match time.find(['Z', '+', '-']) {
            Some(start) => (&time[..start], Some(&time[start..])),
            None => (time, None),
        },
    };
    let (year, rest) = iso_year(date)?;
    let mut millis = iso_date_days(rest, year)? as f64 * MS_PER_DAY;
    if !time.is_empty() {
        millis += iso_time(time)?;
    }
    if let Some(zone) = zone {
        millis += iso_zone(zone)?;
    }
    if millis.abs() > 8.64e15 {
        return None;
    }
    CalendarDate::from_days((millis / MS_PER_DAY).floor() as i64)
}

fn iso_year(date: &str) -> Option<(i64, &str)> {
    let bytes = date.as_bytes();
    let all_digits = |range: std::ops::Range<usize>| {
        bytes.len() >= range.end && bytes[range].iter().all(u8::is_ascii_digit)
    };
    let signed = |length: usize| {
        matches!(bytes.first(), Some(b'+' | b'-')) && all_digits(1..length + 1)
    };
    if all_digits(0..4) {
        return Some((date[..4].parse().ok()?, &date[4..]));
    }
    if signed(6) {
        return Some((date[..7].parse().ok()?, &date[7..]));
    }
    if bytes.len() == 2 && all_digits(0..2) {
        return Some((date.parse::<i64>().ok()? * 100, ""));
    }
    if bytes.len() == 5 && signed(4) {
        return Some((date.parse::<i64>().ok()? * 100, ""));
    }
    None
}

fn iso_date_days(rest: &str, year: i64) -> Option<i64> {
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let bytes = rest.as_bytes();
    let numeric = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
    let (mut day_of_year, mut month, mut day) = (1, 1, 1);
    if let Some(week) = rest.strip_prefix('W') {
        let (weeks, weekday) = match week.len() {
            2 => (week, None),
            3 => (&week[..2], Some(&week[2..])),
            4 if week.as_bytes()[2] == b'-' => (&week[..2], Some(&week[3..])),
            _ => return None,
        };
        if !numeric(weeks) || !weekday.is_none_or(numeric) {
            return None;
        }
        let week: i64 = weeks.parse().ok()?;
        let weekday: i64 = weekday.map_or(Some(1), |text| text.parse().ok())? - 1;
        if !(1..=53).contains(&week) || !(0..=6).contains(&weekday) {
            return None;
        }
        let fourth = days_from_civil(year, 1, 4);
        let fourth_weekday = match (fourth + 4).rem_euclid(7) {
            0 => 7,
            other => other,
        };
        return Some(fourth + (week - 1) * 7 + weekday + 1 - fourth_weekday);
    }
    let compact;
    let rest = if bytes.len() == 5 && bytes[2] == b'-' {
        compact = format!("{}{}", &rest[..2], &rest[3..]);
        compact.as_str()
    } else {
        rest
    };
    if !numeric(rest) {
        return None;
    }
    match rest.len() {
        0 => {}
        3 => day_of_year = rest.parse().ok()?,
        2 => month = rest.parse().ok()?,
        4 => {
            month = rest[..2].parse().ok()?;
            day = rest[2..].parse().ok()?;
        }
        _ => return None,
    }
    if !(1..=12).contains(&month)
        || day < 1
        || day > i64::from(days_in_month(year, month as u8))
        || day_of_year < 1
        || day_of_year > if is_leap(year) { 366 } else { 365 }
    {
        return None;
    }
    Some(days_from_civil(year, month as u8, 1) + day_of_year.max(day) - 1)
}

fn iso_time(time: &str) -> Option<f64> {
    fn unit(input: &str) -> Option<(f64, &str)> {
        let bytes = input.as_bytes();
        if bytes.len() < 2 || !bytes[..2].iter().all(u8::is_ascii_digit) {
            return None;
        }
        let mut end = 2;
        if matches!(bytes.get(2), Some(b'.' | b',')) {
            end = 3 + bytes[3..].iter().take_while(|byte| byte.is_ascii_digit()).count();
        }
        let value = input[..end].replace(',', ".");
        let value = value.strip_suffix('.').unwrap_or(&value);
        Some((value.parse().unwrap_or(0.0), &input[end..]))
    }
    let (hours, mut rest) = unit(time)?;
    let mut later = [0.0, 0.0];
    for slot in &mut later {
        if rest.is_empty() {
            break;
        }
        let (value, remaining) = unit(rest.strip_prefix(':').unwrap_or(rest))?;
        *slot = value;
        rest = remaining;
    }
    if !rest.is_empty() {
        return None;
    }
    let [minutes, seconds] = later;
    let valid = if hours == 24.0 {
        minutes == 0.0 && seconds == 0.0
    } else {
        (0.0..60.0).contains(&seconds) && (0.0..60.0).contains(&minutes) && (0.0..25.0).contains(&hours)
    };
    valid.then_some(hours * 3_600_000.0 + minutes * 60_000.0 + seconds * 1000.0)
}

fn iso_zone(zone: &str) -> Option<f64> {
    if zone == "Z" {
        return Some(0.0);
    }
    let bytes = zone.as_bytes();
    let sign = match bytes.first() {
        Some(b'+') => -1.0,
        Some(b'-') => 1.0,
        _ => return Some(0.0),
    };
    let digits_at = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    let minutes = match bytes.len() {
        3 if digits_at(1..3) => 0,
        5 if digits_at(1..5) => zone[3..5].parse::<i64>().ok()?,
        6 if digits_at(1..3) && bytes[3] == b':' && digits_at(4..6) => zone[4..6].parse().ok()?,
        _ => return Some(0.0),
    };
    let hours: i64 = zone[1..3].parse().ok()?;
    (minutes <= 59).then_some(sign * (hours * 3_600_000 + minutes * 60_000) as f64)
}

fn is_leap(year: i64) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub(crate) fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub(crate) fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u8;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u8;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalised(value: &str) -> String {
        format_date(value)
    }

    #[test]
    fn numeric_formats() {
        assert_eq!(normalised(" 12-09-2026 "), "12-09-2026");
        assert_eq!(normalised("1-2-2026"), "01-02-2026");
        assert_eq!(normalised("1.2.2026"), "01-02-2026");
        assert_eq!(normalised("1-2-26"), "01-02-0026");
        assert_eq!(normalised("31-02-2026"), "31-02-2026");
        assert_eq!(normalised("29-02-2024"), "29-02-2024");
        assert_eq!(normalised("00-01-2026"), "00-01-2026");
        assert_eq!(normalised("1-13-2026"), "1-13-2026");
        assert_eq!(normalised("123-01-2026"), "123-01-2026");
        assert_eq!(normalised("unknown"), "unknown");
        assert_eq!(normalised(""), "");
    }

    #[test]
    fn long_format() {
        assert_eq!(normalised("March 5, 2026"), "05-03-2026");
        assert_eq!(normalised("sep 30, 2026"), "30-09-2026");
        assert_eq!(normalised("J 1, 2026"), "01-01-2026");
        assert_eq!(normalised("June 31, 2026"), "June 31, 2026");
        assert_eq!(normalised("May 35, 2026"), "May 35, 2026");
    }

    #[test]
    fn iso_formats() {
        assert_eq!(normalised("2026-09-12"), "12-09-2026");
        assert_eq!(normalised("20260912"), "12-09-2026");
        assert_eq!(normalised("2026-09"), "01-09-2026");
        assert_eq!(normalised("2026-0912"), "12-09-2026");
        assert_eq!(normalised("202609-12"), "12-09-2026");
        assert_eq!(normalised("2026"), "01-01-2026");
        assert_eq!(normalised("2026-032"), "01-02-2026");
        assert_eq!(normalised("2026-W01-1"), "29-12-2025");
        assert_eq!(normalised("2026-09-12T10:30"), "12-09-2026");
        assert_eq!(normalised("2026-09-12 24:00"), "13-09-2026");
        assert_eq!(normalised("2026-09-12T23:00:00Z"), "12-09-2026");
        assert_eq!(normalised("2026-09-12T23:00:00-02:00"), "13-09-2026");
        assert_eq!(normalised("2026-09-12T25:00"), "2026-09-12T25:00");
        assert_eq!(normalised("2026-02-30"), "2026-02-30");
        assert_eq!(normalised("+002026-09-12"), "12-09-2026");
        assert_eq!(normalised("10:30"), "10:30");
    }

    #[test]
    fn civil_days_round_trip() {
        for days in [-800_000, -1, 0, 1, 20_000, 2_000_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
    }
}
