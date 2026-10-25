//! Real-time fights with more than two fighters: groups, reinforcements on
//! either side, allies, targets, teams, the end of a fight, settlement and
//! mid-fight saves, in the practice yard.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;
use serde_json::json;

fn yard() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/practice-yard"
    ))
    .unwrap()
}

/// The yard with a warden who fights beside the player, and whatever else
/// `conduct` adds to his profile.
fn yard_with_warden(conduct: serde_json::Value) -> WorldSpec {
    let mut world = yard();
    let mut combat = json!({
        "stats": { "hp": 30, "patk": 8, "pdef": 4, "satk": 0, "sdef": 3, "speed": 100 },
        "xp": 50,
        "ally": true
    });
    combat
        .as_object_mut()
        .unwrap()
        .extend(conduct.as_object().unwrap().clone());
    let warden = serde_json::from_value(json!({
        "id": "warden",
        "name": "The Warden",
        "description": "He keeps the yard, and the peace in it.",
        "combat": combat
    }))
    .unwrap();
    world.characters.push(warden);
    world.locations[0].characters.push("warden".into());
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    world
}

/// A player who can outlast three fighters at once.
fn hardy(mut world: WorldSpec) -> WorldSpec {
    for level in &mut world.world.combat.as_mut().unwrap().levels {
        level.stats.hp = 400;
    }
    world
}

fn step(n: u32) -> Command {
    Intent(realmkit_engine::Intent::Advance(n))
}

fn strike() -> Command {
    Intent(realmkit_engine::Intent::Attack { target: None })
}

fn strike_at(target: &str) -> Command {
    Intent(realmkit_engine::Intent::Attack {
        target: Some(target.into()),
    })
}

fn contact(by: &str) -> Command {
    Intent(realmkit_engine::Intent::Contact {
        attacker: by.into(),
        target: None,
    })
}

fn contact_on(by: &str, on: &str) -> Command {
    Intent(realmkit_engine::Intent::Contact {
        attacker: by.into(),
        target: Some(on.into()),
    })
}

fn join(character: &str, team: u32) -> Command {
    Intent(realmkit_engine::Intent::Join {
        character: character.into(),
        team,
    })
}

fn run(engine: &mut Engine<'_>, commands: &[Command]) -> Vec<Event> {
    commands
        .iter()
        .flat_map(|c| engine.execute(c.clone()).unwrap())
        .collect()
}

fn fight(engine: &Engine<'_>) -> RealtimeEncounter {
    engine.realtime().unwrap().clone()
}

fn fighter(engine: &Engine<'_>, id: &str) -> Fighter {
    let fight = fight(engine);
    fight.fighters[fight.index(id).unwrap()].clone()
}

fn hp(engine: &Engine<'_>, id: &str) -> u32 {
    fighter(engine, id).vitals.unwrap().hp
}

fn active(f: &Fighter) -> bool {
    matches!(
        f.action,
        RealtimeAction::Attack {
            phase: AttackPhase::Active,
            connected: false,
            ..
        }
    )
}

/// One frame of a host whose every blow lands where it was aimed, if that
/// fighter still stands; a ready player strikes at the first foe still
/// fighting; then the clock moves one step. Returns the commands it sent.
fn frame(engine: &mut Engine<'_>) -> Vec<Command> {
    let mut sent = Vec::new();
    for f in fight(engine).fighters.iter().filter(|f| active(f)) {
        let command = contact(&f.character);
        if engine.execute(command.clone()).is_ok() {
            sent.push(command);
        }
        if engine.realtime().is_none() {
            return sent;
        }
    }
    let fight = fight(engine);
    let player = vitals(engine);
    if fight.fighters[0].action == RealtimeAction::Ready {
        let foe = (1..fight.fighters.len())
            .find(|&i| fight.fighters[i].team != 0 && fight.fighting(i, &player))
            .unwrap();
        sent.push(strike_at(&fight.fighters[foe].character));
        engine.execute(sent.last().unwrap().clone()).unwrap();
    }
    sent.push(step(1));
    engine.execute(step(1)).unwrap();
    sent
}

