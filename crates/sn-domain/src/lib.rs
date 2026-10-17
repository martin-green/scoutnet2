//! Rena domäntyper för Scoutnet 2: inga beroenden till databas eller webb.

pub mod body;
pub mod personnummer;
pub mod term;

pub use body::{BodyKey, BodyKind, InvalidBodyKey, TroopType};
pub use personnummer::{Personnummer, PersonnummerError};
pub use term::{Half, Term};
