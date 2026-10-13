//! Export timestamps and business days. The workbench writes UTC instants as
//! `YYYY-MM-DDTHH:MM:SS.sssZ` and cuts business days at Beijing midnight (UTC+8, no DST).

use std::fmt;

/// A calendar date. Ordering follows the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CivilDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl CivilDate {
    /// Parses `YYYY-MM-DD` and rejects impossible dates.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let date = Self {
            year: i32::try_from(digits(&bytes[0..4])?).ok()?,
            month: digits(&bytes[5..7])?,
            day: digits(&bytes[8..10])?,
        };
        date.is_valid().then_some(date)
    }

    pub fn month_key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    fn is_valid(self) -> bool {
        (1..=12).contains(&self.month)
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
    }

    fn next_day(self) -> Self {
        if self.day < days_in_month(self.year, self.month) {
            Self {
                day: self.day + 1,
                ..self
            }
        } else if self.month < 12 {
            Self {
                month: self.month + 1,
                day: 1,
                ..self
            }
        } else {
            Self {
                year: self.year + 1,
                month: 1,
                day: 1,
            }
        }
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// A validated export instant. The original text is kept: it is exactly what PostgreSQL returns
/// for the stored value, so re-imports compare without conversion drift.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExportInstant {
    text: String,
    date: CivilDate,
    hour: u32,
}

impl ExportInstant {
    /// Accepts only `YYYY-MM-DDTHH:MM:SS.sssZ`, the single shape the workbench export writes.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let shape_ok = bytes.len() == 24
            && bytes[10] == b'T'
            && bytes[13] == b':'
            && bytes[16] == b':'
            && bytes[19] == b'.'
            && bytes[23] == b'Z';
        if !shape_ok {
            return None;
        }
        let date = CivilDate::parse(&text[0..10])?;
        let hour = digits(&bytes[11..13])?;
        let minute = digits(&bytes[14..16])?;
        let second = digits(&bytes[17..19])?;
        digits(&bytes[20..23])?;
        (hour < 24 && minute < 60 && second < 60).then(|| Self {
            text: text.to_owned(),
            date,
            hour,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The Beijing business day this instant falls on.
    pub fn shanghai_date(&self) -> CivilDate {
        if self.hour + 8 >= 24 {
            self.date.next_day()
        } else {
            self.date
        }
    }
}

fn digits(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(
        bytes
            .iter()
            .fold(0, |value, byte| value * 10 + u32::from(byte - b'0')),
    )
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{CivilDate, ExportInstant};

    fn shanghai(text: &str) -> String {
        ExportInstant::parse(text)
            .expect("valid instant")
            .shanghai_date()
            .to_string()
    }

    #[test]
    fn business_day_turns_at_beijing_midnight() {
        assert_eq!(shanghai("2026-09-30T15:59:59.999Z"), "2026-09-30");
        assert_eq!(shanghai("2026-09-30T16:00:00.000Z"), "2026-10-01");
        assert_eq!(shanghai("2026-12-31T16:00:00.000Z"), "2027-01-01");
        assert_eq!(shanghai("2028-02-28T16:30:00.000Z"), "2028-02-29");
        assert_eq!(shanghai("2026-02-28T16:30:00.000Z"), "2026-03-01");
    }

    #[test]
    fn only_the_export_shape_is_accepted() {
        for invalid in [
            "2026-09-30T16:00:00Z",
            "2026-09-30T16:00:00.000+08:00",
            "2026-02-30T01:00:00.000Z",
            "2026-13-01T01:00:00.000Z",
            "2026-09-30T24:00:00.000Z",
            "2026-09-30 16:00:00.000Z",
            "２０２６-09-30T16:00:00.000Z",
        ] {
            assert!(
                ExportInstant::parse(invalid).is_none(),
                "{invalid} must be rejected"
            );
        }
        assert!(CivilDate::parse("2026-10-12").is_some());
        assert!(CivilDate::parse("2026-10-32").is_none());
        assert!(CivilDate::parse("2026-1-12").is_none());
    }
}
