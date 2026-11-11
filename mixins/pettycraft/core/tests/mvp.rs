//! The MVP's required scenarios, against the public API only.

use pettycraft_core::scenario::CaseState;
use pettycraft_core::{
    Action, Config, EffectKind, HitEvent, HitOutcome, MobId, Pettycraft, PlayerId, Rejection, Tick,
    Uuid, store,
};

const ALEX: PlayerId = PlayerId(Uuid([0x0a1e, 1, 2, 3]));
const STEVE: PlayerId = PlayerId(Uuid([0x57e5, 4, 5, 6]));
const CLARA: MobId = MobId(Uuid([0xc1a, 7, 8, 9]));
const DORA: MobId = MobId(Uuid([0xd02a, 10, 11, 12]));

fn hit(attacker: Option<PlayerId>, victim: MobId, tick: Tick) -> HitEvent {
    HitEvent {
        attacker,
        attacker_name: Some("Alex".into()),
        victim,
        victim_entity: 42,
        species: "minecraft:chicken".into(),
        victim_name: None,
        tick,
        position: [1.5, 64.0, -3.5],
    }
}

fn recorded(out: &HitOutcome) -> &pettycraft_core::engine::Recorded {
    out.recorded()
        .unwrap_or_else(|| panic!("expected an incident, got {out:?}"))
}

fn tells(actions: &[Action]) -> Vec<(PlayerId, &str)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Tell { player, text, .. } => Some((*player, text.as_str())),
            _ => None,
        })
        .collect()
}

#[test]
fn first_hit_on_a_chicken_records_an_incident_and_schedules_a_reaction() {
    let mut p = Pettycraft::new(Config::default());
    let c = p.config.clone();
    let out = p.on_hit(&hit(Some(ALEX), CLARA, 1000));
    let r = recorded(&out);
    assert_eq!(r.offences, 1);
    assert_eq!(r.case_state, CaseState::Grievance);
    assert_eq!(r.effect, EffectKind::Indignation);
    assert_eq!(r.incident.offender, ALEX);
    assert_eq!(r.incident.victim, CLARA);
    assert_eq!(r.incident.species, "minecraft:chicken");
    assert!(p.take_dirty(), "a new incident needs saving");

    // Nothing fires in the hit's own tick: the effect follows shortly and
    // the message later.
    assert!(!p.has_due(1000));
    let effects = p.due(1000 + c.effect_delay);
    assert!(matches!(
        effects.as_slice(),
        [Action::Effect {
            entity: 42,
            kind: EffectKind::Indignation,
            ..
        }]
    ));
    assert!(p.due(1000 + c.message_delay - 1).is_empty());
    let msgs = p.due(1000 + c.message_delay);
    assert_eq!(tells(&msgs), vec![(ALEX, r.line.as_str())]);
    assert!(p.state().schedule.is_empty());
}

#[test]
fn repeated_hits_escalate_the_same_grudge_and_file_a_case() {
    let mut p = Pettycraft::new(Config::default());
    let gap = p.config.pair_cooldown;
    let first = p.on_hit(&hit(Some(ALEX), CLARA, 0));
    let second = p.on_hit(&hit(Some(ALEX), CLARA, gap));
    let third = p.on_hit(&hit(Some(ALEX), CLARA, 2 * gap));
    let (a, b, c) = (recorded(&first), recorded(&second), recorded(&third));
    assert_eq!((a.offences, b.offences, c.offences), (1, 2, 3));
    assert_eq!(a.case, b.case, "one case per grudge");
    assert_eq!(b.case_state, CaseState::Brooding);
    assert_eq!(b.effect, EffectKind::Outrage);
    assert_eq!(c.case_state, CaseState::Filed);
    assert_eq!(c.effect, EffectKind::CaseFiled);
    let g = p.state().grudges.get(CLARA, ALEX).unwrap();
    assert_eq!(g.offences, 3);
    assert!(g.intensity > 0);
    assert_eq!(p.state().reputation.get(ALEX).offences, 3);
    assert_eq!(p.state().grudges.len(), 1);
}

