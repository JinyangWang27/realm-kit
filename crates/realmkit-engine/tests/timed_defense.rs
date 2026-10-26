//! Weapon handling and timed defence in real-time fights: light and heavy
//! attacks, parries, dodges, poise and interruption, how a reported contact
//! meets each of them, and saves and replays, at the fencing hall.

use realmkit_engine::{Command::*, Intent as I, *};
use realmkit_spec::*;
use serde_json::json;

const HALL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/fencing-hall");

fn hall() -> WorldSpec {
    WorldSpec::load(HALL).unwrap()
}

/// The walkthrough's fight commands, in the CLI's words.
fn script(text: &str) -> Vec<Command> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !["look", "status"].contains(l))
        .map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            let id = |i: usize| words.get(i).map(|w| w.to_string());
            Intent(match words[0] {
                "duel" => return Duel(words[1].into()),
                "step" => I::Advance(words.get(1).map_or(1, |n| n.parse().unwrap())),
                "strike" => match id(1) {
                    None => I::Attack { target: None },
                    Some(skill) => I::Skill {
                        skill,
                        target: None,
                    },
                },
                "parry" => I::Parry,
                "dodge" => I::Dodge,
                "guard" => I::Guard,
                "disengage" => I::Retreat,
                "contact" => I::Contact {
                    attacker: words[1].into(),
                    target: id(2),
                },
                _ => panic!("unknown step {line}"),
            })
        })
        .collect()
}

fn step(n: u32) -> Command {
    Intent(I::Advance(n))
}

fn skill(id: &str) -> Command {
    Intent(I::Skill {
        skill: id.into(),
        target: None,
    })
}

fn contact(by: &str) -> Command {
    Intent(I::Contact {
        attacker: by.into(),
        target: None,
    })
}

fn run(engine: &mut Engine<'_>, commands: &[Command]) -> Vec<Event> {
    commands
        .iter()
        .flat_map(|c| {
            engine
                .execute(c.clone())
                .unwrap_or_else(|e| panic!("{c:?}: {e}"))
        })
        .collect()
}

fn fighting<'w>(world: &'w WorldSpec, commands: &[Command]) -> Engine<'w> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    run(&mut engine, commands);
    engine
}

fn action(engine: &Engine<'_>, id: &str) -> RealtimeAction {
    let fight = engine.realtime().unwrap();
    fight.fighters[fight.index(id).unwrap()].action.clone()
}

fn hp(engine: &Engine<'_>, id: &str) -> u32 {
    let fight = engine.realtime().unwrap();
    fight.vitals(fight.index(id).unwrap(), engine.state()).hp
}

fn staggered(id: &str) -> Event {
    Event::Staggered {
        character: id.into(),
    }
}

/// A hit's damage between two characters of the hall, from the shared formula.
fn hit(world: &WorldSpec, engine: &Engine<'_>, on: &str, power: u32) -> u32 {
    let stats = world.character(on).unwrap().combat.as_ref().unwrap().stats;
    let player = engine.player_stats().unwrap();
    damage_modified(&player, &stats, Channel::Physical, power, 25, 100, (1, 1)).unwrap()
}

