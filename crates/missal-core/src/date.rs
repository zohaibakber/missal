use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CalendarDate {
    pub fn new(year: i32, month: u8, day: u8) -> Option<CalendarDate> {
        ((1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && day >= 1
            && day <= days_in_month(year, month))
        .then_some(CalendarDate { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    pub fn sort_key(self) -> i32 {
        self.year * 10_000 + i32::from(self.month) * 100 + i32::from(self.day)
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}-{:02}-{:04}", self.day, self.month, self.year)
    }
}

pub fn digit_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        '\u{0660}'..='\u{0669}' => Some(c as u32 - 0x0660),
        '\u{06F0}'..='\u{06F9}' => Some(c as u32 - 0x06F0),
        _ => None,
    }
}

pub fn fold_digits(text: &str) -> String {
    text.chars()
        .map(|c| match digit_value(c) {
            Some(digit) if !c.is_ascii_digit() => char::from(b'0' + digit as u8),
            _ => c,
        })
        .collect()
}

pub fn parse_date(value: &str) -> Option<CalendarDate> {
    let mut parts = [0u32; 3];
    let mut widths = [0usize; 3];
    let mut index = 0;
    let mut separator = None;
    for c in value.trim().chars() {
        if let Some(digit) = digit_value(c) {
            if widths[index] == 4 {
                return None;
            }
            parts[index] = parts[index] * 10 + digit;
            widths[index] += 1;
        } else if matches!(c, '-' | '/' | '.')
            && index < 2
            && widths[index] > 0
            && separator.is_none_or(|s| s == c)
        {
            separator = Some(c);
            index += 1;
        } else {
            return None;
        }
    }
    if index != 2 || widths[2] == 0 {
        return None;
    }
    let [a, b, c] = parts;
    let (year, month, day) = match widths {
        [4, 1..=2, 1..=2] => (a, b, c),
        [1..=2, 1..=2, 4] => (c, b, a),
        [1..=2, 1..=2, 2] => (2000 + c, b, a),
        _ => return None,
    };
    CalendarDate::new(year as i32, month as u8, day as u8)
}

pub fn format_date(value: &str) -> String {
    match parse_date(value) {
        Some(date) => date.to_string(),
        None => value.trim().to_owned(),
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_month_year_in_any_digits_and_separator() {
        assert_eq!(format_date(" 12-09-2026 "), "12-09-2026");
        assert_eq!(format_date("1-2-2026"), "01-02-2026");
        assert_eq!(format_date("1.2.2026"), "01-02-2026");
        assert_eq!(format_date("1/2/26"), "01-02-2026");
        assert_eq!(format_date("۱۲/۰۹/۲۰۲۶"), "12-09-2026");
        assert_eq!(format_date("2026-09-12"), "12-09-2026");
        assert_eq!(format_date("29-02-2024"), "29-02-2024");
    }

    #[test]
    fn impossible_or_ambiguous_dates_are_kept_as_typed() {
        for text in [
            "31-02-2026",
            "29-02-2025",
            "00-01-2026",
            "1-13-2026",
            "123-01-2026",
            "1-2/2026",
            "2026",
            "unknown",
            "",
            "12-09-2026 10:30",
        ] {
            assert_eq!(parse_date(text), None, "{text}");
            assert_eq!(format_date(text), text.trim());
        }
    }

    #[test]
    fn dates_sort_chronologically() {
        let key = |text: &str| parse_date(text).unwrap().sort_key();
        assert!(key("31-12-2025") < key("01-01-2026"));
        assert!(key("01-02-2026") < key("02-02-2026"));
    }
}