/// Frames until the fight ends, or `limit` frames pass.
fn fight_on(engine: &mut Engine<'_>, limit: usize) -> Vec<Command> {
    let mut sent = Vec::new();
    for _ in 0..limit {
        if engine.realtime().is_none() {
            break;
        }
        sent.extend(frame(engine));
    }
    sent
}

/// The command was refused, for exactly this reason.
fn assert_refused(result: Result<Vec<Event>, EngineError>, expected: EngineError) {
    assert_eq!(
        format!("{:?}", result.unwrap_err()),
        format!("{expected:?}")
    );
}

fn count(events: &[Event], wanted: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

#[test]
fn a_fight_grows_from_one_on_one_to_two_on_two_and_settles_once() {
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new_with_seed(&world, 4).unwrap();
    let mut commands = vec![Duel("brigand".into())];
    run(&mut engine, &commands);
    commands.extend(fight_on(&mut engine, 12));
    // An enemy, then a friend, arrive in the middle of the fight.
    commands.extend([join("left_twin", 1), join("warden", 0)]);
    run(&mut engine, &commands[commands.len() - 2..]);
    let mut seen_four = false;
    while engine.realtime().is_some() {
        seen_four |= fight(&engine).fighters.len() == 4;
        commands.extend(frame(&mut engine));
    }
    assert!(seen_four);
    // The same commands replay to the same fight, joins and targets included.
    let mut first = Engine::new_with_seed(&world, 4).unwrap();
    let events = run(&mut first, &commands);
    let mut second = Engine::new_with_seed(&world, 4).unwrap();
    assert_eq!(run(&mut second, &commands), events);
    assert_eq!(first.snapshot().to_json(), second.snapshot().to_json());
    assert_eq!(first.snapshot().to_json(), engine.snapshot().to_json());
    // Both foes fell and the warden struck too; the fight ended once, at
    // the last foe, and paid each fallen foe once. The warden gives nothing.
    let joined: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Joined { character, team } => Some((character.as_str(), *team)),
            _ => None,
        })
        .collect();
    assert_eq!(joined, [("left_twin", 1), ("warden", 0)]);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Struck { attacker, .. } if attacker == "warden")));
    assert_eq!(
        count(&events, |e| matches!(e, Event::EnemyDefeated { .. })),
        2
    );
    let ended = events
        .iter()
        .position(|e| matches!(e, Event::EncounterEnded { .. }))
        .unwrap();
    assert_eq!(
        events[ended],
        Event::EncounterEnded {
            outcome: Outcome::Victory
        }
    );
    assert_eq!(
        count(&events, |e| matches!(e, Event::EncounterEnded { .. })),
        1
    );
    assert!(events[..ended].iter().all(|e| !matches!(
        e,
        Event::ExperienceGranted { .. } | Event::ItemReceived { .. }
    )));
    let xp: Vec<_> = events[ended..]
        .iter()
        .filter_map(|e| match e {
            Event::ExperienceGranted { amount } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(xp, [10, 4], "the brigand's, then the twin's, in join order");
    let purses = count(
        &events,
        |e| matches!(e, Event::ItemReceived { item, .. } if item == "coin_purse"),
    );
    assert_eq!(purses, 1);
    let defeated = &combat(&engine).defeated;
    assert!(defeated.contains("brigand") && defeated.contains("left_twin"));
    assert!(!defeated.contains("warden"));
    // Nothing more can settle it.
    assert!(matches!(
        engine.execute(step(1)),
        Err(EngineError::NotFighting)
    ));
}

#[test]
fn a_group_starts_one_against_two_and_strikes_must_name_a_foe() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    let events = run(&mut engine, &[Duel("right_twin".into())]);
    assert_eq!(
        events,
        [Event::EncounterStarted {
            opponents: vec!["left_twin".into(), "right_twin".into()]
        }]
    );
    let start = fight(&engine);
    let teams: Vec<_> = start
        .fighters
        .iter()
        .map(|f| (f.character.as_str(), f.team, f.control, f.joined))
        .collect();
    assert_eq!(
        teams,
        [
            ("trainee", 0, Control::Player, 0),
            ("left_twin", 1, Control::Policy, 0),
            ("right_twin", 1, Control::Policy, 0)
        ]
    );
    // With two foes standing a strike names one, and only a foe.
    let before = engine.snapshot().to_json();
    for (command, refusal) in [
        (strike(), EngineError::NeedTarget),
        (
            strike_at("trainee"),
            EngineError::InvalidTarget("trainee".into()),
        ),
        (
            strike_at("brigand"),
            EngineError::InvalidTarget("brigand".into()),
        ),
    ] {
        assert_refused(engine.execute(command), refusal);
        assert_eq!(engine.snapshot().to_json(), before);
    }
    // The menu offers a strike at each of them.
    let offers = offered(&engine);
    for twin in ["left_twin", "right_twin"] {
        assert!(offers.contains(&(strike_at(twin), true)));
    }
    assert!(!offers.contains(&(strike(), true)));
    run(&mut engine, &[strike_at("right_twin"), step(1)]);
    let RealtimeAction::Attack { target, .. } = fighter(&engine, "trainee").action else {
        panic!("attacking");
    };
    assert_eq!(target, "right_twin");
    // Both twins aim at the player, the first foe in join order, at once.
    for twin in ["left_twin", "right_twin"] {
        let RealtimeAction::Attack { target, .. } = fighter(&engine, twin).action else {
            panic!("{twin} attacking");
        };
        assert_eq!(target, "trainee");
    }
}

