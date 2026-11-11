//! Reputation System: a player's standing with the mobs, all of them.

use crate::ids::{PlayerId, Tick};
use crate::serde_util::map_as_pairs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reputation {
    /// 0 is neutral; offences lower it.
    pub score: i32,
    pub offences: u32,
    pub last_offence: Option<Tick>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Neutral,
    Suspicious,
    Notorious,
    Wanted,
}

impl Standing {
    pub fn of(score: i32) -> Self {
        match score {
            s if s > -3 => Self::Neutral,
            s if s > -10 => Self::Suspicious,
            s if s > -25 => Self::Notorious,
            _ => Self::Wanted,
        }
    }
}

impl fmt::Display for Standing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Neutral => "neutral",
            Self::Suspicious => "suspicious",
            Self::Notorious => "notorious",
            Self::Wanted => "wanted",
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReputationSystem {
    #[serde(with = "map_as_pairs")]
    players: BTreeMap<PlayerId, Reputation>,
}

impl ReputationSystem {
    /// Lowers `player`'s score by `severity`; their reputation after it.
    pub fn penalize(&mut self, player: PlayerId, severity: i32, tick: Tick) -> Reputation {
        let r = self.players.entry(player).or_default();
        r.score = r.score.saturating_sub(severity);
        r.offences += 1;
        r.last_offence = Some(tick);
        r.clone()
    }

    pub fn get(&self, player: PlayerId) -> Reputation {
        self.players.get(&player).cloned().unwrap_or_default()
    }

    pub fn standing(&self, player: PlayerId) -> Standing {
        Standing::of(self.get(player).score)
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}
