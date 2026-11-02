//! Date handling without a date library (DESIGN section 5).
use crate::options::DatePrecision;

/// A parsed ISO date; finer parts are `None` when the input did not have them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoDate {
    pub year: u16,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

/// The `year`/`month`/`day` attributes of the XSD `DateType`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateAttrs {
    pub year: String,
    pub month: Option<String>,
    pub day: Option<String>,
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn leap(y: u16) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: u16, m: u8) -> u8 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(y) => 29,
        _ => 28,
    }
}

/// Accepts `YYYY`, `YYYY-MM`, `YYYY-MM-DD` (ASCII digits, year 1000..=9999, real calendar dates).
pub fn parse_iso_date(s: &str) -> Option<IsoDate> {
    let parts: Vec<&str> = s.split('-').collect();
    let (ys, ms, ds) = match parts.as_slice() {
        [y] => (*y, None, None),
        [y, m] => (*y, Some(*m), None),
        [y, m, d] => (*y, Some(*m), Some(*d)),
        _ => return None,
    };
    if ys.len() != 4 || !all_digits(ys) {
        return None;
    }
    let year: u16 = ys.parse().ok()?;
    if !(1000..=9999).contains(&year) {
        return None;
    }
    let month = match ms {
        None => None,
        Some(m) => {
            if m.len() != 2 || !all_digits(m) {
                return None;
            }
            let v: u8 = m.parse().ok()?;
            if !(1..=12).contains(&v) {
                return None;
            }
            Some(v)
        }
    };
    let day = match ds {
        None => None,
        Some(d) => {
            if d.len() != 2 || !all_digits(d) {
                return None;
            }
            let v: u8 = d.parse().ok()?;
            let m = month?;
            if v < 1 || v > days_in_month(year, m) {
                return None;
            }
            Some(v)
        }
    };
    Some(IsoDate { year, month, day })
}

impl IsoDate {
    /// Attributes for `DateType` honouring the precision; never invents a finer part.
    pub fn to_attrs(self, p: DatePrecision) -> DateAttrs {
        let month = match (p, self.month) {
            (DatePrecision::Year, _) => None,
            (_, m) => m,
        };
        let day = match (p, month, self.day) {
            (DatePrecision::Day, Some(_), d) => d,
            _ => None,
        };
        DateAttrs {
            year: format!("{:04}", self.year),
            month: month.map(|m| format!("--{m:02}")),
            day: day.map(|d| format!("---{d:02}")),
        }
    }

    /// Text form for free text: `YYYY`, `YYYY-MM`, `YYYY-MM-DD` per precision and what is known.
    pub fn to_text(self, p: DatePrecision) -> String {
        let a = self.to_attrs(p);
        let mut s = a.year;
        if let Some(m) = a.month {
            s.push('-');
            s.push_str(m.trim_start_matches('-'));
        }
        if let Some(d) = a.day {
            s.push('-');
            s.push_str(d.trim_start_matches('-'));
        }
        s
    }

    /// Comparison key for order checks.
    pub fn key(self) -> (u16, u8, u8) {
        (self.year, self.month.unwrap_or(1), self.day.unwrap_or(1))
    }
}