#[test]
fn different_players_get_separate_grudges_cases_and_messages() {
    let mut p = Pettycraft::new(Config::default());
    let a = p.on_hit(&hit(Some(ALEX), CLARA, 100));
    // The same chicken, the same tick, another player: not a duplicate.
    let s = p.on_hit(&hit(Some(STEVE), CLARA, 100));
    let (a, s) = (recorded(&a), recorded(&s));
    assert_ne!(a.case, s.case);
    assert_eq!((a.offences, s.offences), (1, 1));
    assert!(p.state().grudges.get(CLARA, ALEX).is_some());
    assert!(p.state().grudges.get(CLARA, STEVE).is_some());
    // Another chicken: another grudge for the same player.
    recorded(&p.on_hit(&hit(Some(ALEX), DORA, 101)));
    assert_eq!(p.state().grudges.against(ALEX).count(), 2);

    let delay = p.config.message_delay;
    let msgs = p.due(101 + delay);
    let mut to: Vec<PlayerId> = tells(&msgs).into_iter().map(|(p, _)| p).collect();
    to.sort();
    assert_eq!(to, vec![ALEX, ALEX, STEVE]);
}

#[test]
fn a_hit_with_no_attacker_makes_no_incident() {
    let mut p = Pettycraft::new(Config::default());
    let out = p.on_hit(&hit(None, CLARA, 5));
    assert_eq!(out, HitOutcome::Rejected(Rejection::NoAttacker));
    assert_eq!(p.state().incidents.total(), 0);
    assert!(p.state().grudges.is_empty());
    assert!(p.state().schedule.is_empty());
    assert_eq!(p.state().stats.no_attacker, 1);
    assert!(!p.take_dirty());
}

