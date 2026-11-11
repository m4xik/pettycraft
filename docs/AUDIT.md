# Technical audit — Horizon native mods for PETTYCRAFT

> **Current pin: `fdc87ae`** (the Horizon the launcher installs here, and the
> commit the player's other native mod pins). The audit below was first made
> at `ba6344a`; porting to `fdc87ae` changed two things:
>
> - `Game::lan_send_particles` now takes one `lan_particles::QueuedParticle
>   { dimension, options, state, at, spread, speed: [f64; 3], count: i32,
>   randomization }` (as `fishing.rs::send_particles` builds it).
> - `mob_damage_event` gained a second caller, the `/damage` command
>   (`commands/world_cmds.rs:499`), which does not go through
>   `hurt_by_player`. The pending-hit record is therefore valid only for the
>   same mob **and the same tick**.
>
> Every other API below type-checks unchanged at `fdc87ae`; the launcher
> builds with horizon-mixins `5a64af9`.

Audited on 2026-11-11. Every symbol below was read in the source of the pinned
revision (`git archive ba6344a`), not in `main`: `main` is far ahead
(native-mods.md went from 156 to 5556 lines, with a named server hook catalog
that **does not exist** at the pin). Line numbers are those of the pin.

## 1. Compatible revisions

| What | Revision | Source |
|---|---|---|
| Horizon | `ba6344a59cb9e755b730dcf02285fc0d33b97d3c` (Nightly 2026-10-30) | `horizon-native-mod-template/compatibility.json` |
| horizon-mixins | `d9c83d7096b7ea5b83678da2efdfd6161f74997e` (0.6.0, plan schema 6) | same |
| `NATIVE_HOOKS_API` at the pin | 3 | `crates/horizon-client/src/game/native_hooks.rs:41` |
| Rust | template: 1.95.0; launcher builds with its own 1.99.1 (`windows-gnu` + MinGW) | template `rust-toolchain.toml`, `docs/native-mods.md` |

The template pins that Nightly, and every native mod of a player's build must
pin the same commit, so PETTYCRAFT pins it too (`compatibility.json`,
`horizon-mixin.toml`).

## 2. How native mods work at the pin

A native mod is a horizon-mixins bundle: `horizon-mixin.toml` + helper `.rs`
files. horizon-mixins rewrites a pristine export of Horizon's source (never the
checkout) and the launcher compiles a custom game binary. Relevant facts:

- **HEAD** hooks prepend a helper call; `cancellable = true` lets it return
  early (`ControlFlow<R>`). HEAD tolerates closures/`?`/macros in the target.
- **RETURN / AFTER_CALL / AFTER_ASSIGN** refuse targets with closures, `?`,
  async (`docs/advanced-injections.md`, `boundary-head-and-modules.md`).
- **`[[module]]`** injects a self-contained module (`pub(crate) mod name`) into
  a source file; as a child of `game` it sees `Game`'s private fields. No
  `//!`, no external `mod` files, no `include!`.
- **`[[crate]]`** (0.6.0) stages a real crate as a workspace member that
  `horizon-client` depends on; dependencies must already be in Horizon's
  `Cargo.lock` (serde 1.0.229 and serde_json 1.0.151 are).
- **Named hooks** (`hook` + `hook_version`) resolve through `native-hooks.json`.
  At the pin it lists only `Game<'_>` hook points (frame, tick, world_lifecycle,
  hud, key, wheel, command, zoom, minimap, mods, settings, settings_click,
  panel_event). **There is no damage, entity, particle or persistence hook.**
- Dedicated servers: native mods are not built yet (`native-mods.md`,
  "Multiplayer"). Single player and LAN host run server-side hooks for everyone.

## 3. Confirmed APIs (exist at the pin, used as-is)