/// Regex-free `xsd:dateTime` check: `YYYY-MM-DDThh:mm:ss(.d+)?(Z|(+|-)hh:mm)?`.
pub fn valid_datetime(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 19 || !s.is_ascii() {
        return false;
    }
    let Some(date) = s.get(0..10) else {
        return false;
    };
    let Some(d) = parse_iso_date(date) else {
        return false;
    };
    if d.month.is_none() || d.day.is_none() {
        return false;
    }
    if b.get(10) != Some(&b'T') || b.get(13) != Some(&b':') || b.get(16) != Some(&b':') {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let t = s.get(r)?;
        if t.len() == 2 && all_digits(t) {
            t.parse().ok()
        } else {
            None
        }
    };
    let (Some(h), Some(mi), Some(se)) = (num(11..13), num(14..16), num(17..19)) else {
        return false;
    };
    if h > 23 || mi > 59 || se > 59 {
        return false;
    }
    let mut rest = s.get(19..).unwrap_or("");
    if let Some(r) = rest.strip_prefix('.') {
        let n = r.bytes().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return false;
        }
        rest = r.get(n..).unwrap_or("");
    }
    if rest.is_empty() || rest == "Z" {
        return true;
    }
    let Some(tz) = rest.strip_prefix('+').or_else(|| rest.strip_prefix('-')) else {
        return false;
    };
    if tz.len() != 5 || tz.as_bytes().get(2) != Some(&b':') {
        return false;
    }
    let (Some(th), Some(tm)) = (tz.get(0..2), tz.get(3..5)) else {
        return false;
    };
    if !all_digits(th) || !all_digits(tm) {
        return false;
    }
    let (th, tm): (u32, u32) = (th.parse().unwrap_or(99), tm.parse().unwrap_or(99));
    th < 14 && tm < 60 || th == 14 && tm == 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use DatePrecision::*;

    #[test]
    fn parse_valid_and_invalid() {
        assert!(parse_iso_date("2024-02-29").is_some());
        assert!(parse_iso_date("2023-02-29").is_none());
        assert!(parse_iso_date("2024-13-01").is_none());
        assert!(parse_iso_date("2024-00").is_none());
        assert!(parse_iso_date("0000").is_none());
        assert!(parse_iso_date("20014-12-01").is_none());
        assert!(parse_iso_date("2024-6").is_none());
        assert!(parse_iso_date("---").is_none());
        assert!(parse_iso_date("").is_none());
        assert!(parse_iso_date("2020/03/15").is_none());
        assert!(parse_iso_date("15-03-2020").is_none());
        assert!(parse_iso_date("2024-02-30").is_none());
        assert!(parse_iso_date("2000-02-29").is_some());
        assert!(parse_iso_date("1900-02-29").is_none());
        assert!(parse_iso_date("2024-\u{661}1").is_none());
    }

    #[test]
    fn attrs_matrix() {
        let d = parse_iso_date("2020-03-15").unwrap();
        let a = d.to_attrs(Month);
        assert_eq!(
            (a.year.as_str(), a.month.as_deref(), a.day),
            ("2020", Some("--03"), None)
        );
        let a = d.to_attrs(Day);
        assert_eq!(
            (a.month.as_deref(), a.day.as_deref()),
            (Some("--03"), Some("---15"))
        );
        let a = d.to_attrs(Year);
        assert_eq!((a.month, a.day), (None, None));
        let y = parse_iso_date("1996").unwrap();
        let a = y.to_attrs(Day);
        assert_eq!((a.year.as_str(), a.month, a.day), ("1996", None, None));
        assert_eq!(d.to_text(Month), "2020-03");
        assert_eq!(d.to_text(Day), "2020-03-15");
        assert_eq!(y.to_text(Month), "1996");
    }

    #[test]
    fn datetime() {
        assert!(valid_datetime("2026-11-02T10:00:00Z"));
        assert!(valid_datetime("2026-11-02T10:00:00"));
        assert!(valid_datetime("2026-11-02T10:00:00.123+02:00"));
        assert!(valid_datetime("2026-11-02T10:00:00-05:30"));
        assert!(!valid_datetime("2026-11-02T24:00:00Z"));
        assert!(!valid_datetime("2026-11-02T10:60:00Z"));
        assert!(!valid_datetime("2026-02-30T10:00:00Z"));
        assert!(!valid_datetime("2026-11-02"));
        assert!(!valid_datetime("2026-11-02T10:00:00.Z"));
        assert!(!valid_datetime("2026-11-02T10:00:00+2:00"));
        assert!(!valid_datetime("garbage"));
        assert!(!valid_datetime("2026-11-02 10:00:00Z"));
    }
}
