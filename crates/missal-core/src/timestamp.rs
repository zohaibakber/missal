use std::fmt;

use crate::date::civil_from_days;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn from_unix_millis(millis: i64) -> Timestamp {
        let days = millis.div_euclid(86_400_000);
        let rest = millis.rem_euclid(86_400_000);
        let (year, month, day) = civil_from_days(days);
        Timestamp(format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
            rest / 3_600_000,
            rest / 60_000 % 60,
            rest / 1000 % 60,
            rest % 1000
        ))
    }

    pub fn parse(value: &str) -> Option<Timestamp> {
        let bytes = value.as_bytes();
        let shape = b"dddd-dd-ddTdd:dd:dd.dddZ";
        (bytes.len() == shape.len()
            && bytes.iter().zip(shape).all(|(&byte, &expected)| {
                if expected == b'd' {
                    byte.is_ascii_digit()
                } else {
                    byte == expected
                }
            }))
        .then(|| Timestamp(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_to_iso_string() {
        assert_eq!(
            Timestamp::from_unix_millis(1_793_144_341_007).as_str(),
            "2026-10-27T23:39:01.007Z"
        );
        assert_eq!(Timestamp::from_unix_millis(0).as_str(), "1970-01-01T00:00:00.000Z");
        assert!(Timestamp::parse("2026-10-27T23:39:01.007Z").is_some());
        assert!(Timestamp::parse("2026-10-27T23:39:01Z").is_none());
    }
}
