//! Identifiers. UUIDs keep Minecraft's four-int form (`[I; a, b, c, d]`, the
//! form Horizon's `acting_player_uuid` and `mob_uuid` return) and save as the
//! usual hyphenated text.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A game tick: Horizon's `world.game_time`, which the world saves, so ticks
/// stay comparable across restarts.
pub type Tick = i64;

/// A UUID in Minecraft's int-array form, most significant int first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Uuid(pub [i32; 4]);

impl Uuid {
    pub fn from_u128(v: u128) -> Self {
        Self([
            (v >> 96) as i32,
            (v >> 64) as i32,
            (v >> 32) as i32,
            v as i32,
        ])
    }
    pub fn as_u128(self) -> u128 {
        self.0
            .iter()
            .fold(0u128, |acc, &part| (acc << 32) | u128::from(part as u32))
    }
    /// The first 8 hex digits, for reports.
    pub fn short(self) -> String {
        format!("{:08x}", self.0[0] as u32)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.as_u128();
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            (v >> 96) as u32,
            (v >> 80) as u16,
            (v >> 64) as u16,
            (v >> 48) as u16,
            v & 0xffff_ffff_ffff
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UuidError(String);

impl fmt::Display for UuidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a UUID: {:?}", self.0)
    }
}

impl std::error::Error for UuidError {}

impl FromStr for Uuid {
    type Err = UuidError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex: String = s.chars().filter(|&c| c != '-').collect();
        if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(UuidError(s.to_owned()));
        }
        u128::from_str_radix(&hex, 16)
            .map(Self::from_u128)
            .map_err(|_| UuidError(s.to_owned()))
    }
}

impl From<Uuid> for String {
    fn from(u: Uuid) -> Self {
        u.to_string()
    }
}

impl TryFrom<String> for Uuid {
    type Error = UuidError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// The player who did something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub Uuid);

/// A mob by its persistent UUID (not its entity id, which a reload renumbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MobId(pub Uuid);

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for MobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IncidentId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaseId(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_text_round_trips_and_matches_minecraft_ints() {
        let u: Uuid = "069a79f4-44e9-4726-a5be-fca90e38aaf5".parse().unwrap();
        assert_eq!(u.to_string(), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        assert_eq!(u.0[0], 0x069a79f4);
        assert_eq!(Uuid::from_u128(u.as_u128()), u);
        let negative = Uuid([-1, 0, i32::MIN, 7]);
        assert_eq!(negative.to_string().parse::<Uuid>().unwrap(), negative);
        assert!("not-a-uuid".parse::<Uuid>().is_err());
        assert_eq!(u.short(), "069a79f4");
    }
}
