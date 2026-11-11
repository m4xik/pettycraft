//! Tunables. Code defaults only for now; nothing is saved with the world.

/// 20 ticks per second.
pub const SECOND: i64 = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Mob kinds that remember being hit (`minecraft:chicken`).
    pub tracked_species: Vec<String>,
    /// Hits by the same player on the same mob closer than this together count
    /// as one incident.
    pub pair_cooldown: i64,
    /// At most `player_burst` incidents per player in any `player_window`.
    pub player_window: i64,
    pub player_burst: usize,
    /// Ticks from the hit to the mob's visible indignation.
    pub effect_delay: i64,
    /// Ticks from the hit to the message the offender gets.
    pub message_delay: i64,
    /// An undelivered message (player away) is tried again after this, at
    /// most `message_attempts` times in all.
    pub message_retry: i64,
    pub message_attempts: u8,
    /// Offences against one mob that turn its grievance into a filed case.
    pub file_case_after: u32,
    /// A grievance or brooding case with no new offence for this long is
    /// dismissed.
    pub case_expiry: i64,
    /// Bounds on what is kept, so the save stays small.
    pub max_incidents: usize,
    pub max_pending: usize,
    pub max_grudge_incidents: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tracked_species: vec!["minecraft:chicken".to_owned()],
            pair_cooldown: 2 * SECOND,
            player_window: 60 * SECOND,
            player_burst: 8,
            effect_delay: 4,
            message_delay: 3 * SECOND,
            message_retry: 5 * SECOND,
            message_attempts: 3,
            file_case_after: 3,
            case_expiry: 20 * 60 * SECOND,
            max_incidents: 256,
            max_pending: 64,
            max_grudge_incidents: 32,
        }
    }
}

/// `minecraft:chicken` for `chicken` or `minecraft:chicken`, as Horizon's
/// `attack_mob` normalizes a mob kind.
pub fn normalize_species(id: &str) -> String {
    format!("minecraft:{}", id.trim_start_matches("minecraft:"))
}

impl Config {
    pub fn tracks(&self, species: &str) -> bool {
        self.tracked_species.contains(&normalize_species(species))
    }
}
