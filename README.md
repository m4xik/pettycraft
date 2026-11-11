# PETTYCRAFT

A comedy native mod for [Horizon](https://github.com/horizon-rust/horizon):
mobs remember what players did to them. Milestone 1: hit a chicken, it
remembers, gets visibly upset, and the offender hears about it a few seconds
later. Repeat offences escalate the grudge until a case is **filed** (the
court scene itself is a later milestone).

- Pinned to Horizon `fdc87ae` with horizon-mixins `5a64af9` (what the
  launcher builds with); first audited on `ba6344a` / `d9c83d7` — see
  [compatibility.json](compatibility.json). All native mods of one build must
  pin the same Horizon commit.
- [docs/AUDIT.md](docs/AUDIT.md): which Horizon APIs exist at the pin, which
  needed a mixin, and the limits.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): the two layers and one hit
  end to end.

## Layout

```text
Cargo.toml                       workspace (the core crate, for standalone tests)
compatibility.json               pinned revisions
mixins/pettycraft/               the native bundle
  horizon-mixin.toml             crate, module and 6 HEAD injections (4 on hook points)
  core/                          pettycraft_core: rules, no Horizon types
    src/{incident,grudge,reputation,director,scenario,schedule,engine,store}.rs
    tests/mvp.rs                 the required scenarios
  runtime.rs                     pettycraft_horizon: crate::game::pettycraft
  hurt.rs damaged.rs tick.rs lifecycle.rs register.rs perform.rs
```

## In game

Hit a chicken (melee, sweep or spear). After a moment angry particles rise
from it; three seconds later the attacker gets a chat line. The state is kept
in `<world>/pettycraft/state.json`.

`/pettycraft` (or `/pettycraft status`) shows the totals and your record;
`/pettycraft pending` lists queued reactions; `/pettycraft save` saves now.

## Checks

Standalone (any OS; the template's toolchain is 1.95.0):

```bash
cargo +1.95.0 fmt --all --check
```

```bash
cargo +1.95.0 clippy --workspace --all-targets --locked -- -D warnings
```

```bash
cargo +1.95.0 test --workspace --locked
```

Native (stage the bundle onto the pinned source; `apply` needs a fresh `--out` on Windows, and check the real game
binary). `HORIZON_SRC` is a pristine export of `fdc87ae` containing a
`.horizon-revision` file, for example the launcher's
`native/source/fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`; on Windows put the
launcher's Rust and MinGW (`<launcher data>/rust/1.99.1-x86_64-pc-windows-gnu/bin`
and `…/mingw64/bin`) first on `PATH`. Do not export with `git archive` under
`core.autocrlf=true`: CRLF line endings break horizon-mixins' `Cargo.lock` edit.

```bash
horizon-mixins plan --workspace "$HORIZON_SRC" --mods mixins --out target/mixins/plan.json
```

```bash
horizon-mixins apply --workspace "$HORIZON_SRC" --mods mixins --out target/mixins/modded
```

```bash
cargo check --locked -p horizon-client --bin horizon-client --no-default-features --manifest-path target/mixins/modded/Cargo.toml
```

```bash
cargo test --locked -p horizon-client --lib --manifest-path target/mixins/modded/Cargo.toml -- pettycraft
```

To play it, install the bundle folder `mixins/pettycraft` as a native mod in
the Horizon launcher (Mods → native mod → Build, or
`horizon-launcher native-build mixins/pettycraft --instance NAME --trust-native-code`).
Building native code is the player's decision: read the source first.

License: MIT.