#[test]
fn joining_keeps_the_clock_and_everyone_as_they_were() {
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into()), strike(), step(3)]);
    run(&mut engine, &[contact("trainee")]);
    let before = fight(&engine);
    let player = vitals(&engine);
    let events = run(&mut engine, &[join("warden", 0)]);
    assert_eq!(
        events,
        [Event::Joined {
            character: "warden".into(),
            team: 0
        }]
    );
    let after = fight(&engine);
    assert_eq!(after.step, before.step);
    assert_eq!(after.fighters[..2], before.fighters[..]);
    assert_eq!(vitals(&engine), player);
    let warden = &after.fighters[2];
    assert_eq!(
        (warden.team, warden.control, warden.joined, &warden.action),
        (0, Control::Policy, before.step, &RealtimeAction::Ready)
    );
    assert_eq!(warden.vitals.unwrap().hp, 30);
    // One foe left, so the player's strike still needs no target; the
    // warden begins his first attack at the next step, at the brigand.
    assert!(offered(&engine).contains(&(strike(), false)));
    run(&mut engine, &[step(1)]);
    let RealtimeAction::Attack { target, .. } = fighter(&engine, "warden").action else {
        panic!("attacking");
    };
    assert_eq!(target, "brigand");
}

#[test]
fn refused_joins_change_nothing() {
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    // Nobody joins a fight that is not running.
    assert_refused(engine.execute(join("warden", 0)), EngineError::NotFighting);
    run(
        &mut engine,
        &[Duel("brigand".into()), step(2), join("warden", 0)],
    );
    let before = engine.snapshot().to_json();
    for (command, refusal) in [
        (
            join("brigand", 1),
            EngineError::AlreadyJoined("brigand".into()),
        ),
        (
            join("warden", 0),
            EngineError::AlreadyJoined("warden".into()),
        ),
        (
            join("trainee", 0),
            EngineError::AlreadyJoined("trainee".into()),
        ),
        (join("nobody", 1), EngineError::NotHere("nobody".into())),
        // An ally joins only the player's side, a foe never does.
        (
            join("master", 0),
            EngineError::InvalidTeam {
                character: "master".into(),
                team: 0,
            },
        ),
        // Teams are numbered as they appear: 1 exists, 2 would be new, 3 skips one.
        (
            join("master", 3),
            EngineError::InvalidTeam {
                character: "master".into(),
                team: 3,
            },
        ),
    ] {
        assert_refused(engine.execute(command.clone()), refusal);
        assert_eq!(engine.snapshot().to_json(), before, "{command:?}");
    }
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into())]);
    let before = engine.snapshot().to_json();
    assert_refused(
        engine.execute(join("warden", 1)),
        EngineError::InvalidTeam {
            character: "warden".into(),
            team: 1,
        },
    );
    assert_eq!(engine.snapshot().to_json(), before);
    // A defeated foe stays defeated, and an ally is never the one fought.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into())]);
    fight_on(&mut engine, 500);
    assert!(combat(&engine).defeated.contains("brigand"));
    assert_refused(
        engine.execute(Duel("warden".into())),
        EngineError::NotHostile("warden".into()),
    );
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| *c == Duel("warden".into())));
    run(&mut engine, &[Duel("master".into())]);
    let before = engine.snapshot().to_json();
    assert_refused(
        engine.execute(join("brigand", 1)),
        EngineError::AlreadyDefeated("brigand".into()),
    );
    assert_eq!(engine.snapshot().to_json(), before);
}

