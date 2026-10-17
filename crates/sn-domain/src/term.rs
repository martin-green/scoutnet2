//! Terminer (VT/HT) – den naturliga tidsenheten för medlemskap, avgifter och närvaro.

use std::{fmt, str::FromStr};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Half {
    /// Vårtermin, januari–juni.
    Spring,
    /// Hösttermin, juli–december.
    Autumn,
}

/// En termin, t.ex. `HT 2026`. Ordnas kronologiskt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Term {
    year: i32,
    half: Half,
}

impl Term {
    pub const fn new(year: i32, half: Half) -> Self {
        Self { year, half }
    }

    pub fn containing(date: NaiveDate) -> Self {
        let half = if date.month() <= 6 { Half::Spring } else { Half::Autumn };
        Self { year: date.year(), half }
    }

    pub const fn year(self) -> i32 {
        self.year
    }

    pub const fn half(self) -> Half {
        self.half
    }

    /// Kompakt id som i Skojjt: år·10 + 1 för HT.
    pub const fn id(self) -> i32 {
        self.year * 10 + matches!(self.half, Half::Autumn) as i32
    }

    /// Omvändningen till [`Term::id`].
    pub const fn from_id(id: i32) -> Self {
        let half = if id.rem_euclid(10) == 1 { Half::Autumn } else { Half::Spring };
        Self { year: id.div_euclid(10), half }
    }

    pub fn start(self) -> NaiveDate {
        let month = if matches!(self.half, Half::Spring) { 1 } else { 7 };
        NaiveDate::from_ymd_opt(self.year, month, 1).expect("första dagen i terminen finns")
    }

    pub fn end(self) -> NaiveDate {
        let (month, day) = if matches!(self.half, Half::Spring) { (6, 30) } else { (12, 31) };
        NaiveDate::from_ymd_opt(self.year, month, day).expect("sista dagen i terminen finns")
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self.half {
            Half::Spring => Self { year: self.year, half: Half::Autumn },
            Half::Autumn => Self { year: self.year + 1, half: Half::Spring },
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self.half {
            Half::Spring => Self { year: self.year - 1, half: Half::Autumn },
            Half::Autumn => Self { year: self.year, half: Half::Spring },
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = if matches!(self.half, Half::Spring) { "VT" } else { "HT" };
        write!(f, "{prefix} {}", self.year)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ogiltig termin, förväntade t.ex. 'HT 2026' eller 'vt2027'")]
pub struct TermParseError;

impl FromStr for Term {
    type Err = TermParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase();
        let half = match compact.get(..2) {
            Some("VT") => Half::Spring,
            Some("HT") => Half::Autumn,
            _ => return Err(TermParseError),
        };
        if let Some(year) = compact.get(2..)
            && let Ok(year) = year.parse::<i32>()
            && (1900..=2200).contains(&year)
        {
            return Ok(Self { year, half });
        }
        Err(TermParseError)
    }
}

impl Serialize for Term {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Term {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[test]
    fn term_basics() {
        let ht = Term::containing(NaiveDate::from_ymd_opt(2026, 10, 16).expect("datum"));
        assert_eq!(ht.to_string(), "HT 2026");
        assert_eq!(ht.id(), 20261);
        assert_eq!(Term::from_id(ht.id()), ht);
        assert_eq!(Term::from_id(20270), ht.next());
        assert_eq!(ht.next().to_string(), "VT 2027");
        assert_eq!(ht.previous().previous(), Term::new(2025, Half::Autumn));
        assert_eq!("vt2027".parse::<Term>(), Ok(ht.next()));
        assert_matches!("xx2027".parse::<Term>(), Err(_));
        assert!(ht < ht.next());
    }
}
