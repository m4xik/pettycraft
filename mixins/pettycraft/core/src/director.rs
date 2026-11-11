//! Comedy Director: picks the reaction to an incident. Pure and
//! deterministic: the same incident always gets the same line, so tests and
//! replays agree, and nothing needs a random source.

use crate::config::Config;
use crate::reputation::Standing;
use crate::scenario::CaseState;
use crate::schedule::EffectKind;

/// What the director knows about an incident.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene<'a> {
    /// The incident's number, to vary lines.
    pub incident: u64,
    /// Offences of this player against this mob, this one included.
    pub offences: u32,
    pub case: CaseState,
    /// The case moved into `case` with this incident.
    pub case_changed: bool,
    pub standing: Standing,
    pub victim_name: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reaction {
    pub effect: EffectKind,
    pub effect_delay: i64,
    pub line: String,
    pub line_delay: i64,
}

const FIRST: &[&str] = &[
    "{victim} смотрит на вас очень пристально. Она это запомнит.",
    "{victim} громко кудахчет и, кажется, записывает ваше имя.",
    "{victim} обиженно отворачивается. Инцидент зафиксирован.",
];

const REPEAT: &[&str] = &[
    "{victim}: «Опять?! Это уже {n}-й раз». Свидетели всё видели.",
    "{victim} собирает доказательства. Эпизод №{n}.",
    "{victim} больше не кудахчет. Она молча копит обиду ({n}).",
];

const FILED: &[&str] = &[
    "{victim} подала иск. Дело передано в Курятный суд. Ожидайте повестку.",
    "Терпение {victim_gen} лопнуло: дело официально заведено.",
];

/// A wanted player is told so, after the line.
const WANTED: &str = " Ваша репутация среди мобов: «в розыске».";

fn pick(lines: &[&'static str], seed: u64) -> &'static str {
    lines[(seed % lines.len() as u64) as usize]
}

pub fn direct(scene: &Scene<'_>, config: &Config) -> Reaction {
    let victim = scene.victim_name.unwrap_or("Курица");
    let victim_gen = scene.victim_name.unwrap_or("курицы");
    let (effect, template) = if scene.case == CaseState::Filed && scene.case_changed {
        (EffectKind::CaseFiled, pick(FILED, scene.incident))
    } else if scene.offences <= 1 {
        (EffectKind::Indignation, pick(FIRST, scene.incident))
    } else {
        (EffectKind::Outrage, pick(REPEAT, scene.incident))
    };
    let mut line = template
        .replace("{victim_gen}", victim_gen)
        .replace("{victim}", victim)
        .replace("{n}", &scene.offences.to_string());
    if scene.standing == Standing::Wanted {
        line.push_str(WANTED);
    }
    Reaction {
        effect,
        effect_delay: config.effect_delay,
        line,
        line_delay: config.message_delay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(offences: u32, case: CaseState, changed: bool) -> Scene<'static> {
        Scene {
            incident: 1,
            offences,
            case,
            case_changed: changed,
            standing: Standing::Neutral,
            victim_name: None,
        }
    }

    #[test]
    fn first_repeat_and_filed_get_their_own_reactions() {
        let c = Config::default();
        let first = direct(&scene(1, CaseState::Grievance, false), &c);
        assert_eq!(first.effect, EffectKind::Indignation);
        assert!(first.line.starts_with("Курица"));
        let again = direct(&scene(2, CaseState::Brooding, true), &c);
        assert_eq!(again.effect, EffectKind::Outrage);
        assert!(again.line.contains('2'));
        let filed = direct(&scene(3, CaseState::Filed, true), &c);
        assert_eq!(filed.effect, EffectKind::CaseFiled);
    }

    #[test]
    fn lines_are_deterministic_and_use_the_name() {
        let c = Config::default();
        let mut s = scene(1, CaseState::Grievance, false);
        s.victim_name = Some("Клара");
        assert_eq!(direct(&s, &c), direct(&s, &c));
        assert!(direct(&s, &c).line.contains("Клара"));
        assert!(!direct(&s, &c).line.contains('{'));
    }
}
