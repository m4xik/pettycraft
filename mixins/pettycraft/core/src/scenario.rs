//! Scenario State Machine: one active case per (mob, player) grudge.
//!
//! ```text
//! Grievance --offence--> Brooding --offence (≥ file_case_after)--> Filed
//!     |                      |
//!     +-------expiry---------+--> Dismissed
//! ```
//!
//! `Filed` waits for a hearing. The hearing itself (the court scene, its NPCs)
//! is a later milestone; nothing in this version leaves `Filed` but a manual
//! dismissal.

use crate::config::Config;
use crate::ids::{CaseId, IncidentId, MobId, PlayerId, Tick};
use crate::serde_util::map_as_pairs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaseState {
    /// One offence: the mob is upset.
    Grievance,
    /// Repeated offences: the mob is gathering evidence.
    Brooding,
    /// Enough offences: the case awaits a hearing.
    Filed,
    /// Closed without a hearing.
    Dismissed,
}

impl CaseState {
    pub fn is_open(self) -> bool {
        !matches!(self, Self::Dismissed)
    }
}

impl fmt::Display for CaseState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Grievance => "grievance",
            Self::Brooding => "brooding",
            Self::Filed => "filed",
            Self::Dismissed => "dismissed",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseEvent {
    Offence {
        tick: Tick,
        offences: u32,
    },
    /// Time passed with no offence.
    Quiet {
        now: Tick,
    },
    Dismiss,
}

/// An active (or just closed) case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveCase {
    pub id: CaseId,
    pub plaintiff: MobId,
    pub defendant: PlayerId,
    pub state: CaseState,
    pub opened: Tick,
    pub updated: Tick,
    pub evidence: Vec<IncidentId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub case: CaseId,
    pub from: CaseState,
    pub to: CaseState,
}

impl ActiveCase {
    /// The state `event` leads to from `self.state`, if it changes.
    pub fn next(&self, event: CaseEvent, config: &Config) -> Option<CaseState> {
        use CaseState::*;
        let to = match (self.state, event) {
            (Dismissed, _) => return None,
            (_, CaseEvent::Dismiss) => Dismissed,
            (Grievance | Brooding, CaseEvent::Offence { offences, .. })
                if offences >= config.file_case_after =>
            {
                Filed
            }
            (Grievance, CaseEvent::Offence { .. }) => Brooding,
            (Grievance | Brooding, CaseEvent::Quiet { now })
                if now - self.updated >= config.case_expiry =>
            {
                Dismissed
            }
            _ => return None,
        };
        (to != self.state).then_some(to)
    }

    fn apply(&mut self, event: CaseEvent, config: &Config) -> Option<Transition> {
        if let CaseEvent::Offence { tick, .. } = event {
            self.updated = self.updated.max(tick);
        }
        let to = self.next(event, config)?;
        let from = std::mem::replace(&mut self.state, to);
        Some(Transition {
            case: self.id,
            from,
            to,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseUpdate {
    pub case: CaseId,
    pub state: CaseState,
    pub opened: bool,
    pub transition: Option<Transition>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenarioMachine {
    next_id: u64,
    cases: BTreeMap<CaseId, ActiveCase>,
    /// The open case of each (mob, player).
    #[serde(with = "map_as_pairs")]
    open: BTreeMap<(MobId, PlayerId), CaseId>,
}

impl ScenarioMachine {
    /// Opens a case for a first offence, or advances the open one.
    pub fn offence(
        &mut self,
        plaintiff: MobId,
        defendant: PlayerId,
        incident: IncidentId,
        tick: Tick,
        offences: u32,
        config: &Config,
    ) -> CaseUpdate {
        let key = (plaintiff, defendant);
        let (id, opened) = match self.open.get(&key) {
            Some(&id) => (id, false),
            None => {
                self.next_id += 1;
                let id = CaseId(self.next_id);
                self.cases.insert(
                    id,
                    ActiveCase {
                        id,
                        plaintiff,
                        defendant,
                        state: CaseState::Grievance,
                        opened: tick,
                        updated: tick,
                        evidence: Vec::new(),
                    },
                );
                self.open.insert(key, id);
                (id, true)
            }
        };
        let case = self.cases.get_mut(&id).expect("an open case is kept");
        case.evidence.push(incident);
        if case.evidence.len() > config.max_grudge_incidents {
            case.evidence.remove(0);
        }
        // A new case already stands at Grievance for its first offence.
        let transition = if opened {
            None
        } else {
            case.apply(CaseEvent::Offence { tick, offences }, config)
        };
        let state = case.state;
        CaseUpdate {
            case: id,
            state,
            opened,
            transition,
        }
    }

    /// Dismisses quiet grievances; closed cases leave the open index.
    pub fn tick(&mut self, now: Tick, config: &Config) -> Vec<Transition> {
        let mut out = Vec::new();
        for case in self.cases.values_mut() {
            if let Some(t) = case.apply(CaseEvent::Quiet { now }, config) {
                out.push(t);
            }
        }
        self.reindex();
        out
    }

    pub fn dismiss(&mut self, id: CaseId, config: &Config) -> Option<Transition> {
        let t = self.cases.get_mut(&id)?.apply(CaseEvent::Dismiss, config);
        self.reindex();
        t
    }

    fn reindex(&mut self) {
        let cases = &self.cases;
        self.open
            .retain(|_, id| cases.get(id).is_some_and(|c| c.state.is_open()));
        // Closed cases are not kept forever.
        self.cases.retain(|_, c| c.state.is_open());
    }

    pub fn get(&self, id: CaseId) -> Option<&ActiveCase> {
        self.cases.get(&id)
    }

    pub fn open_case(&self, plaintiff: MobId, defendant: PlayerId) -> Option<&ActiveCase> {
        self.open
            .get(&(plaintiff, defendant))
            .and_then(|id| self.cases.get(id))
    }

    pub fn against(&self, defendant: PlayerId) -> impl Iterator<Item = &ActiveCase> {
        self.cases
            .values()
            .filter(move |c| c.defendant == defendant)
    }

    pub fn all(&self) -> impl Iterator<Item = &ActiveCase> {
        self.cases.values()
    }
}