#[test]
fn a_world_restart_keeps_incidents_grudges_pending_messages_and_limits() {
    let dir = std::env::temp_dir().join(format!("pettycraft-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = store::path_in(&dir);

    let mut p = Pettycraft::new(Config::default());
    recorded(&p.on_hit(&hit(Some(ALEX), CLARA, 500)));
    store::save(&path, p.state()).unwrap();

    // Restart: a new runtime from the file.
    let (state, warning) = store::load_or_quarantine(&path).unwrap();
    assert!(warning.is_none());
    let mut p = Pettycraft::with_state(Config::default(), state);
    assert_eq!(p.state().incidents.total(), 1);
    assert_eq!(p.state().grudges.get(CLARA, ALEX).unwrap().offences, 1);
    // The duplicate report of the same blow is still refused after reload.
    assert_eq!(
        p.on_hit(&hit(Some(ALEX), CLARA, 500)),
        HitOutcome::Rejected(Rejection::Duplicate)
    );
    // The message scheduled before the restart is still delivered.
    let msgs = p.due(500 + p.config.message_delay);
    assert_eq!(tells(&msgs).len(), 1);
    // The next offence continues the count and the same case.
    let gap = p.config.pair_cooldown;
    let next = p.on_hit(&hit(Some(ALEX), CLARA, 500 + gap));
    assert_eq!(recorded(&next).offences, 2);
    assert_eq!(recorded(&next).case_state, CaseState::Brooding);
    // Ids keep counting rather than restarting at 1.
    assert_eq!(recorded(&next).incident.id.0, 2);

    // A corrupt file is moved aside, not fatal.
    std::fs::write(&path, "{ not json").unwrap();
    let (state, warning) = store::load_or_quarantine(&path).unwrap();
    assert_eq!(state, Default::default());
    assert!(warning.is_some());
    assert!(path.with_extension("json.corrupt").exists());
    // A file from a newer format is refused and left in place.
    std::fs::write(&path, r#"{"format": 99, "state": {}}"#).unwrap();
    assert!(matches!(
        store::load(&path),
        Err(store::StoreError::Newer(99))
    ));
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rate_limits_dedupe_same_tick_cool_down_pairs_and_cap_bursts() {
    let c = Config::default();
    let mut p = Pettycraft::new(c.clone());
    recorded(&p.on_hit(&hit(Some(ALEX), CLARA, 0)));
    // The same blow reported twice in one tick.
    assert_eq!(
        p.on_hit(&hit(Some(ALEX), CLARA, 0)),
        HitOutcome::Rejected(Rejection::Duplicate)
    );
    // Spam-clicking the same chicken inside the cooldown.
    assert_eq!(
        p.on_hit(&hit(Some(ALEX), CLARA, c.pair_cooldown - 1)),
        HitOutcome::Rejected(Rejection::RateLimited)
    );
    // A report from before the last one never counts again.
    assert_eq!(
        p.on_hit(&hit(Some(ALEX), CLARA, -100)),
        HitOutcome::Rejected(Rejection::RateLimited)
    );
    recorded(&p.on_hit(&hit(Some(ALEX), CLARA, c.pair_cooldown)));

    // A burst over many chickens: at most player_burst per window.
    let mut q = Pettycraft::new(c.clone());
    let mut accepted = 0;
    for i in 0..(c.player_burst as i32 + 5) {
        let mob = MobId(Uuid([100 + i, 0, 0, 0]));
        if q.on_hit(&hit(Some(ALEX), mob, Tick::from(i)))
            .recorded()
            .is_some()
        {
            accepted += 1;
        }
    }
    assert_eq!(accepted, c.player_burst);
    assert_eq!(q.state().stats.rate_limited, 5);
    // Others are not limited by Alex's burst.
    recorded(&q.on_hit(&hit(Some(STEVE), CLARA, 3)));
    // After the window, Alex may offend again.
    let later = c.player_window + 1;
    recorded(&q.on_hit(&hit(Some(ALEX), MobId(Uuid([999, 0, 0, 0])), later)));
    // Pending reactions stay bounded.
    assert!(q.state().schedule.len() <= c.max_pending);
}

#[test]
fn untracked_mobs_are_ignored() {
    let mut p = Pettycraft::new(Config::default());
    let mut cow = hit(Some(ALEX), CLARA, 0);
    cow.species = "minecraft:cow".into();
    assert_eq!(p.on_hit(&cow), HitOutcome::Rejected(Rejection::Untracked));
    // Horizon may report the kind without its namespace.
    let mut bare = hit(Some(ALEX), CLARA, 0);
    bare.species = "chicken".into();
    assert!(p.on_hit(&bare).recorded().is_some());
}

#[test]
fn undelivered_messages_are_retried_then_dropped() {
    let mut p = Pettycraft::new(Config::default());
    let c = p.config.clone();
    recorded(&p.on_hit(&hit(Some(ALEX), CLARA, 0)));
    let mut now = c.message_delay;
    let _ = p.due(c.effect_delay);
    for _ in 0..c.message_attempts {
        let msgs: Vec<Action> = p
            .due(now)
            .into_iter()
            .filter(|a| matches!(a, Action::Tell { .. }))
            .collect();
        assert_eq!(msgs.len(), 1);
        for m in msgs {
            p.undelivered(m, now);
        }
        now += c.message_retry;
    }
    assert!(p.state().schedule.is_empty());
    assert_eq!(p.state().stats.messages_dropped, 1);
}

#[test]
fn quiet_grievances_are_dismissed_and_reopen_as_new_cases() {
    let mut p = Pettycraft::new(Config::default());
    let c = p.config.clone();
    let first = recorded(&p.on_hit(&hit(Some(ALEX), CLARA, 0))).case;
    assert!(p.maintain(c.case_expiry - 1).is_empty());
    let t = p.maintain(c.case_expiry);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].to, CaseState::Dismissed);
    assert!(p.state().cases.open_case(CLARA, ALEX).is_none());
    let again = p.on_hit(&hit(Some(ALEX), CLARA, c.case_expiry + 1));
    let again = recorded(&again);
    assert_ne!(again.case, first);
    // The grudge itself is not forgotten.
    assert_eq!(again.offences, 2);
}

#[test]
fn the_report_lists_the_players_record() {
    let mut p = Pettycraft::new(Config::default());
    recorded(&p.on_hit(&hit(Some(ALEX), CLARA, 7)));
    let lines = p.report(Some(ALEX)).join("\n");
    assert!(lines.contains("1 incidents"), "{lines}");
    assert!(lines.contains(&CLARA.0.short()), "{lines}");
    assert!(
        !lines.contains(&CLARA.to_string()),
        "full UUIDs wrap in chat: {lines}"
    );
    assert!(lines.contains("grievance"), "{lines}");
    assert!(!p.report(None).join("\n").contains("you ("));
}
