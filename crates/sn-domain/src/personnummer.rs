//! Svenska personnummer och samordningsnummer.
//!
//! Prototypen genererar bara **testpersonnummer** enligt Skatteverkets konvention
//! (födelsenummer 980–999). De tilldelas aldrig riktiga personer.

use std::{fmt, str::FromStr};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Första födelsenumret i Skatteverkets serie för testpersonnummer.
pub const TEST_SERIAL_START: u16 = 980;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Personnummer,
    /// Dagen i numret är födelsedagen plus 60.
    Samordningsnummer,
}

/// Ett validerat personnummer eller samordningsnummer, lagrat i 12-siffrig form.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Personnummer {
    birth_date: NaiveDate,
    serial: u16,
    check: u8,
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PersonnummerError {
    #[error("fel format, förväntade ÅÅÅÅMMDD-NNNN eller ÅÅMMDD-NNNN")]
    Format,
    #[error("ogiltigt datum")]
    Date,
    #[error("felaktig kontrollsiffra")]
    Checksum,
}

impl Personnummer {
    /// Skapar ett testpersonnummer. `index` (0–19) väljer födelsenummer 980–999.
    pub fn test_number(birth_date: NaiveDate, index: u8) -> Self {
        let serial = TEST_SERIAL_START + u16::from(index % 20);
        let check = luhn_check_digit(&ten_digit_prefix(birth_date, serial, Kind::Personnummer));
        Self { birth_date, serial, check, kind: Kind::Personnummer }
    }

    /// Tolkar ett nummer. Tvåsiffriga årtal tolkas relativt `today`
    /// (`+` som avskiljare betyder att personen är minst 100 år).
    pub fn parse_relative(input: &str, today: NaiveDate) -> Result<Self, PersonnummerError> {
        let s = input.trim();
        let centenarian = s.contains('+');
        let digits: String = s.chars().filter(char::is_ascii_digit).collect();
        if digits.len() != s.chars().filter(|c| !matches!(c, '-' | '+' | ' ')).count() {
            return Err(PersonnummerError::Format);
        }

        let (year, rest) = match digits.len() {
            12 => (parse_num(&digits[..4])?, &digits[4..]),
            10 => {
                let yy = parse_num(&digits[..2])?;
                let mut year = today.year() / 100 * 100 + yy;
                if year > today.year() {
                    year -= 100;
                }
                if centenarian {
                    year -= 100;
                }
                (year, &digits[2..])
            }
            _ => return Err(PersonnummerError::Format),
        };

        let month = parse_num(&rest[..2])?;
        let raw_day = parse_num(&rest[2..4])?;
        let serial = u16::try_from(parse_num(&rest[4..7])?).map_err(|_| PersonnummerError::Format)?;
        let check = u8::try_from(parse_num(&rest[7..8])?).map_err(|_| PersonnummerError::Format)?;

        let (kind, day) =
            if raw_day > 60 { (Kind::Samordningsnummer, raw_day - 60) } else { (Kind::Personnummer, raw_day) };
        let birth_date = u32::try_from(month)
            .ok()
            .zip(u32::try_from(day).ok())
            .and_then(|(m, d)| NaiveDate::from_ymd_opt(year, m, d))
            .ok_or(PersonnummerError::Date)?;

        let pnr = Self { birth_date, serial, check, kind };
        if luhn_check_digit(&ten_digit_prefix(birth_date, serial, kind)) != check {
            return Err(PersonnummerError::Checksum);
        }
        Ok(pnr)
    }

    pub const fn birth_date(self) -> NaiveDate {
        self.birth_date
    }

    pub const fn kind(self) -> Kind {
        self.kind
    }

    /// Sant för Skatteverkets testpersonnummer (födelsenummer 980–999).
    pub const fn is_test_number(self) -> bool {
        self.serial >= TEST_SERIAL_START
    }

    /// Juridiskt kön enligt näst sista siffran. Används **endast** där en myndighet
    /// kräver det (t.ex. vissa kommunala bidrag), aldrig som visat kön.
    pub const fn legal_sex_is_female(self) -> bool {
        self.serial.is_multiple_of(2)
    }

    /// Maskerad form för visning utan särskild behörighet: `ÅÅÅÅMMDD-****`.
    pub fn masked(self) -> String {
        format!("{}-****", self.birth_date.format("%Y%m%d"))
    }

