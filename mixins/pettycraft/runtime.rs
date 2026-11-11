// PETTYCRAFT's Horizon adapter (the pettycraft_horizon layer), injected by
// horizon-mixins as `crate::game::pettycraft` at the top of
// crates/horizon-client/src/game.rs. It is a child module of `game`, so it
// reads `Game`'s own fields; it adds no field to `Game` and changes no
// existing code. The rules live in the `pettycraft_core` bundle crate.
//
// Everything runs on the authoritative game only: the integrated server's
// thread in single player and on LAN, a dedicated server, or the deprecated
// in-frame world. A client connected to a server (`remote` is set) does
// nothing here. State is per world (keyed by the world's save folder) in a
// thread-local of the thread that runs the world, as Veinminer keeps its own.
//
// Hooks (see horizon-mixin.toml):
//   hurt_by_player HEAD  -> before_hurt   remember who is hitting which mob
//   mob_damage_event HEAD -> damaged      the hit landed: report it
//   native_tick HEAD     -> tick          due effects/messages, upkeep, saves
//   native_world_lifecycle HEAD -> lifecycle  save on close
//   register_commands HEAD -> register_commands   /pettycraft in the tree
//   perform_command HEAD   -> perform_command     runs it

use super::Game;
use horizon_client::mod_api::{NativeCommand, NativeServerPlayer};
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_client::text::Text;
use pettycraft_core::{
    Action, Config, HitEvent, HitOutcome, MobId, Pettycraft, PlayerId, Tick, Uuid, store,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) const MOD_ID: &str = "pettycraft";
/// Ticks between upkeep passes (expired limits, quiet cases).
const UPKEEP_EVERY: Tick = 100;
/// A changed state is saved at most this often (and when the world closes).
const SAVE_EVERY: Tick = 1200;

/// The hit `hurt_by_player` is about to deal.
#[derive(Clone, Copy)]
struct PendingHit {
    mob: u32,
    by_player: bool,
    /// Valid only in this tick: `/damage` also reaches `mob_damage_event`,
    /// without `hurt_by_player`, and must not inherit a stale record.
    tick: Tick,
}

struct World {
    runtime: Pettycraft,
    path: PathBuf,
    /// False when the save on disk is from a newer PETTYCRAFT: never
    /// overwrite it.
    writable: bool,
    dirty: bool,
    last_save: Tick,
    last_upkeep: Tick,
}

thread_local! {
    static WORLDS: RefCell<HashMap<PathBuf, World>> = RefCell::new(HashMap::new());
    static PENDING: Cell<Option<PendingHit>> = const { Cell::new(None) };
}

fn authoritative(game: &Game<'_>) -> bool {
    game.remote.is_none() && !game.world_root.as_os_str().is_empty() && game.native_on(MOD_ID)
}

fn open(root: &Path, now: Tick) -> World {
    let path = store::path_in(root);
    let (state, writable) = match store::load_or_quarantine(&path) {
        Ok((state, warning)) => {
            if let Some(w) = warning {
                log::warn!("[pettycraft] {w}");
            }
            (state, true)
        }
        Err(e) => {
            log::error!(
                "[pettycraft] {}: {e}; not saving this world",
                path.display()
            );
            (Default::default(), false)
        }
    };
    World {
        runtime: Pettycraft::with_state(Config::default(), state),
        path,
        writable,
        dirty: false,
        last_save: now,
        last_upkeep: now,
    }
}

/// Runs `f` with this world's state, taken out of the map meanwhile so that a
/// nested call (a game method reaching another hook) cannot double-borrow it.
fn with_world<T>(game: &mut Game<'_>, f: impl FnOnce(&mut Game<'_>, &mut World) -> T) -> T {
    let key = game.world_root.clone();
    let now = game.world.game_time;
    let mut world = WORLDS
        .with(|w| w.borrow_mut().remove(&key))
        .unwrap_or_else(|| open(&key, now));
    let out = f(game, &mut world);
    world.dirty |= world.runtime.take_dirty();
    WORLDS.with(|w| w.borrow_mut().insert(key, world));
    out
}

fn save(world: &mut World, now: Tick) {
    world.dirty |= world.runtime.take_dirty();
    if !world.dirty || !world.writable {
        return;
    }
    match store::save(&world.path, world.runtime.state()) {
        Ok(()) => {
            world.dirty = false;
            world.last_save = now;
        }
        Err(e) => log::error!("[pettycraft] saving {}: {e}", world.path.display()),
    }
}