fn dealt(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { amount, .. } | Event::DamageReceived { amount, .. } => {
                Some(*amount)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn light_and_heavy_attacks_trade_time_for_power() {
    let world = hall();
    // The jab runs at 60% of a basic attack's time, the cleave at 160%:
    // wind-up 2, active 2, recovery 3 against 5, 4 and 7.
    let mut light = fighting(&world, &[Duel("bruiser".into()), skill("jab")]);
    let mut heavy = fighting(&world, &[Duel("bruiser".into()), skill("cleave")]);
    for (engine, phases) in [(&mut light, [2, 4, 7]), (&mut heavy, [5, 9, 16])] {
        for (until, phase) in phases.into_iter().zip([
            AttackPhase::Windup,
            AttackPhase::Active,
            AttackPhase::Recovery,
        ]) {
            assert!(matches!(
                action(engine, "fencer"),
                RealtimeAction::Attack { phase: p, until: u, .. } if p == phase && u == until
            ));
            let now = engine.realtime().unwrap().step;
            if phase == AttackPhase::Active {
                let events = run(engine, &[contact("fencer")]);
                let power = world.skill(&skill_of(engine)).unwrap().power;
                assert_eq!(dealt(&events), [hit(&world, engine, "bruiser", power)]);
            }
            run(engine, &[step((until - now) as u32)]);
        }
        assert_eq!(action(engine, "fencer"), RealtimeAction::Ready);
    }
    let fresh = Engine::new(&world).unwrap();
    assert!(hit(&world, &fresh, "bruiser", 70) < hit(&world, &fresh, "bruiser", 220));
}

fn skill_of(engine: &Engine<'_>) -> Id {
    match engine.realtime().unwrap().fighters[0].action.clone() {
        RealtimeAction::Attack { skill, .. } => skill.unwrap(),
        _ => panic!("attacking"),
    }
}

#[test]
fn heavy_poise_stands_through_light_impact_and_simultaneous_blows_resolve_in_order() {
    let world = hall();
    // The bruiser's maul winds up from step 1 to 6 with poise 1; a jab of
    // impact 1 lands on it without interrupting it.
    let mut engine = fighting(
        &world,
        &[Duel("bruiser".into()), step(1), skill("jab"), step(2)],
    );
    let events = run(&mut engine, &[contact("fencer")]);
    assert!(!events.contains(&staggered("bruiser")));
    assert!(matches!(
        action(&engine, "bruiser"),
        RealtimeAction::Attack {
            phase: AttackPhase::Windup,
            ..
        }
    ));
    // A cleave begun with the maul strikes at the same step. Whichever blow
    // the host reports first lands and interrupts the other, whose report
    // then finds no attack: both have poise 1 and impact 2.
    let both = [Duel("bruiser".into()), step(1), skill("cleave"), step(5)];
    let mut player_first = fighting(&world, &both);
    let events = run(&mut player_first, &[contact("fencer")]);
    assert!(events.contains(&staggered("bruiser")));
    assert_eq!(
        action(&player_first, "bruiser"),
        RealtimeAction::Staggered { until: 11 }
    );
    assert_eq!(
        player_first
            .execute(contact("bruiser"))
            .unwrap_err()
            .to_string(),
        EngineError::NoContact("bruiser".into()).to_string()
    );
    let mut bruiser_first = fighting(&world, &both);
    let events = run(&mut bruiser_first, &[contact("bruiser")]);
    assert_eq!(events.last(), Some(&staggered("fencer")));
    assert_eq!(
        bruiser_first
            .execute(contact("fencer"))
            .unwrap_err()
            .to_string(),
        EngineError::NoContact("fencer".into()).to_string()
    );
}

#[test]
fn a_parry_in_its_window_deflects_the_blow_and_opens_a_counter() {
    let world = hall();
    // The duelist's thrust is active at steps 4 and 5; a parry opens for one step.
    let mut engine = fighting(&world, &[Duel("duelist".into()), step(4), Intent(I::Parry)]);
    assert_eq!(
        action(&engine, "fencer"),
        RealtimeAction::Parry {
            open_until: 5,
            until: 9
        }
    );
    let events = run(&mut engine, &[contact("duelist")]);
    assert_eq!(
        events,
        [
            Event::Parried {
                character: "fencer".into(),
                attacker: "duelist".into()
            },
            staggered("duelist"),
        ]
    );
    assert_eq!(hp(&engine, "fencer"), 60);
    // The attacker is staggered for 5 steps and the parrier skips its recovery.
    assert_eq!(
        action(&engine, "duelist"),
        RealtimeAction::Staggered { until: 9 }
    );
    assert_eq!(
        action(&engine, "fencer"),
        RealtimeAction::Parry {
            open_until: 5,
            until: 5
        }
    );
    assert_eq!(
        engine.execute(skill("jab")).unwrap_err().to_string(),
        EngineError::Committed.to_string(),
        "the window is still open"
    );
    // The counter: a jab from step 5 lands at 7 while the duelist is helpless,
    // and a staggered fighter is not staggered again.
    let events = run(
        &mut engine,
        &[step(1), skill("jab"), step(2), contact("fencer")],
    );
    assert!(!events.contains(&staggered("duelist")));
    assert_eq!(
        hp(&engine, "duelist"),
        40 - hit(&world, &engine, "duelist", 70)
    );
    assert_eq!(
        action(&engine, "duelist"),
        RealtimeAction::Staggered { until: 9 }
    );
}

#[test]
fn a_mistimed_parry_takes_the_full_hit_and_commits_its_recovery() {
    let world = hall();
    let unparried = fighting(
        &world,
        &[Duel("duelist".into()), step(4), contact("duelist")],
    );
    // A parry one step early has closed when the thrust lands.
    let mut engine = fighting(
        &world,
        &[Duel("duelist".into()), step(3), Intent(I::Parry), step(1)],
    );
    let events = run(&mut engine, &[contact("duelist")]);
    assert!(!events.iter().any(|e| matches!(e, Event::Parried { .. })));
    assert_eq!(hp(&engine, "fencer"), hp(&unparried, "fencer"));
    // Exposed until its recovery ends, it can neither attack nor defend; a
    // fighter already recovering has nothing left to interrupt.
    assert_eq!(
        action(&engine, "fencer"),
        RealtimeAction::Parry {
            open_until: 4,
            until: 8
        }
    );
    let before = engine.snapshot().to_json();
    for intent in [
        I::Attack { target: None },
        I::Guard,
        I::Parry,
        I::Dodge,
        I::Retreat,
    ] {
        assert_eq!(
            engine.execute(Intent(intent)).unwrap_err().to_string(),
            EngineError::Committed.to_string()
        );
    }
    assert_eq!(engine.snapshot().to_json(), before);
    let parry = Command::Intent(I::Parry);
    assert!(engine
        .actions()
        .iter()
        .any(|a| a.command == parry && !a.available));
    run(&mut engine, &[step(4)]);
    assert_eq!(action(&engine, "fencer"), RealtimeAction::Ready);
}

#[test]
fn a_dodge_evades_only_inside_its_window_and_leaves_the_attack_live() {
    let world = hall();
    let mut engine = fighting(
        &world,
        &[Duel("duelist".into()), step(3), Intent(I::Dodge), step(1)],
    );
    let events = run(&mut engine, &[contact("duelist")]);
    assert_eq!(
        events,
        [Event::Evaded {
            character: "fencer".into(),
            attacker: "duelist".into()
        }]
    );
    assert_eq!(hp(&engine, "fencer"), 60);
    assert!(matches!(
        action(&engine, "duelist"),
        RealtimeAction::Attack {
            phase: AttackPhase::Active,
            connected: false,
            ..
        }
    ));
    // The window closes at step 5 while the thrust is still active: it lands,
    // and the dodge cannot be renewed before its recovery ends at 8.
    let events = run(&mut engine, &[step(1), contact("duelist")]);
    assert_eq!(dealt(&events).len(), 1);
    assert!(!events.contains(&staggered("fencer")));
    assert_eq!(
        engine.execute(Intent(I::Dodge)).unwrap_err().to_string(),
        EngineError::Committed.to_string()
    );
}

#[test]
fn impact_above_poise_interrupts_guards_retreats_and_idle_fighters() {
    let world = hall();
    // A raised guard (poise 1) stands through a thrust (impact 1)...
    let mut engine = fighting(&world, &[Duel("duelist".into()), Intent(I::Guard), step(4)]);
    let events = run(&mut engine, &[contact("duelist")]);
    assert!(matches!(events[0], Event::Blocked { .. }));
    assert_eq!(
        action(&engine, "fencer"),
        RealtimeAction::Guard { ready_at: 1 }
    );
    // ...but a maul (impact 2) breaks it, after the guard has taken its share.
    let mut engine = fighting(&world, &[Duel("bruiser".into()), Intent(I::Guard), step(6)]);
    let events = run(&mut engine, &[contact("bruiser")]);
    assert!(matches!(events[0], Event::Blocked { .. }));
    assert_eq!(events.last(), Some(&staggered("fencer")));
    assert_eq!(
        action(&engine, "fencer"),
        RealtimeAction::Staggered { until: 11 }
    );
    assert_eq!(
        engine
            .execute(Intent(I::LowerGuard))
            .unwrap_err()
            .to_string(),
        EngineError::NotGuarding.to_string()
    );
    // A guard still rising, a retreat and an idle fighter have no poise.
    let rising = [Duel("duelist".into()), step(4), Intent(I::Guard)];
    let retreating = [Duel("duelist".into()), Intent(I::Retreat), step(4)];
    for setup in [&rising[..], &retreating[..]] {
        let mut engine = fighting(&world, setup);
        let events = run(&mut engine, &[contact("duelist")]);
        assert_eq!(events.last(), Some(&staggered("fencer")), "{setup:?}");
        // An interrupted retreat never completes.
        run(&mut engine, &[step(10)]);
        assert!(engine.realtime().is_some());
    }
    let mut engine = fighting(&world, &[Duel("duelist".into()), step(4)]);
    let events = run(&mut engine, &[contact("duelist")]);
    assert_eq!(events.last(), Some(&staggered("fencer")));
    // Basic attacks have no impact, and a recovering attacker is not interrupted.
    let mut engine = fighting(
        &world,
        &[
            Duel("duelist".into()),
            Intent(I::Attack { target: None }),
            step(5),
        ],
    );
    let events = run(&mut engine, &[contact("duelist")]);
    assert!(!events.contains(&staggered("fencer")));
    assert!(matches!(
        action(&engine, "fencer"),
        RealtimeAction::Attack {
            phase: AttackPhase::Recovery,
            ..
        }
    ));
}

#[test]
fn engine_controlled_fighters_parry_by_their_reaction() {
    let world = hall();
    // The swordmaster guards; when the player's attack is active and has
    // not landed, she parries, with the same window and recovery.
    let opening = [
        Duel("swordmaster".into()),
        step(1),
        Intent(I::Attack { target: None }),
        step(3),
    ];
    let mut engine = fighting(&world, &opening);
    assert_eq!(
        action(&engine, "swordmaster"),
        RealtimeAction::Parry {
            open_until: 5,
            until: 9
        }
    );
    let events = run(&mut engine, &[contact("fencer")]);
    assert_eq!(events.last(), Some(&staggered("fencer")));
    assert_eq!(hp(&engine, "swordmaster"), 40);
    // A blow that lands late in its active window beats the parry.
    let mut engine = fighting(&world, &opening);
    let events = run(&mut engine, &[step(1), contact("fencer")]);
    assert_eq!(dealt(&events).len(), 1);
    // Without the reaction she only blocks.
    let mut plain = hall();
    let profile = plain.characters.iter_mut().find(|c| c.id == "swordmaster");
    profile.unwrap().combat.as_mut().unwrap().reaction = None;
    let mut engine = fighting(&plain, &opening);
    let events = run(&mut engine, &[contact("fencer")]);
    assert!(matches!(events[0], Event::Blocked { .. }));
}

fn ford(oake: serde_json::Value) -> WorldSpec {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/hollow-ford"
    ))
    .unwrap();
    let combat = world.world.combat.as_mut().unwrap();
    let realtime = combat.realtime.as_mut().unwrap();
    realtime.stagger = 4;
    realtime.parry = Some(TimedDefense {
        window: 1,
        recovery: 3,
    });
    // The lookout's hook breaks guards; it aims at the weakest, Brother Oake.
    combat.skills.push(
        serde_json::from_value(json!({
            "id": "hook", "name": "Hook", "power": 100, "channel": "physical",
            "impact": 2, "text": "{attacker} hooks {target}: {damage} damage."
        }))
        .unwrap(),
    );
    for c in &mut world.characters {
        let profile = c.combat.as_mut();
        match c.id.as_str() {
            "ford_lookout" => profile.unwrap().skills.push("hook".into()),
            "oake" => {
                let profile = profile.unwrap();
                profile.stats.hp = 12;
                profile.reaction = serde_json::from_value(oake.clone()).unwrap();
            }
            _ => {}
        }
    }
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    world
}

