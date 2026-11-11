//! Incident Engine: turns raw hit reports into incidents, once each.
//!
//! A hit report can arrive twice for one blow (a sweep and the main hit in the
//! same tick, two hooks on one path) and players can click fast. The engine
//! keeps, per (player, mob), the tick of the last accepted incident, and per
//! player the ticks of recent incidents, and refuses what is a duplicate or
//! over the rate limit. Both are saved, so a restart does not reset them.

use crate::config::{Config, normalize_species};
use crate::ids::{IncidentId, MobId, PlayerId, Tick};
use crate::serde_util::map_as_pairs;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// What the adapter saw: a mob's health went down from a hit.
#[derive(Clone, Debug, PartialEq)]
pub struct HitEvent {
    /// The player behind the hit, when the damage source names one.
    pub attacker: Option<PlayerId>,
    /// The player's name, for messages; not an identity.
    pub attacker_name: Option<String>,
    pub victim: MobId,
    /// The victim's current entity id (to find it again for effects).
    pub victim_entity: u32,
    /// The mob kind (`minecraft:chicken` or `chicken`).
    pub species: String,
    /// The mob's custom name, if it has one.
    pub victim_name: Option<String>,
    pub tick: Tick,
    pub position: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentKind {
    /// A player hit a tracked mob.
    Assault,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Incident {
    pub id: IncidentId,
    pub kind: IncidentKind,
    pub offender: PlayerId,
    pub offender_name: Option<String>,
    pub victim: MobId,
    pub species: String,
    pub victim_name: Option<String>,
    pub tick: Tick,
    pub position: [f64; 3],
}

/// Why a hit made no incident.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// No player behind the damage (a fall, a dispenser, an unknown source).
    NoAttacker,
    /// Not a mob kind that keeps grudges.
    Untracked,
    /// The same player hit the same mob in the same tick already.
    Duplicate,
    /// Inside the per-mob cooldown or over the player's burst limit.
    RateLimited,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IncidentEngine {
    next_id: u64,
    /// The newest incidents, oldest first.
    log: VecDeque<Incident>,
    /// The tick of the last accepted incident of each (player, mob).
    #[serde(with = "map_as_pairs")]
    last: BTreeMap<(PlayerId, MobId), Tick>,
    /// Each player's recent incident ticks, oldest first.
    #[serde(with = "map_as_pairs")]
    recent: BTreeMap<PlayerId, VecDeque<Tick>>,
}

impl IncidentEngine {
    /// The incident `hit` makes, or why it makes none. Accepting it records
    /// it in the log and the limits.
    pub fn intake(&mut self, hit: &HitEvent, config: &Config) -> Result<Incident, Rejection> {
        let offender = hit.attacker.ok_or(Rejection::NoAttacker)?;
        if !config.tracks(&hit.species) {
            return Err(Rejection::Untracked);
        }
        let key = (offender, hit.victim);
        if let Some(&last) = self.last.get(&key) {
            if last == hit.tick {
                return Err(Rejection::Duplicate);
            }
            // A tick earlier than the last one (a hook replay, a clock set
            // back) is treated as inside the cooldown too.
            if hit.tick - last < config.pair_cooldown {
                return Err(Rejection::RateLimited);
            }
        }
        let recent = self.recent.entry(offender).or_default();
        while recent
            .front()
            .is_some_and(|&t| hit.tick - t >= config.player_window)
        {
            recent.pop_front();
        }
        if recent.len() >= config.player_burst {
            return Err(Rejection::RateLimited);
        }
        recent.push_back(hit.tick);
        self.last.insert(key, hit.tick);
        self.next_id += 1;
        let incident = Incident {
            id: IncidentId(self.next_id),
            kind: IncidentKind::Assault,
            offender,
            offender_name: hit.attacker_name.clone(),
            victim: hit.victim,
            species: normalize_species(&hit.species),
            victim_name: hit.victim_name.clone(),
            tick: hit.tick,
            position: hit.position,
        };
        self.log.push_back(incident.clone());
        while self.log.len() > config.max_incidents {
            self.log.pop_front();
        }
        Ok(incident)
    }

    /// Forgets limits that can no longer refuse anything at `now`.
    pub fn prune(&mut self, now: Tick, config: &Config) {
        self.last.retain(|_, &mut t| now - t < config.pair_cooldown);
        self.recent.retain(|_, ticks| {
            ticks.retain(|&t| now - t < config.player_window);
            !ticks.is_empty()
        });
    }

    /// The kept incidents, oldest first.
    pub fn log(&self) -> impl DoubleEndedIterator<Item = &Incident> {
        self.log.iter()
    }

    pub fn get(&self, id: IncidentId) -> Option<&Incident> {
        self.log.iter().find(|i| i.id == id)
    }

    /// How many incidents were ever accepted (the last id).
    pub fn total(&self) -> u64 {
        self.next_id
    }

    /// Limit records kept now (for the debug report).
    pub fn tracked_limits(&self) -> (usize, usize) {
        (self.last.len(), self.recent.len())
    }
}