/// HEAD of `Game::hurt_by_player`: the player of this turn is about to hit
/// the mob at `index` with `source`.
pub(super) fn before_hurt(game: &mut Game<'_>, index: usize, source: horizon_client::mobs::Damage) {
    let Some(mob) = game.mobs.get(index) else {
        PENDING.set(None);
        return;
    };
    PENDING.set(Some(PendingHit {
        mob: mob.id,
        by_player: source.attacker == Some(horizon_client::mobs::Who::Player),
        tick: game.world.game_time,
    }));
}

/// HEAD of `Game::mob_damage_event`, which only `hurt_by_player` calls, once
/// the mob's health went down.
pub(super) fn damaged(game: &mut Game<'_>, id: u32) {
    let pending = PENDING.take();
    if !authoritative(game) {
        return;
    }
    let Some(mob) = game.mobs.iter().find(|m| m.id == id) else {
        return;
    };
    let species = mob.kind.id.clone();
    let position = [mob.position.x, mob.position.y, mob.position.z];
    let victim_name = mob.custom_name.clone();
    let tick = game.world.game_time;
    with_world(game, |game, world| {
        // Only tracked kinds get a persistent UUID asked for.
        if !world.runtime.config.tracks(&species) {
            return;
        }
        let attacker = pending
            .filter(|p| p.mob == id && p.by_player && p.tick == tick)
            .map(|_| PlayerId(Uuid(game.acting_player_uuid())));
        let hit = HitEvent {
            attacker,
            attacker_name: game.lan_acting_name.clone(),
            victim: MobId(Uuid(game.mob_uuid(id))),
            victim_entity: id,
            species,
            victim_name,
            tick,
            position,
        };
        match world.runtime.on_hit(&hit) {
            HitOutcome::Recorded(r) => log::info!(
                "[pettycraft] incident #{} by {} on {} ({} offences, case {:?})",
                r.incident.id.0,
                r.incident.offender,
                r.incident.victim,
                r.offences,
                r.case_state,
            ),
            HitOutcome::Rejected(why) => log::debug!("[pettycraft] hit on {id} ignored: {why:?}"),
        }
    });
}

/// HEAD of `Game::native_tick`, every tick of every `Game`.
pub(super) fn tick(game: &mut Game<'_>) {
    if !authoritative(game) {
        return;
    }
    let now = game.world.game_time;
    // The common case is one map lookup and nothing to do.
    let busy = WORLDS.with(|w| {
        w.borrow()
            .get(&game.world_root)
            .is_none_or(|w| w.runtime.has_due(now) || now - w.last_upkeep >= UPKEEP_EVERY)
    });
    if !busy {
        return;
    }
    with_world(game, |game, world| {
        for action in world.runtime.due(now) {
            if perform(game, &action) {
                if matches!(action, Action::Tell { .. }) {
                    world.runtime.delivered();
                }
            } else {
                world.runtime.undelivered(action, now);
            }
        }
        // By elapsed time, not `now % N`: game time stands still under
        // `/tick freeze` and must not run upkeep every tick then.
        if now - world.last_upkeep >= UPKEEP_EVERY {
            world.last_upkeep = now;
            world.runtime.maintain(now);
            world.dirty |= world.runtime.take_dirty();
            if world.dirty && now - world.last_save >= SAVE_EVERY {
                save(world, now);
            }
        }
    });
}

/// HEAD of `Game::native_world_lifecycle`: save and forget the world when it
/// closes.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: super::native_lifecycle::WorldPhase) {
    use super::native_lifecycle::WorldPhase;
    if !matches!(
        phase,
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed
    ) {
        return;
    }
    let now = game.world.game_time;
    let worlds: Vec<World> = WORLDS.with(|w| w.borrow_mut().drain().map(|(_, w)| w).collect());
    for mut world in worlds {
        save(&mut world, now);
    }
}