/// Brother Oake joins the party and the fight at the ford, and the clock runs
/// until the lookout's hook at him is active.
fn hooked(world: &WorldSpec) -> Engine<'_> {
    let mut engine = fighting(
        world,
        &[
            Talk("mara".into()),
            ChooseDialogue(1),
            ChooseDialogue(1),
            Travel("crossroads".into()),
            Talk("oake".into()),
            ChooseDialogue(1),
            Travel("ford".into()),
            Duel("ford_captain".into()),
            Intent(I::Join {
                character: "oake".into(),
                team: 0,
            }),
        ],
    );
    for _ in 0..50 {
        if let RealtimeAction::Attack {
            target,
            phase: AttackPhase::Active,
            ..
        } = action(&engine, "ford_lookout")
        {
            assert_eq!(target, "oake");
            return engine;
        }
        run(&mut engine, &[step(1)]);
    }
    panic!("the hook never came");
}

#[test]
fn named_companions_parry_and_are_staggered_by_the_same_rules() {
    // Guarding under his standing order, Oake parries the hook, staggering the lookout.
    let world = ford(json!("parry"));
    let mut engine = hooked(&world);
    assert!(matches!(
        action(&engine, "oake"),
        RealtimeAction::Parry { .. }
    ));
    let events = run(&mut engine, &[contact("ford_lookout")]);
    assert_eq!(events.last(), Some(&staggered("ford_lookout")));
    let save = engine.snapshot().to_json();
    Engine::restore(&world, SaveSnapshot::from_json(&save).unwrap()).unwrap();
    // Without a reaction he has attacked by then, and the hook interrupts
    // his wind-up; his own vitals carry the hit.
    let world = ford(json!(null));
    let mut engine = hooked(&world);
    assert!(matches!(
        action(&engine, "oake"),
        RealtimeAction::Attack {
            phase: AttackPhase::Windup,
            ..
        }
    ));
    let events = run(&mut engine, &[contact("ford_lookout")]);
    assert_eq!(events.last(), Some(&staggered("oake")));
    assert!(engine.companion("oake").unwrap().vitals.hp < 12);
    let save = engine.snapshot().to_json();
    Engine::restore(&world, SaveSnapshot::from_json(&save).unwrap()).unwrap();
}

