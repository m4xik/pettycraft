//! The per-world runtime: one [`Pettycraft`] per world, driven by the adapter.

use crate::config::Config;
use crate::director::{self, Scene};
use crate::grudge::GrudgeEngine;
use crate::ids::{CaseId, PlayerId, Tick};
use crate::incident::{HitEvent, Incident, IncidentEngine, Rejection};
use crate::reputation::ReputationSystem;
use crate::scenario::{CaseState, ScenarioMachine, Transition};
use crate::schedule::{Action, EffectKind, Schedule};
use serde::{Deserialize, Serialize};

/// Grudges listed in the debug report.
const REPORT_GRUDGES: usize = 8;

/// Counters for the debug report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub hits_seen: u64,
    pub incidents: u64,
    pub duplicates: u64,
    pub rate_limited: u64,
    pub no_attacker: u64,
    pub untracked: u64,
    pub messages_sent: u64,
    pub messages_dropped: u64,
}

/// Everything saved with a world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldState {
    pub incidents: IncidentEngine,
    pub grudges: GrudgeEngine,
    pub reputation: ReputationSystem,
    pub cases: ScenarioMachine,
    pub schedule: Schedule,
    pub stats: Stats,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    pub incident: Incident,
    pub offences: u32,
    pub case: CaseId,
    pub case_state: CaseState,
    pub effect: EffectKind,
    pub line: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HitOutcome {
    Recorded(Box<Recorded>),
    Rejected(Rejection),
}