| Need | API at the pin | Where |
|---|---|---|
| Player hits a mob (server side) | `Game::hurt_by_player(&mut self, index: usize, amount: f32, source: Damage) -> bool` — `LivingEntity.hurtServer` of a player's hit; called by melee (`attack_mob`, game.rs:9167), sweep (mace.rs:504), spear (spear.rs:224) | `game/mace.rs:158` |
| The hit landed | `Game::mob_damage_event(&mut self, id: u32)`, called **only** from `hurt_by_player`, only `if hurt && health != old_health` | `game/sculk.rs:565`, `mace.rs:187` |
| Damage source / attacker kind | `horizon_client::mobs::Damage { kind, attacker: Option<Who>, from }` (`Copy`); `Who::{Player, Mob(u32)}` (`PartialEq`) | `mobs.rs:583`, `horizon-client-core/src/who.rs` |
| **UUID of the hitting player** | `Game::acting_player_uuid(&mut self) -> [i32; 4]`: the guest whose turn it is (`lan_acting_uuid`), else the level's player | `game.rs:2872` |
| Who is hitting in single player | the integrated server runs on its own thread; the window joins as a guest ("A single-player owner is a placed guest", lan_actions.rs); attacks run `guest_attack` → `as_guest` → `attack_mob` with `lan_acting_uuid` set | `game/integrated.rs:1`, `lan_actions.rs:1868` |
| Mob persistent UUID | `Game::mob_uuid(&mut self, id: u32) -> [i32; 4]` (entity ids are renumbered on reload; UUIDs are saved) | `game/persist.rs:738` |
| Mob kind, position, name | `Mob { id: u32, kind: Arc<MobType{ id: String }>, position: Vec3, custom_name: Option<String> }`, `Mob::height()`, `Mob::alive()` | `mobs.rs:1006`, `mobs/living.rs:455` |
| Server tick | hook point `native_tick` (named hook `tick` v1): "once a tick in every Game: the window's and the server's own" | `native_hooks.rs:274`, call `game.rs:5514` |
| World open/close | hook point `native_world_lifecycle(context, phase, error_text)` (named hook `world_lifecycle` v1, API 3); `WorldPhase::{WorldOpenReady, WorldOpenFailed, WorldCloseFinal, WorldCloseFailed}`; server-side only, none in the deprecated in-frame path | `native_hooks.rs:279`, `native_lifecycle.rs` |
| World save folder | `Game.world_root: PathBuf` (= `save_root` of the opened world) | `game.rs:1823`, `9526` |
| Game time | `Game.world.game_time: i64` (saved with the world) | `horizon-client-render/src/world.rs:90` |
| Authority | `Game.remote: Option<Remote>` (set on a client connected to a server); `native_window()` | `game.rs:2000`, `native_hooks.rs:357` |
| Particles to players | `ParticleOptions::simple(ParticleType)`, `ParticleEngine::send_particles`, `Game::lan_send_particles(&options, at, spread, speed, count)` (as `fishing.rs::send_particles` does); `ParticleType::AngryVillager` | `lan_particles.rs:367`, `horizon-client-core/src/particle_type.rs:77` |
| Chat line to one player | `Game::for_player(Target::Guest(conn), \|g\| g.system_message(text))`; guests in `LanHost.hub.guests` (`Guest { conn, uuid: u128, .. }`) | `commands/players.rs:153`, `chat.rs:596`, `lan/guest.rs:60` |
| Commands | `Game::run_command(&mut self, &str) -> bool`; `Game::guest_command(host, conn, command, signed)`; `LanHost.commands: Vec<GuestCommand>` (Veinminer's pattern) | `commands.rs:93`, `lan_actions.rs:1937`, `lan_actions.rs:122` |
| Mod on/off | `Game::native_on(mod_id)` | `native_hooks.rs:346` |

## 4. What needed our own mixin (no hook point exists)

| Need | Mixin | Why not something else |
|---|---|---|
| Damage to a chicken by a player | HEAD on `mace::Game<'_>::hurt_by_player` (records target + `source.attacker`) | no damage hook point at the pin |
| Confirm the hit landed | HEAD on `sculk::Game<'_>::mob_damage_event` | RETURN on `hurt_by_player` is refused: its body has a closure (`breach`) |
| `/pettycraft` | cancellable HEAD on `commands::Game<'_>::run_command` and `lan_actions::Game<'_>::guest_command` | `native_command` hook point is the *window's*, before the server; in single player the state lives on the server thread |
| Saving | file I/O from the support module | no persistence API for mods; `native_data_dir` is per-install, not per-world |
| Particles, messages | called from the support module with the APIs above | no helper hook point for them |

Tick and world close use the stable hook points (`tick` v1, `world_lifecycle`
v1) by name.

## 5. Limits found

1. **Pinned revision only.** Raw targets (`hurt_by_player`, `mob_damage_event`,
   `run_command`, `guest_command`) are not hook points, so the mod cannot use
   `"hooks": N` and must be re-checked for every Nightly.
2. **Interaction hooks.** Right-click on an entity (`Game::interact_entity`,
   game.rs:4527) exists but is not needed for this milestone.
3. **Attacker identity is inferred** from the turn (`acting_player_uuid`),
   gated by `Damage.attacker == Some(Who::Player)` recorded for the same mob
   id just before. If the source is not a player (or the pending record is for
   another mob) the hit has *no attacker* and makes no incident.
4. Hits that do not go through `hurt_by_player` (arrows, tridents, fire
   aspect ticks, explosions) are not seen. They go through `Mob::damage*`
   directly with no player identity at that level.
5. **Dedicated servers** cannot run native mods yet (Horizon limitation).
   LAN works through the host.
6. **In-frame mode** (deprecated) fires no lifecycle events: state is then
   saved every 60 s when changed, not on close.
7. **Windows:** the template's `tools/horizon.py` uses Linux file locks.
   Staging here was done with the `horizon-mixins` CLI directly; the launcher's
   own Windows pipeline (Rust 1.99.1 + MinGW) is the supported path.
8. No Horizon source is modified: everything goes through horizon-mixins
   plan/apply on an export.