#[test]
fn the_fixture_replays_identically_and_resumes_from_any_save() {
    let world = hall();
    let text = std::fs::read_to_string(format!("{HALL}/walkthrough.txt")).unwrap();
    let script = script(&text);
    let mut whole = Engine::new_with_seed(&world, 7).unwrap();
    let events = run(&mut whole, &script);
    // It shows every rule: both parries, the dodge, interruptions both ways
    // and a guard broken, then settles each fight once.
    for (event, times) in [
        (staggered("duelist"), 2),
        (staggered("bruiser"), 1),
        (staggered("fencer"), 2),
        (
            Event::Evaded {
                character: "fencer".into(),
                attacker: "duelist".into(),
            },
            1,
        ),
        (
            Event::Parried {
                character: "swordmaster".into(),
                attacker: "fencer".into(),
            },
            1,
        ),
    ] {
        let seen = events.iter().filter(|e| **e == event).count();
        assert_eq!(seen, times, "{event:?}");
    }
    let ended: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::EncounterEnded { outcome } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(ended, [Outcome::Victory, Outcome::Fled, Outcome::Fled]);
    for cut in 1..script.len() {
        let mut first = Engine::new_with_seed(&world, 7).unwrap();
        let mut seen = run(&mut first, &script[..cut]);
        let bytes = first.snapshot().to_json();
        let mut resumed =
            Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
        seen.extend(run(&mut resumed, &script[cut..]));
        assert_eq!(seen, events, "cut at {cut}");
        assert_eq!(resumed.snapshot().to_json(), whole.snapshot().to_json());
    }
}