    fn day_component(self) -> u32 {
        match self.kind {
            Kind::Personnummer => self.birth_date.day(),
            Kind::Samordningsnummer => self.birth_date.day() + 60,
        }
    }
}

impl fmt::Display for Personnummer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}{:02}{:02}-{:03}{}",
            self.birth_date.year(),
            self.birth_date.month(),
            self.day_component(),
            self.serial,
            self.check
        )
    }
}

/// Visar aldrig hela numret i loggar eller felsökningsutskrifter.
impl fmt::Debug for Personnummer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Personnummer").field(&self.masked()).finish()
    }
}

impl FromStr for Personnummer {
    type Err = PersonnummerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_relative(s, chrono::Local::now().date_naive())
    }
}

impl Serialize for Personnummer {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Personnummer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

fn parse_num(s: &str) -> Result<i32, PersonnummerError> {
    s.parse().map_err(|_| PersonnummerError::Format)
}

fn ten_digit_prefix(birth_date: NaiveDate, serial: u16, kind: Kind) -> [u8; 9] {
    let day = match kind {
        Kind::Personnummer => birth_date.day(),
        Kind::Samordningsnummer => birth_date.day() + 60,
    };
    let yy = birth_date.year().rem_euclid(100).unsigned_abs();
    let s = format!("{yy:02}{:02}{day:02}{serial:03}", birth_date.month());
    let mut out = [0u8; 9];
    for (o, b) in out.iter_mut().zip(s.bytes()) {
        *o = b - b'0';
    }
    out
}

/// Luhn-algoritmen över de nio första siffrorna i 10-siffrig form.
fn luhn_check_digit(digits: &[u8; 9]) -> u8 {
    let sum: u32 = digits
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let v = u32::from(d) * if i % 2 == 0 { 2 } else { 1 };
            v / 10 + v % 10
        })
        .sum();
    u8::try_from((10 - sum % 10) % 10).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("giltigt datum")
    }

    #[test]
    fn known_test_number_round_trips() {
        // Testpersonnummer från Skatteverkets serie 980–999.
        let pnr: Personnummer = "19121212-9819".parse().expect("giltigt");
        assert!(pnr.is_test_number());
        assert_eq!(pnr.to_string(), "19121212-9819");
        assert_eq!(pnr.masked(), "19121212-****");
    }

    #[test]
    fn rejects_bad_checksum_and_format() {
        assert_eq!("19121212-9818".parse::<Personnummer>(), Err(PersonnummerError::Checksum));
        assert_eq!("1912-12".parse::<Personnummer>(), Err(PersonnummerError::Format));
        assert_eq!("19121312-9819".parse::<Personnummer>(), Err(PersonnummerError::Date));
    }

    #[test]
    fn ten_digit_form_uses_century_and_plus() {
        let today = d(2026, 10, 16);
        let young = Personnummer::parse_relative("121212-9819", today).expect("giltigt");
        assert_eq!(young.birth_date(), d(2012, 12, 12));
        let old = Personnummer::parse_relative("121212+9819", today).expect("giltigt");
        assert_eq!(old.birth_date(), d(1912, 12, 12));
    }

    #[test]
    fn coordination_number_adds_sixty_to_day() {
        let base = Personnummer::test_number(d(1990, 1, 1), 3);
        let s = base.to_string().replacen("19900101", "19900161", 1);
        let prefix = ten_digit_prefix(d(1990, 1, 1), base.serial, Kind::Samordningsnummer);
        let fixed = format!("{}{}", &s[..s.len() - 1], luhn_check_digit(&prefix));
        let samnr: Personnummer = fixed.parse().expect("giltigt samordningsnummer");
        assert_eq!(samnr.kind(), Kind::Samordningsnummer);
        assert_eq!(samnr.birth_date(), d(1990, 1, 1));
    }

    #[test]
    fn debug_never_leaks_full_number() {
        let pnr = Personnummer::test_number(d(2015, 6, 1), 0);
        assert!(!format!("{pnr:?}").contains(&pnr.to_string()[9..]));
    }

    proptest! {
        #[test]
        fn generated_test_numbers_are_valid(days in 0i64..40_000, idx in 0u8..20) {
            let birth = d(1920, 1, 1) + chrono::Duration::days(days);
            let pnr = Personnummer::test_number(birth, idx);
            let parsed: Personnummer = pnr.to_string().parse().expect("ska tolkas");
            prop_assert_eq!(parsed, pnr);
            prop_assert!(parsed.is_test_number());
        }
    }
}
