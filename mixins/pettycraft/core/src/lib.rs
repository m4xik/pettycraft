//! PETTYCRAFT's game rules, independent of Horizon.
//!
//! Every type here is plain Rust: the Horizon adapter (the `pettycraft`
//! support module of the mixin bundle) turns native events into [`HitEvent`]s,
//! feeds them to a [`Pettycraft`] per world, and carries out the [`Action`]s it
//! hands back. Nothing here reads a clock, a random source or the file system
//! on its own, so the same inputs always give the same state.
//!
//! Layers:
//! - [`incident`]: the Incident Engine (validation, de-duplication, rate limits);
//! - [`grudge`]: the Grudge Engine (what each mob holds against each player);
//! - [`reputation`]: the Reputation System (a player's standing with mobs);
//! - [`director`]: the Comedy Director (which reaction, which line, when);
//! - [`scenario`]: the Scenario State Machine (one active case per grudge);
//! - [`schedule`]: delayed actions, cheap to poll every tick;
//! - [`engine`]: the per-world runtime tying them together;
//! - [`store`]: the versioned JSON save format.

pub mod config;
pub mod director;
pub mod engine;
pub mod grudge;
pub mod ids;
pub mod incident;
pub mod reputation;
pub mod scenario;
pub mod schedule;
pub mod store;

mod serde_util;

pub use config::Config;
pub use engine::{HitOutcome, Pettycraft, Stats, WorldState};
pub use ids::{CaseId, IncidentId, MobId, PlayerId, Tick, Uuid};
pub use incident::{HitEvent, Incident, IncidentKind, Rejection};
pub use schedule::{Action, EffectKind};