#[test]
fn saves_with_defence_states_the_rules_cannot_reach_are_refused() {
    let world = hall();
    // At step 4 against the swordmaster, parrying the player's blow.
    let engine = fighting(
        &world,
        &[
            Duel("swordmaster".into()),
            step(1),
            Intent(I::Attack { target: None }),
            step(3),
        ],
    );
    let tamper = |world: &WorldSpec, i: usize, action: RealtimeAction| {
        let mut snapshot = engine.snapshot();
        let combat = snapshot.state.combat.as_mut().unwrap();
        let Stance::Realtime(fight) = &mut combat.stance else {
            panic!("fighting");
        };
        fight.fighters[i].action = action;
        Engine::restore(world, snapshot).is_ok()
    };
    use RealtimeAction::*;
    let parry = |open_until, until| Parry { open_until, until };
    let dodge = |open_until, until| Dodge { open_until, until };
    assert!(tamper(&world, 1, parry(5, 9)));
    assert!(tamper(&world, 1, parry(5, 5)), "a clean parry");
    assert!(tamper(&world, 1, parry(4, 8)), "recovering");
    assert!(!tamper(&world, 1, parry(4, 4)), "ended");
    assert!(!tamper(&world, 1, parry(6, 10)), "begun after now");
    assert!(!tamper(&world, 1, parry(5, 8)), "a short recovery");
    assert!(!tamper(&world, 1, dodge(6, 9)), "she reacts with a parry");
    assert!(tamper(&world, 0, dodge(6, 9)));
    assert!(tamper(&world, 0, dodge(5, 8)));
    assert!(
        !tamper(&world, 0, dodge(6, 6)),
        "only a parry skips recovery"
    );
    assert!(!tamper(&world, 0, dodge(7, 10)), "begun after now");
    assert!(tamper(&world, 0, Staggered { until: 9 }));
    assert!(tamper(&world, 1, Staggered { until: 5 }));
    assert!(!tamper(&world, 0, Staggered { until: 10 }), "too long");
    assert!(!tamper(&world, 0, Staggered { until: 4 }), "ended");
    // Nobody dodges where the world has no dodge, nor staggers without a stagger.
    let mut bare = hall();
    let realtime = bare
        .world
        .combat
        .as_mut()
        .unwrap()
        .realtime
        .as_mut()
        .unwrap();
    realtime.dodge = None;
    assert!(!tamper(&bare, 0, dodge(6, 9)));
    let mut calm = hall();
    let combat = calm.world.combat.as_mut().unwrap();
    combat.realtime.as_mut().unwrap().stagger = 0;
    combat.realtime.as_mut().unwrap().parry = None;
    for skill in &mut combat.skills {
        skill.impact = 0;
    }
    for c in &mut calm.characters {
        if let Some(p) = c.combat.as_mut() {
            p.reaction = None;
        }
    }
    assert!(calm.diagnostics().is_empty(), "{:?}", calm.diagnostics());
    assert!(!tamper(&calm, 0, Staggered { until: 5 }));
}

