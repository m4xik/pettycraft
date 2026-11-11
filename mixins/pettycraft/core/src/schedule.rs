//! Delayed actions, ordered by due tick. Polling when nothing is due is one
//! map lookup, so the adapter can ask every tick.

use crate::ids::{MobId, PlayerId, Tick};
use crate::serde_util::map_as_pairs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    /// A first offence: a few angry puffs.
    Indignation,
    /// A repeat offence: more of them.
    Outrage,
    /// The case was filed.
    CaseFiled,
}

impl EffectKind {
    /// How many particles the adapter shows.
    pub fn particles(self) -> u32 {
        match self {
            Self::Indignation => 3,
            Self::Outrage => 6,
            Self::CaseFiled => 10,
        }
    }
}

/// What the adapter carries out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// A visual reaction at the mob (found by entity id, else at `fallback`).
    Effect {
        victim: MobId,
        entity: u32,
        fallback: [f64; 3],
        kind: EffectKind,
    },
    /// A chat line to one player.
    Tell {
        player: PlayerId,
        text: String,
        attempts: u8,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    next_seq: u64,
    #[serde(with = "map_as_pairs")]
    queue: BTreeMap<(Tick, u64), Action>,
}

impl Schedule {
    /// Queues `action` for `due`; past `max`, the soonest-due entries are
    /// dropped first (they are the stalest).
    pub fn push(&mut self, due: Tick, action: Action, max: usize) {
        self.next_seq += 1;
        self.queue.insert((due, self.next_seq), action);
        while self.queue.len() > max {
            self.queue.pop_first();
        }
    }

    /// Whether anything is due at `now` (cheap).
    pub fn has_due(&self, now: Tick) -> bool {
        self.queue
            .first_key_value()
            .is_some_and(|(&(due, _), _)| due <= now)
    }

    /// Removes and returns what is due at `now`, in due order.
    pub fn pop_due(&mut self, now: Tick) -> Vec<Action> {
        let mut out = Vec::new();
        while self.has_due(now) {
            if let Some((_, a)) = self.queue.pop_first() {
                out.push(a);
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Tick, &Action)> {
        self.queue.iter().map(|(&(due, _), a)| (due, a))
    }
}
