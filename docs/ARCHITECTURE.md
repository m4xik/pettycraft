# PETTYCRAFT architecture

Two layers. The rules know nothing about Horizon; the adapter knows nothing
about comedy.

```text
 Horizon (pinned fdc87ae, rewritten by horizon-mixins, never edited by hand)
 ─────────────────────────────────────────────────────────────────────────
  Game::hurt_by_player ──HEAD──┐          Game::native_tick ──HEAD──┐
  Game::mob_damage_event ─HEAD─┤          native_world_lifecycle ─HEAD─┤
  Game::run_command / guest_command ─HEAD (cancellable)─┐             │
                               ▼                        ▼             ▼
 pettycraft_horizon  (support module crate::game::pettycraft, runtime.rs)
 ─────────────────────────────────────────────────────────────────────────
  event adapter      before_hurt + damaged → HitEvent (UUIDs, species, tick)
  server handling    authoritative() gate, per-world state lease (thread-local)
  persistence        <world>/pettycraft/state.json, every 60 s if changed + on close
  visual effects     angry_villager particles via sendParticles path
  messages           for_player(Guest(conn)) → system_message
  developer command  /pettycraft [status|pending|save]
                               │ HitEvent / Tick            ▲ Action
                               ▼                            │
 pettycraft_core  (bundle crate pettycraft-core, pure Rust + serde)
 ─────────────────────────────────────────────────────────────────────────
  Incident Engine    validate → de-duplicate → rate-limit → Incident
  Grudge Engine      (mob, player) → Grudge { offences, intensity }
  Reputation System  player → score, standing
  Scenario Machine   (mob, player) → ActiveCase: Grievance → Brooding → Filed | Dismissed
  Comedy Director    Scene → Reaction { effect, line, delays }  (deterministic)
  Schedule           due-ordered Actions, O(1) "anything due?"
  Store              versioned JSON, atomic write, corrupt-file quarantine
```

## Data structures (separate, as required)

| Struct | Module | Key |
|---|---|---|
| `Incident` — one accepted offence | `incident` | `IncidentId` |
| `Grudge` — what one mob holds against one player | `grudge` | `(MobId, PlayerId)` |
| `ActiveCase` — the case built from a grudge | `scenario` | `CaseId`, one open per `(MobId, PlayerId)` |
| `Reputation` — a player's standing | `reputation` | `PlayerId` |
| `Action` — a delayed effect or message | `schedule` | `(due tick, seq)` |

Mobs are keyed by their **persistent UUID** (`Game::mob_uuid`), not their
entity id, so grudges survive a reload. Ticks are `world.game_time`, which the
world saves, so delays and limits stay valid across restarts.

## One hit, end to end

1. `hurt_by_player(index, amount, source)` HEAD: remember `(mob id,
   source.attacker == Player)` in a thread-local.
2. `mob_damage_event(id)` HEAD (only after health changed): if the remembered
   hit is for this mob and by a player, attacker = `acting_player_uuid()`;
   else attacker = none. Untracked kinds stop here.
3. `Pettycraft::on_hit`: Incident Engine accepts or rejects (`NoAttacker`,
   `Untracked`, `Duplicate` — same player, mob and tick — or `RateLimited` —
   within 2 s of the last incident on that mob, or over 8 per player per
   minute). Accepted: grudge +1, reputation −severity, case opened/advanced,
   director picks a reaction, two actions are scheduled (effect +4 ticks,
   message +3 s).
4. `native_tick` HEAD: one lookup when nothing is due. Due actions are
   performed; an undelivered message (player offline) is retried 3 times,
   5 s apart. Every 100 ticks: upkeep (prune limits, dismiss quiet cases),
   and a save if changed and the last save is 60 s old.
5. `WorldCloseFinal` / `WorldCloseFailed`: save and drop the state.

## Why these choices

- **Server only.** Every entry point checks `remote.is_none()`: a client
  connected to a server (the window in single player too, since the world
  runs on the integrated server thread) never runs rules or touches files.
- **Multiplayer-ready.** Players are UUIDs; per-pair and per-player limits;
  messages go only to the offender, through the LAN hub, so LAN guests work
  as the host does. Dedicated servers wait on Horizon's native-mod build.
- **Vanilla untouched.** Every hook is a non-cancelling HEAD except the two
  command hooks, which only cancel `/pettycraft`. Damage, drops, AI and
  blocks are unchanged; nothing is removed from the world.
- **Cheap ticks.** Nothing heavy per tick; bounded collections (256
  incidents, 64 pending actions, 32 incident ids per grudge).
- **No AI services, no network.** Lines come from fixed tables, chosen by
  incident number.