#[test]
fn worlds_without_timed_defence_refuse_it_and_offer_none() {
    let world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/practice-yard"
    ))
    .unwrap();
    let mut engine = fighting(&world, &[Duel("master".into())]);
    let before = engine.snapshot().to_json();
    assert_eq!(
        engine.execute(Intent(I::Parry)).unwrap_err().to_string(),
        EngineError::NoParry.to_string()
    );
    assert_eq!(
        engine.execute(Intent(I::Dodge)).unwrap_err().to_string(),
        EngineError::NoDodge.to_string()
    );
    assert_eq!(engine.snapshot().to_json(), before);
    assert!(!engine
        .actions()
        .iter()
        .any(|a| matches!(a.command, Intent(I::Parry | I::Dodge))));
    // The yard's packages serialize no new field, so their revision stands.
    let json = serde_json::to_string(&world.world).unwrap();
    for field in [
        "stagger",
        "guard_poise",
        "parry",
        "dodge",
        "impact",
        "poise",
        "reaction",
    ] {
        assert!(!json.contains(&format!("\"{field}\"")), "{field}");
    }
}

#[test]
fn a_newcomer_staggered_at_the_step_it_joined_saves_and_resumes() {
    let world = hall();
    let script = [
        Duel("duelist".into()),
        step(4),
        Intent(I::Join {
            character: "bruiser".into(),
            team: 2,
        }),
        Intent(I::Contact {
            attacker: "duelist".into(),
            target: Some("bruiser".into()),
        }),
    ];
    let mut engine = fighting(&world, &script);
    assert_eq!(
        action(&engine, "bruiser"),
        RealtimeAction::Staggered { until: 9 }
    );
    let bytes = engine.snapshot().to_json();
    let mut resumed = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    assert_eq!(run(&mut resumed, &[step(6)]), run(&mut engine, &[step(6)]));
    // It still has regenerated nothing at the step it joined.
    let mut snapshot = SaveSnapshot::from_json(&bytes).unwrap();
    let Stance::Realtime(fight) = &mut snapshot.state.combat.as_mut().unwrap().stance else {
        panic!("fighting");
    };
    fight.fighters[2].mp_remainder = 1;
    assert!(Engine::restore(&world, snapshot).is_err());
}