#[test]
fn friendly_fire_is_refused_unless_the_world_allows_it() {
    let mut world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("brigand".into()), join("warden", 0), step(4)],
    );
    // The warden's wind-up (3) is over and his blow at the brigand is live.
    assert!(active(&fighter(&engine, "warden")));
    let before = engine.snapshot().to_json();
    for (command, refusal) in [
        (
            contact_on("warden", "trainee"),
            EngineError::NotHostile("trainee".into()),
        ),
        (
            contact_on("warden", "warden"),
            EngineError::InvalidTarget("warden".into()),
        ),
        (
            contact_on("warden", "master"),
            EngineError::InvalidTarget("master".into()),
        ),
        (
            strike_at("warden"),
            EngineError::NotHostile("warden".into()),
        ),
        (contact("brigand"), EngineError::NoContact("brigand".into())),
    ] {
        assert_refused(engine.execute(command.clone()), refusal);
        assert_eq!(engine.snapshot().to_json(), before, "{command:?}");
    }
    // Where the world allows it, a stray blow can catch a teammate, though
    // nobody aims at one.
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .realtime
        .as_mut()
        .unwrap()
        .friendly_fire = true;
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("brigand".into()), join("warden", 0), step(4)],
    );
    let events = run(&mut engine, &[contact_on("warden", "trainee")]);
    assert!(matches!(
        &events[..],
        [Event::DamageReceived { source, .. }] if source == "warden"
    ));
    assert!(vitals(&engine).hp < 40);
    assert_refused(
        engine.execute(strike_at("warden")),
        EngineError::NotHostile("warden".into()),
    );
}

#[test]
fn the_first_foe_to_fall_does_not_end_the_fight() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("left_twin".into())]);
    let mut events = Vec::new();
    while hp(&engine, "left_twin") > 0 {
        let fight = fight(&engine);
        if active(&fight.fighters[0]) {
            events.extend(run(&mut engine, &[contact("trainee")]));
        } else if fight.fighters[0].action == RealtimeAction::Ready {
            events.extend(run(&mut engine, &[strike_at("left_twin")]));
        } else {
            events.extend(run(&mut engine, &[step(1)]));
        }
    }
    assert_eq!(
        events.last(),
        Some(&Event::EnemyDefeated {
            monster: "left_twin".into()
        })
    );
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::EncounterEnded { .. } | Event::ExperienceGranted { .. }
    )));
    // The fallen twin is out: whatever he was doing stops, he regains
    // nothing and never acts again, and nobody can aim at him.
    let fallen = fighter(&engine, "left_twin");
    assert_eq!(fallen.action, RealtimeAction::Ready);
    run(&mut engine, &[step(30)]);
    assert_eq!(fighter(&engine, "left_twin"), fallen);
    assert_refused(
        engine.execute(contact("left_twin")),
        EngineError::NoContact("left_twin".into()),
    );
    // One foe is left, so a strike needs no target again.
    assert!(offered(&engine).iter().any(|(c, _)| *c == strike()));
    let events = {
        let mut events = Vec::new();
        while engine.realtime().is_some() {
            let fight = fight(&engine);
            let command = if active(&fight.fighters[0]) {
                contact("trainee")
            } else if fight.fighters[0].action == RealtimeAction::Ready {
                strike()
            } else {
                step(1)
            };
            events.extend(run(&mut engine, &[command]));
        }
        events
    };
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::ExperienceGranted { .. }))
            .count(),
        2,
        "both twins pay out, once each, at the end"
    );
}

