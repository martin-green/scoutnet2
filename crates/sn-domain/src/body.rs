//! Organisationsträdets byggstenar: enhetstyper och avdelningstyper.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Typ av enhet ("body") i organisationsträdet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    Organisation,
    Region,
    Network,
    District,
    Corps,
    CorpsDistrict,
    /// Kår.
    Group,
    /// Geografisk sektion inom en kår.
    Section,
    /// Avdelning.
    Troop,
    /// Patrull.
    Patrol,
}

impl BodyKind {
    pub const ALL: [Self; 10] = [
        Self::Organisation,
        Self::Region,
        Self::Network,
        Self::District,
        Self::Corps,
        Self::CorpsDistrict,
        Self::Group,
        Self::Section,
        Self::Troop,
        Self::Patrol,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Organisation => "organisation",
            Self::Region => "region",
            Self::Network => "network",
            Self::District => "district",
            Self::Corps => "corps",
            Self::CorpsDistrict => "corps_district",
            Self::Group => "group",
            Self::Section => "section",
            Self::Troop => "troop",
            Self::Patrol => "patrol",
        }
    }

    /// Svensk benämning för UI.
    pub const fn label_sv(self) -> &'static str {
        match self {
            Self::Organisation => "Organisation",
            Self::Region => "Region",
            Self::Network => "Nätverk",
            Self::District => "Distrikt",
            Self::Corps => "Samverkansorganisation",
            Self::CorpsDistrict => "Samv.org.-distrikt",
            Self::Group => "Kår",
            Self::Section => "Sektion",
            Self::Troop => "Avdelning",
            Self::Patrol => "Patrull",
        }
    }
}

impl fmt::Display for BodyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("okänd enhetstyp: {0}")]
pub struct UnknownBodyKind(pub String);

impl FromStr for BodyKind {
    type Err = UnknownBodyKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|k| k.as_str() == s).ok_or_else(|| UnknownBodyKind(s.to_owned()))
    }
}

/// Läsbar, stabil nyckel för en enhet: typ och nummer, t.ex. `group-1001`.
///
/// Numret är unikt per typ och är för migrerade enheter samma id som i Scoutnet och Skojjt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyKey {
    pub kind: BodyKind,
    pub number: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ogiltig enhetsnyckel: {0}")]
pub struct InvalidBodyKey(pub String);

impl fmt::Display for BodyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.kind, self.number)
    }
}

impl FromStr for BodyKey {
    type Err = InvalidBodyKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidBodyKey(s.to_owned());
        let (kind, number) = s.rsplit_once('-').ok_or_else(invalid)?;
        let kind = kind.parse::<BodyKind>().map_err(|_| invalid())?;
        // Bara kanoniska nummer (inga inledande nollor eller tecken), så att varje enhet har exakt en adress.
        let number = number.parse::<i32>().ok().filter(|n| *n > 0 && n.to_string() == number).ok_or_else(invalid)?;
        Ok(Self { kind, number })
    }
}

/// Avdelningstyp (åldersgren), med produktionens benämningar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TroopType {
    Familj,
    Baver,
    Sparare,
    Upptackare,
    Aventyrare,
    Utmanare,
    Rover,
    Annan,
}

impl TroopType {
    pub const ALL: [Self; 8] = [
        Self::Familj,
        Self::Baver,
        Self::Sparare,
        Self::Upptackare,
        Self::Aventyrare,
        Self::Utmanare,
        Self::Rover,
        Self::Annan,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Familj => "familj",
            Self::Baver => "baver",
            Self::Sparare => "sparare",
            Self::Upptackare => "upptackare",
            Self::Aventyrare => "aventyrare",
            Self::Utmanare => "utmanare",
            Self::Rover => "rover",
            Self::Annan => "annan",
        }
    }

    pub const fn label_sv(self) -> &'static str {
        match self {
            Self::Familj => "Familjeavdelning",
            Self::Baver => "Bäveravdelning",
            Self::Sparare => "Spåraravdelning",
            Self::Upptackare => "Upptäckaravdelning",
            Self::Aventyrare => "Äventyraravdelning",
            Self::Utmanare => "Utmanarlag",
            Self::Rover => "Roverlag",
            Self::Annan => "Annan",
        }
    }

    /// Typiskt åldersintervall (ålder under året), `None` för blandade grupper.
    pub const fn age_range(self) -> Option<(u8, u8)> {
        match self {
            Self::Familj => Some((4, 7)),
            Self::Baver => Some((6, 7)),
            Self::Sparare => Some((8, 9)),
            Self::Upptackare => Some((10, 11)),
            Self::Aventyrare => Some((12, 14)),
            Self::Utmanare => Some((15, 18)),
            Self::Rover => Some((19, 25)),
            Self::Annan => None,
        }
    }

    /// Avdelningstyp som en scout normalt tillhör ett visst år.
    pub fn for_age(age: u8) -> Option<Self> {
        [Self::Baver, Self::Sparare, Self::Upptackare, Self::Aventyrare, Self::Utmanare, Self::Rover]
            .into_iter()
            .find(|t| t.age_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&age)))
    }
}

impl FromStr for TroopType {
    type Err = UnknownBodyKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|k| k.as_str() == s).ok_or_else(|| UnknownBodyKind(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_via_str() {
        for k in BodyKind::ALL {
            assert_eq!(k.as_str().parse::<BodyKind>(), Ok(k));
        }
        for t in TroopType::ALL {
            assert_eq!(t.as_str().parse::<TroopType>(), Ok(t));
        }
    }

    #[test]
    fn troop_type_for_age() {
        assert_eq!(TroopType::for_age(8), Some(TroopType::Sparare));
        assert_eq!(TroopType::for_age(13), Some(TroopType::Aventyrare));
        assert_eq!(TroopType::for_age(40), None);
    }

    #[test]
    fn body_key_round_trip_and_rejects_non_canonical() {
        let key: BodyKey = "group-1001".parse().expect("giltig nyckel");
        assert_eq!(key, BodyKey { kind: BodyKind::Group, number: 1001 });
        assert_eq!(key.to_string(), "group-1001");
        assert_eq!("corps_district-7".parse::<BodyKey>().map(|k| k.kind), Ok(BodyKind::CorpsDistrict));
        for bad in ["group-01001", "group-0", "group--1", "group-+5", "kar-1001", "group", "1001", "group-99999999999"]
        {
            assert!(bad.parse::<BodyKey>().is_err(), "{bad} ska inte godtas");
        }
    }
}