/// Carries out one due action; false when it could not (player away).
fn perform(game: &mut Game<'_>, action: &Action) -> bool {
    match action {
        Action::Effect {
            entity,
            fallback,
            kind,
            ..
        } => {
            let at = game
                .mobs
                .iter()
                .find(|m| m.id == *entity && m.alive())
                .map_or(*fallback, |m| {
                    [
                        m.position.x,
                        m.position.y + f64::from(m.height()),
                        m.position.z,
                    ]
                });
            let at = [at[0], at[1] + 0.3, at[2]];
            let options = ParticleOptions::simple(ParticleType::AngryVillager);
            let spread = [0.3, 0.2, 0.3];
            let count = kind.particles();
            // As `ServerLevel.sendParticles` (game/fishing.rs): this game's
            // own particles, and the packet to the players it serves.
            game.particles
                .send_particles(&options, at, spread, 0.0, count);
            game.lan_send_particles(super::lan_particles::QueuedParticle {
                dimension: game.dim.id(),
                options,
                state: None,
                at,
                spread,
                speed: [0.0; 3],
                count: count as i32,
                randomization: 0,
            });
            true
        }
        Action::Tell { player, text, .. } => tell(game, *player, text),
    }
}

/// A chat line to one player of this world, if they are online.
fn tell(game: &mut Game<'_>, player: PlayerId, text: &str) -> bool {
    let line = || Text::literal(format!("[PETTYCRAFT] {text}"));
    let uuid = player.0.as_u128();
    let guest = game.lan.as_ref().and_then(|host| {
        host.hub
            .guests
            .iter()
            .find(|g| g.uuid == uuid)
            .map(|g| g.conn)
    });
    if let Some(conn) = guest {
        return game
            .for_player(super::commands::Target::Guest(conn), |g| {
                g.system_message(line())
            })
            .is_some();
    }
    // The window's own player: in-frame single player, or a LAN host
    // playing in-frame.
    if game.native_window()
        && game.player.is_some()
        && game.level.ensure_player_uuid() == player.0.0
    {
        game.system_message(line());
        return true;
    }
    false
}

/// The `/pettycraft` usages, for the `register_commands` hook point: the
/// server puts them in every player's command tree, so the chat box knows
/// and completes them. Everyone may run them (level 0): they only read the
/// world's PETTYCRAFT state, or save it.
const USAGES: &[&str] = &[
    "pettycraft",
    "pettycraft status",
    "pettycraft pending",
    "pettycraft save",
];

/// HEAD of the `register_commands` hook point.
pub(super) fn register_commands(commands: &mut Vec<NativeCommand>) {
    commands.extend(USAGES.iter().map(|u| NativeCommand::new(u)));
}

/// HEAD of the `perform_command` hook point: a player's command (without its
/// slash) that a mod registered, with that player's state swapped in.
pub(super) fn perform_command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some(MOD_ID) || !authoritative(game) {
        return;
    }
    *ran = true;
    let actor = PlayerId(Uuid::from_u128(player.uuid));
    let now = game.world.game_time;
    let lines = with_world(game, |_, world| match words.next().unwrap_or("status") {
        "status" => world.runtime.report(Some(actor)),
        "pending" => {
            let schedule = &world.runtime.state().schedule;
            let mut out = vec![format!("{} pending at tick {now}", schedule.len())];
            out.extend(schedule.iter().take(10).map(|(due, a)| match a {
                Action::Effect { kind, entity, .. } => {
                    format!("  {due}: effect {kind:?} at entity {entity}")
                }
                Action::Tell {
                    player, attempts, ..
                } => format!(
                    "  {due}: message to {} (attempt {})",
                    player.0.short(),
                    attempts + 1
                ),
            }));
            out
        }
        "save" => {
            world.dirty = true;
            save(world, now);
            vec![if world.dirty || !world.writable {
                format!("PETTYCRAFT: not saved ({})", world.path.display())
            } else {
                format!("PETTYCRAFT: saved to {}", world.path.display())
            }]
        }
        _ => vec!["Usage: /pettycraft [status|pending|save]".to_owned()],
    });
    for line in lines {
        game.native_feedback(Text::literal(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_file_sits_in_the_world_folder() {
        let p = store::path_in(Path::new("saves/My World"));
        assert!(p.starts_with("saves/My World"));
        assert!(p.ends_with("pettycraft/state.json"));
    }

    #[test]
    fn a_chicken_hit_reaches_the_core_once_per_tick() {
        let mut r = Pettycraft::new(Config::default());
        let hit = HitEvent {
            attacker: Some(PlayerId(Uuid([1, 2, 3, 4]))),
            attacker_name: None,
            victim: MobId(Uuid([5, 6, 7, 8])),
            victim_entity: 9,
            species: "chicken".into(),
            victim_name: None,
            tick: 10,
            position: [0.0; 3],
        };
        assert!(r.on_hit(&hit).recorded().is_some());
        assert!(r.on_hit(&hit).recorded().is_none());
    }
}