#[test]
fn allies_guard_fall_and_yield_by_the_same_rules_as_foes() {
    // A warden who holds his guard 5 steps before each attack.
    let world = yard_with_warden(json!({ "guard_hold": 5 }));
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into()), join("warden", 0)]);
    let events = run(&mut engine, &[step(1)]);
    assert!(events.contains(&Event::GuardRaising {
        character: "warden".into()
    }));
    // Up at step 2 (a raise of 1), held to step 7, then he attacks.
    assert_eq!(
        fighter(&engine, "warden").action,
        RealtimeAction::Guard { ready_at: 2 }
    );
    run(&mut engine, &[step(5)]);
    assert!(matches!(
        fighter(&engine, "warden").action,
        RealtimeAction::Guard { .. }
    ));
    run(&mut engine, &[step(1)]);
    assert!(matches!(
        fighter(&engine, "warden").action,
        RealtimeAction::Attack { .. }
    ));
    // A guarded warden takes the guard's share of a blow, like the player.
    let world = yard_with_warden(json!({ "guard_hold": 50 }));
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("brigand".into()), join("warden", 0), step(3)],
    );
    // The brigand aims at the player, but the host reports his blow on the warden.
    let brigand = fighter(&engine, "brigand");
    let RealtimeAction::Attack { until, .. } = brigand.action else {
        panic!("attacking");
    };
    let wait = u32::try_from(until - fight(&engine).step).unwrap();
    run(&mut engine, &[step(wait)]);
    let events = run(&mut engine, &[contact_on("brigand", "warden")]);
    assert!(matches!(&events[0], Event::Blocked { character, .. } if character == "warden"));
    assert!(matches!(&events[1], Event::Struck { attacker, target, .. }
        if attacker == "brigand" && target == "warden"));

    // A warden who falls is out, and the fight goes on without reward for him.
    let mut world = yard_with_warden(json!({}));
    world
        .characters
        .iter_mut()
        .find(|c| c.id == "warden")
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .stats
        .hp = 1;
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into()), join("warden", 0)]);
    let mut events = Vec::new();
    while hp(&engine, "warden") > 0 {
        let brigand = fighter(&engine, "brigand");
        let command = if active(&brigand) {
            contact_on("brigand", "warden")
        } else {
            step(1)
        };
        events.extend(run(&mut engine, &[command]));
    }
    assert_eq!(
        events.last(),
        Some(&Event::AllyDown {
            character: "warden".into()
        })
    );
    assert!(engine.realtime().is_some());
    assert_eq!(fighter(&engine, "warden").action, RealtimeAction::Ready);

    // In a yielding group an ally yields at its threshold instead.
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), join("warden", 0)]);
    let mut events = Vec::new();
    while !fighter(&engine, "warden").yielded {
        let master = fighter(&engine, "master");
        let command = if active(&master) {
            contact_on("master", "warden")
        } else {
            step(1)
        };
        events.extend(run(&mut engine, &[command]));
    }
    assert!(events.contains(&Event::Yielded {
        character: "warden".into()
    }));
    assert!(hp(&engine, "warden") > 0);
    assert!(engine.realtime().is_some(), "the player still fights");
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn engine_fighters_aim_by_their_authored_targeting() {
    let mut world = yard_with_warden(json!({}));
    combatant(&mut world, "brigand").targeting = Targeting::Weakest;
    let mut engine = Engine::new(&world).unwrap();
    // The warden (30 HP) is weaker than the player (40), so the brigand,
    // who aims at the weakest, takes him, though the player came first.
    run(
        &mut engine,
        &[Duel("brigand".into()), join("warden", 0), step(1)],
    );
    let RealtimeAction::Attack { target, .. } = fighter(&engine, "brigand").action else {
        panic!("attacking");
    };
    assert_eq!(target, "warden");
    // By default a fighter aims at the first foe in join order.
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("brigand".into()), join("warden", 0), step(1)],
    );
    let RealtimeAction::Attack { target, .. } = fighter(&engine, "brigand").action else {
        panic!("attacking");
    };
    assert_eq!(target, "trainee");
}