impl HitOutcome {
    pub fn recorded(&self) -> Option<&Recorded> {
        match self {
            Self::Recorded(r) => Some(r),
            Self::Rejected(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pettycraft {
    pub config: Config,
    state: WorldState,
    dirty: bool,
}

impl Pettycraft {
    pub fn new(config: Config) -> Self {
        Self::with_state(config, WorldState::default())
    }

    /// A world's runtime from its saved state.
    pub fn with_state(config: Config, state: WorldState) -> Self {
        Self {
            config,
            state,
            dirty: false,
        }
    }

    pub fn state(&self) -> &WorldState {
        &self.state
    }

    /// A tracked mob was hurt.
    pub fn on_hit(&mut self, hit: &HitEvent) -> HitOutcome {
        let s = &mut self.state;
        s.stats.hits_seen += 1;
        let incident = match s.incidents.intake(hit, &self.config) {
            Ok(i) => i,
            Err(r) => {
                match r {
                    Rejection::NoAttacker => s.stats.no_attacker += 1,
                    Rejection::Untracked => s.stats.untracked += 1,
                    Rejection::Duplicate => s.stats.duplicates += 1,
                    Rejection::RateLimited => s.stats.rate_limited += 1,
                }
                return HitOutcome::Rejected(r);
            }
        };
        s.stats.incidents += 1;
        let grudge = s.grudges.record(&incident, &self.config);
        // Repeat offences weigh more, up to a point.
        let severity = grudge.offences.min(5) as i32;
        s.reputation
            .penalize(incident.offender, severity, incident.tick);
        let case = s.cases.offence(
            incident.victim,
            incident.offender,
            incident.id,
            incident.tick,
            grudge.offences,
            &self.config,
        );
        let reaction = director::direct(
            &Scene {
                incident: incident.id.0,
                offences: grudge.offences,
                case: case.state,
                case_changed: case.transition.is_some(),
                standing: s.reputation.standing(incident.offender),
                victim_name: incident.victim_name.as_deref(),
            },
            &self.config,
        );
        let max = self.config.max_pending;
        s.schedule.push(
            incident.tick + reaction.effect_delay,
            Action::Effect {
                victim: incident.victim,
                entity: hit.victim_entity,
                fallback: incident.position,
                kind: reaction.effect,
            },
            max,
        );
        s.schedule.push(
            incident.tick + reaction.line_delay,
            Action::Tell {
                player: incident.offender,
                text: reaction.line.clone(),
                attempts: 0,
            },
            max,
        );
        self.dirty = true;
        HitOutcome::Recorded(Box::new(Recorded {
            offences: grudge.offences,
            case: case.case,
            case_state: case.state,
            effect: reaction.effect,
            line: reaction.line,
            incident,
        }))
    }

    /// Whether [`Self::due`] would return anything (one lookup).
    pub fn has_due(&self, now: Tick) -> bool {
        self.state.schedule.has_due(now)
    }

    /// The actions due at `now`. A `Tell` the adapter cannot deliver goes
    /// back through [`Self::undelivered`].
    pub fn due(&mut self, now: Tick) -> Vec<Action> {
        let due = self.state.schedule.pop_due(now);
        if !due.is_empty() {
            self.dirty = true;
        }
        due
    }

    /// A due message reached its player.
    pub fn delivered(&mut self) {
        self.state.stats.messages_sent += 1;
    }

    /// A due action could not be carried out at `now` (its player is away).
    /// Messages are retried a few times; effects are dropped.
    pub fn undelivered(&mut self, action: Action, now: Tick) {
        if let Action::Tell {
            player,
            text,
            attempts,
        } = action
        {
            let attempts = attempts.saturating_add(1);
            if attempts < self.config.message_attempts {
                self.state.schedule.push(
                    now + self.config.message_retry,
                    Action::Tell {
                        player,
                        text,
                        attempts,
                    },
                    self.config.max_pending,
                );
            } else {
                self.state.stats.messages_dropped += 1;
            }
            self.dirty = true;
        }
    }

    /// Infrequent upkeep (the adapter calls it every few seconds): forgets
    /// expired limits and dismisses quiet cases.
    pub fn maintain(&mut self, now: Tick) -> Vec<Transition> {
        self.state.incidents.prune(now, &self.config);
        let t = self.state.cases.tick(now, &self.config);
        if !t.is_empty() {
            self.dirty = true;
        }
        t
    }

    /// Whether there is anything new to save since the last call.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// The debug report: the totals, and `player`'s record when given.
    pub fn report(&self, player: Option<PlayerId>) -> Vec<String> {
        let s = &self.state;
        let st = &s.stats;
        let (pairs, players) = s.incidents.tracked_limits();
        let mut out = vec![
            format!(
                "PETTYCRAFT: {} incidents, {} grudges, {} open cases, {} pending",
                s.incidents.total(),
                s.grudges.len(),
                s.cases.all().count(),
                s.schedule.len(),
            ),
            format!(
                "hits {} | dup {} | limited {} | no attacker {} | other mobs {}",
                st.hits_seen, st.duplicates, st.rate_limited, st.no_attacker, st.untracked,
            ),
            format!(
                "messages {} sent, {} dropped | limits {pairs}/{players}",
                st.messages_sent, st.messages_dropped,
            ),
        ];
        if let Some(p) = player {
            let r = s.reputation.get(p);
            out.push(format!(
                "you ({}): score {}, {} offences, {}",
                p.0.short(),
                r.score,
                r.offences,
                s.reputation.standing(p),
            ));
            // The most recent grudges first; the chat is not a database.
            let mut grudges: Vec<_> = s.grudges.against(p).collect();
            grudges.sort_by_key(|g| std::cmp::Reverse(g.last));
            for g in grudges.iter().take(REPORT_GRUDGES) {
                let case = s.cases.open_case(g.holder, p).map_or_else(
                    || "no case".to_owned(),
                    |c| format!("case #{} {}", c.id.0, c.state),
                );
                out.push(format!(
                    "  {} {}: {}x, intensity {}, {case}",
                    g.species.trim_start_matches("minecraft:"),
                    g.holder.0.short(),
                    g.offences,
                    g.intensity,
                ));
            }
            if grudges.len() > REPORT_GRUDGES {
                out.push(format!("  ...and {} more", grudges.len() - REPORT_GRUDGES));
            }
            if let Some(last) = s.incidents.log().rev().find(|i| i.offender == p) {
                out.push(format!(
                    "  last #{} at tick {} ({:.0} {:.0} {:.0})",
                    last.id.0, last.tick, last.position[0], last.position[1], last.position[2],
                ));
            }
        }
        out
    }
}
