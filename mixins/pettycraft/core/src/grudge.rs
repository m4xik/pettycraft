//! Grudge Engine: what one mob holds against one player.

use crate::config::Config;
use crate::ids::{IncidentId, MobId, PlayerId, Tick};
use crate::incident::Incident;
use crate::serde_util::map_as_pairs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Intensity is 0..=MAX_INTENSITY.
pub const MAX_INTENSITY: u8 = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grudge {
    pub holder: MobId,
    pub offender: PlayerId,
    pub species: String,
    /// Offences ever counted, including ones whose incidents were dropped
    /// from the list below.
    pub offences: u32,
    /// The newest incidents behind it, oldest first.
    pub incidents: Vec<IncidentId>,
    pub intensity: u8,
    pub first: Tick,
    pub last: Tick,
}

impl Grudge {
    fn intensity_for(offences: u32) -> u8 {
        // 2, 4, 6... : a chicken does not forget, it escalates.
        offences
            .saturating_mul(2)
            .min(u32::from(MAX_INTENSITY))
            .try_into()
            .unwrap_or(MAX_INTENSITY)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrudgeUpdate {
    pub offences: u32,
    pub intensity: u8,
    pub first_offence: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrudgeEngine {
    #[serde(with = "map_as_pairs")]
    grudges: BTreeMap<(MobId, PlayerId), Grudge>,
}

impl GrudgeEngine {
    pub fn record(&mut self, incident: &Incident, config: &Config) -> GrudgeUpdate {
        let grudge = self
            .grudges
            .entry((incident.victim, incident.offender))
            .or_insert_with(|| Grudge {
                holder: incident.victim,
                offender: incident.offender,
                species: incident.species.clone(),
                offences: 0,
                incidents: Vec::new(),
                intensity: 0,
                first: incident.tick,
                last: incident.tick,
            });
        grudge.offences += 1;
        grudge.incidents.push(incident.id);
        if grudge.incidents.len() > config.max_grudge_incidents {
            let extra = grudge.incidents.len() - config.max_grudge_incidents;
            grudge.incidents.drain(..extra);
        }
        grudge.intensity = Grudge::intensity_for(grudge.offences);
        grudge.last = incident.tick;
        GrudgeUpdate {
            offences: grudge.offences,
            intensity: grudge.intensity,
            first_offence: grudge.offences == 1,
        }
    }

    pub fn get(&self, holder: MobId, offender: PlayerId) -> Option<&Grudge> {
        self.grudges.get(&(holder, offender))
    }

    /// Grudges against `offender`.
    pub fn against(&self, offender: PlayerId) -> impl Iterator<Item = &Grudge> {
        self.grudges
            .values()
            .filter(move |g| g.offender == offender)
    }

    pub fn all(&self) -> impl Iterator<Item = &Grudge> {
        self.grudges.values()
    }

    pub fn len(&self) -> usize {
        self.grudges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grudges.is_empty()
    }
}