#[test]
fn a_third_team_fights_everyone_else() {
    let world = hardy(yard());
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("left_twin".into())]);
    // The twins are team 1; the brigand comes over the wall for himself.
    run(&mut engine, &[join("brigand", 2), step(1)]);
    assert_eq!(fighter(&engine, "brigand").team, 2);
    let RealtimeAction::Attack { target, .. } = fighter(&engine, "brigand").action else {
        panic!("attacking");
    };
    assert_eq!(target, "trainee");
    // Every team is hostile to every other: a twin's blow can land on him.
    let RealtimeAction::Attack { until, .. } = fighter(&engine, "left_twin").action else {
        panic!("attacking");
    };
    let wait = u32::try_from(until - fight(&engine).step).unwrap();
    run(&mut engine, &[step(wait)]);
    let events = run(&mut engine, &[contact_on("left_twin", "brigand")]);
    assert!(matches!(&events[..], [Event::Struck { target, .. }] if target == "brigand"));
    // Victory waits for both other teams.
    let sent = fight_on(&mut engine, 2_000);
    assert!(engine.realtime().is_none(), "{} commands", sent.len());
    let defeated = &combat(&engine).defeated;
    for id in ["left_twin", "right_twin", "brigand"] {
        assert!(defeated.contains(id), "{id}");
    }
}

#[test]
fn reinforcements_after_the_victory_start_a_new_fight() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into())]);
    fight_on(&mut engine, 500);
    assert!(engine.realtime().is_none());
    // The fight was settled by the blow that ended it; a twin arriving now
    // finds no fight to join, and has to be fought afresh.
    assert_refused(
        engine.execute(join("left_twin", 1)),
        EngineError::NotFighting,
    );
    let events = run(&mut engine, &[Duel("left_twin".into())]);
    assert!(matches!(&events[..], [Event::EncounterStarted { .. }]));
    assert_eq!(fight(&engine).step, 0);
}

#[test]
fn a_reinforcement_keeps_its_own_groups_record() {
    let mut world = hardy(yard());
    let groups = &mut world.world.combat.as_mut().unwrap().groups;
    groups
        .iter_mut()
        .find(|g| g.id == "twins")
        .unwrap()
        .repeatable = true;
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("left_twin".into()), join("brigand", 1)]);
    fight_on(&mut engine, 2_000);
    assert!(engine.realtime().is_none());
    let defeated = &combat(&engine).defeated;
    assert!(defeated.contains("brigand"));
    assert!(!defeated.contains("left_twin") && !defeated.contains("right_twin"));
}

#[test]
fn a_grown_fight_saved_at_any_point_resumes_exactly() {
    let world = yard_with_warden(json!({ "guard_hold": 2 }));
    let mut whole = Engine::new_with_seed(&world, 3).unwrap();
    let mut script = vec![Duel("brigand".into())];
    run(&mut whole, &script);
    script.extend(fight_on(&mut whole, 6));
    script.push(join("warden", 0));
    whole.execute(script.last().unwrap().clone()).unwrap();
    script.extend(fight_on(&mut whole, 6));
    script.push(join("left_twin", 1));
    whole.execute(script.last().unwrap().clone()).unwrap();
    script.extend(fight_on(&mut whole, 2_000));
    assert!(whole.realtime().is_none());
    let mut fresh = Engine::new_with_seed(&world, 3).unwrap();
    let events = run(&mut fresh, &script);
    for cut in (1..script.len()).step_by(7) {
        let mut first = Engine::new_with_seed(&world, 3).unwrap();
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
fn saves_with_fight_states_the_rules_cannot_reach_are_refused() {
    let world = yard_with_warden(json!({}));
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("brigand".into()), step(2), join("warden", 0), step(1)],
    );
    run(&mut engine, &[join("left_twin", 1)]);
    Engine::restore(&world, engine.snapshot()).unwrap();
    let tamper = |edit: &dyn Fn(&mut RealtimeEncounter)| {
        let mut snapshot = engine.snapshot();
        let combat = snapshot.state.combat.as_mut().unwrap();
        let Stance::Realtime(fight) = &mut combat.stance else {
            panic!("fighting");
        };
        edit(fight);
        Engine::restore(&world, snapshot).is_err()
    };
    // Fighters: trainee, brigand (1), warden (0, joined at 2), left twin (1, at 3).
    assert!(!tamper(&|_| {}));
    assert!(tamper(&|f| f.fighters.swap(0, 1)));
    assert!(tamper(&|f| f.fighters[3].character = "brigand".into()));
    assert!(tamper(&|f| f.fighters[3].character = "trainee".into()));
    assert!(
        tamper(&|f| f.fighters[2].team = 1),
        "an ally on a foe's team"
    );
    assert!(
        tamper(&|f| f.fighters[3].team = 0),
        "a foe on the player's team"
    );
    assert!(tamper(&|f| f.fighters[3].team = 3), "a team number skipped");
    assert!(!tamper(&|f| f.fighters[3].team = 2), "a new team");
    assert!(
        tamper(&|f| f.fighters[1].team = 2),
        "the first foe is team 1"
    );
    assert!(tamper(&|f| f.fighters[2].control = Control::Player));
    assert!(tamper(&|f| f.fighters[0].team = 1));
    assert!(tamper(&|f| f.fighters[3].joined = 4), "joined after now");
    assert!(tamper(&|f| f.fighters[3].joined = 1), "joined out of order");
    assert!(tamper(&|f| f.fighters[2].vitals = None));
    // A twin cannot be in the fight without his brother if they started it.
    let mut twins = Engine::new(&world).unwrap();
    run(&mut twins, &[Duel("left_twin".into())]);
    let mut snapshot = twins.snapshot();
    let Stance::Realtime(fight) = &mut snapshot.state.combat.as_mut().unwrap().stance else {
        panic!("fighting");
    };
    fight.fighters.pop();
    assert!(Engine::restore(&world, snapshot).is_err());
    // Actions: aimed at a foe, out fighters idle, guards only by policy.
    assert!(tamper(&|f| {
        if let RealtimeAction::Attack { target, .. } = &mut f.fighters[1].action {
            *target = "left_twin".into();
        }
    }));
    assert!(tamper(
        &|f| f.fighters[2].action = RealtimeAction::Guard { ready_at: 3 }
    ));
    assert!(tamper(
        &|f| f.fighters[3].action = RealtimeAction::Retreat { until: 5 }
    ));
    assert!(
        tamper(&|f| f.fighters[1].vitals.as_mut().unwrap().hp = 0),
        "an out fighter attacking"
    );
    assert!(
        tamper(&|f| f.fighters[2].action = RealtimeAction::Ready),
        "an idle policy"
    );
    assert!(
        !tamper(&|f| f.fighters[3].action = RealtimeAction::Ready),
        "it joined now"
    );
}
