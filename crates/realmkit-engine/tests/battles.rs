//! Mass battles against authored armies, whose numbers come from
//! `python3 -m scripts.combat_sim battle examples/marches --army outlaws ... --seed 7`.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn squad(engine: &Engine<'_>, line: &str, level: usize) -> Option<Squad> {
    engine
        .state()
        .retinue
        .as_ref()?
        .roster
        .get(line)?
        .get(&level)
        .copied()
}

fn healthy(healthy: u64) -> Squad {
    Squad {
        healthy,
        wounded: 0,
        xp: 0,
    }
}

/// At Ashmere, facing the outlaws, with exactly `squads` in the roster.
fn before_the_outlaws<'w>(world: &'w WorldSpec, squads: &[(&str, usize, Squad)]) -> Engine<'w> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let mut snapshot = engine.snapshot();
    let roster = &mut snapshot.state.retinue.as_mut().unwrap().roster;
    for (line, level, squad) in squads {
        roster
            .entry((*line).into())
            .or_default()
            .insert(*level, *squad);
    }
    Engine::restore(world, snapshot).unwrap()
}

fn counts(engine: &Engine<'_>, side: usize) -> Vec<u64> {
    engine.battle().unwrap().sides[side]
        .stacks
        .iter()
        .map(|s| s.count)
        .collect()
}

fn outcome(events: &[Event]) -> Option<BattleOutcome> {
    events.iter().find_map(|e| match e {
        Event::BattleEnded { outcome, .. } => Some(*outcome),
        _ => None,
    })
}

#[test]
fn six_levies_lose_to_the_outlaws_as_the_simulator_says() {
    let world = marches();
    let mut engine = before_the_outlaws(&world, &[("levy", 1, healthy(6))]);
    let events = engine.execute(Engage("outlaws".into())).unwrap();
    assert_eq!(
        events,
        [Event::BattleStarted {
            army: "outlaws".into(),
            allies: vec![]
        }]
    );
    assert_eq!(counts(&engine, 0), [6]);
    assert_eq!(counts(&engine, 1), [8, 2, 4]);
    let events = engine.execute(Autoresolve).unwrap();
    let rounds: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::BattleRound {
                strengths,
                losses,
                morale,
                ..
            } => Some((*strengths, *losses, *morale)),
            _ => None,
        })
        .collect();
    assert_eq!(
        rounds,
        [([5, 14], [2, 1], [58, 90]), ([2, 14], [2, 0], [15, 90])]
    );
    assert_eq!(outcome(&events), Some(BattleOutcome::Defeat));
    // One levy stands; of the five lost, half (rounded down) are wounded.
    assert_eq!(
        squad(&engine, "levy", 1),
        Some(Squad {
            healthy: 1,
            wounded: 2,
            xp: 0
        })
    );
    assert_eq!(vitals(&engine).hp, 18);
    assert!(engine.battle().is_none());
}

#[test]
fn a_mixed_force_holds_flanks_and_wins_and_shares_the_spoils() {
    let world = marches();
    let mut engine = before_the_outlaws(
        &world,
        &[
            ("levy", 3, healthy(6)),
            ("bowmen", 1, healthy(4)),
            ("riders", 1, healthy(3)),
        ],
    );
    engine.execute(Engage("outlaws".into())).unwrap();
    engine.execute(Order(BattleOrder::Hold)).unwrap();
    let events = engine.execute(Order(BattleOrder::Flank)).unwrap();
    assert_eq!(outcome(&events), Some(BattleOutcome::Victory));
    // 60 XP: 18 to the player, 42 shared by 5 spearmen, 4 bowmen and 3 riders.
    assert!(events.contains(&Event::ExperienceGranted { amount: 18 }));
    assert_eq!(
        squad(&engine, "levy", 3),
        Some(Squad {
            healthy: 5,
            wounded: 0,
            xp: 17
        })
    );
    assert_eq!(
        squad(&engine, "bowmen", 1),
        Some(Squad {
            healthy: 4,
            wounded: 0,
            xp: 14
        })
    );
    assert_eq!(
        squad(&engine, "riders", 1),
        Some(Squad {
            healthy: 3,
            wounded: 0,
            xp: 10
        })
    );
    assert_eq!(vitals(&engine).hp, 32);
    assert_eq!(engine.state().player.inventory.get("eels"), Some(&4));
    // The outlaws come back: they are repeatable.
    engine.execute(Engage("outlaws".into())).unwrap();
}

#[test]
fn commanding_only_charges_ends_as_autoresolve_does() {
    let world = marches();
    let squads = [("levy", 1, healthy(6))];
    let mut auto = before_the_outlaws(&world, &squads);
    auto.execute(Engage("outlaws".into())).unwrap();
    auto.execute(Autoresolve).unwrap();
    let mut commanded = before_the_outlaws(&world, &squads);
    commanded.execute(Engage("outlaws".into())).unwrap();
    while commanded.battle().is_some() {
        commanded.execute(Order(BattleOrder::Charge)).unwrap();
    }
    assert_eq!(auto.state().retinue, commanded.state().retinue);
    assert_eq!(auto.state().rng, commanded.state().rng);
    assert_eq!(vitals(&auto), vitals(&commanded));
}

#[test]
fn the_keep_guard_joins_once_the_letter_is_read_and_the_fen_thaws() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    for command in [
        Travel("ashmere".into()),
        Talk("reeve".into()),
        ChooseDialogue(1),
        Recruit {
            line: "levy".into(),
            quantity: 4,
        },
        Travel("greyford".into()),
        Travel("hollin_keep".into()),
        Talk("steward".into()),
        ChooseDialogue(1),
        Travel("greyford".into()),
        Travel("ashmere".into()),
    ] {
        engine.execute(command).unwrap();
    }
    // The warden is here, but the fen has not thawed: he does not join.
    let mut early = engine.clone();
    let events = early.execute(Engage("outlaws".into())).unwrap();
    assert!(events.contains(&Event::BattleStarted {
        army: "outlaws".into(),
        allies: vec![]
    }));
    // An ally is never an enemy.
    assert!(matches!(
        engine.execute(Engage("warden".into())),
        Err(EngineError::NotHostile(_))
    ));
    engine.execute(Wait(2_280 - 1_320)).unwrap();
    let events = engine.execute(Engage("outlaws".into())).unwrap();
    assert!(events.contains(&Event::BattleStarted {
        army: "outlaws".into(),
        allies: vec!["warden".into()]
    }));
    assert_eq!(counts(&engine, 0), [4, 6]);
    let events = engine.execute(Autoresolve).unwrap();
    assert_eq!(outcome(&events), Some(BattleOutcome::Victory));
    // As the simulator has it: two levies stand and five keep guards; the
    // guards' losses are theirs, not ours.
    // The levies' 42 XP (14 a head) lifts them to level 2 for 6 each.
    assert!(events.contains(&Event::Promoted {
        line: "levy".into(),
        level: 2,
        count: 3
    }));
    assert_eq!(
        squad(&engine, "levy", 2),
        Some(Squad {
            healthy: 2,
            wounded: 1,
            xp: 24
        })
    );
    assert_eq!(vitals(&engine).hp, 27);
}

#[test]
fn a_retreat_costs_a_pursuit_and_the_battle() {
    let world = marches();
    let mut engine = before_the_outlaws(&world, &[("levy", 1, healthy(6))]);
    engine.execute(Engage("outlaws".into())).unwrap();
    let events = engine.execute(Order(BattleOrder::Retreat)).unwrap();
    assert_eq!(outcome(&events), Some(BattleOutcome::Defeat));
    assert_eq!(
        squad(&engine, "levy", 1),
        Some(Squad {
            healthy: 4,
            wounded: 1,
            xp: 0
        })
    );
    assert_eq!(vitals(&engine).hp, 36);
}

#[test]
fn a_player_beaten_alone_is_knocked_out_not_killed() {
    let world = marches();
    let mut engine = before_the_outlaws(&world, &[]);
    engine.execute(Engage("outlaws".into())).unwrap();
    let events = engine.execute(Autoresolve).unwrap();
    assert_eq!(outcome(&events), Some(BattleOutcome::Defeat));
    assert!(!engine.is_dead());
    assert_eq!(vitals(&engine).hp, 1);
    engine.execute(Rest).unwrap_err(); // Ashmere is not safe, but play goes on
    engine.execute(Travel("greyford".into())).unwrap();
}

#[test]
fn a_battle_saved_between_rounds_resumes_exactly() {
    let world = marches();
    let mut engine = before_the_outlaws(
        &world,
        &[("levy", 3, healthy(6)), ("riders", 1, healthy(3))],
    );
    engine.execute(Engage("outlaws".into())).unwrap();
    engine.execute(Order(BattleOrder::Hold)).unwrap();
    let mut resumed = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(
        engine.execute(Autoresolve).unwrap(),
        resumed.execute(Autoresolve).unwrap()
    );
    assert_eq!(engine.state(), resumed.state());
}

#[test]
fn only_orders_are_given_in_a_battle_and_refusals_draw_nothing() {
    let world = marches();
    let mut engine = before_the_outlaws(&world, &[("levy", 1, healthy(6))]);
    engine.execute(Engage("outlaws".into())).unwrap();
    let before = engine.state().clone();
    assert!(engine.execute(Travel("greyford".into())).is_err());
    // No riders, so no flank.
    assert!(matches!(
        engine.execute(Order(BattleOrder::Flank)),
        Err(EngineError::NoFlank)
    ));
    assert_eq!(engine.state(), &before);
    let offers = offered(&engine);
    assert!(offers.contains(&(Order(BattleOrder::Charge), true)));
    assert!(offers.contains(&(Autoresolve, true)));
    assert!(!offers.iter().any(|(c, _)| *c == Order(BattleOrder::Flank)));
    // Away from a battle there are no orders.
    let mut idle = before_the_outlaws(&world, &[]);
    assert!(matches!(
        idle.execute(Autoresolve),
        Err(EngineError::NotInBattle)
    ));
}

#[test]
fn saves_reject_battles_the_rules_could_not_produce() {
    let world = marches();
    let mut engine = before_the_outlaws(&world, &[("levy", 1, healthy(6))]);
    engine.execute(Engage("outlaws".into())).unwrap();
    engine.execute(Order(BattleOrder::Charge)).unwrap();
    let good = engine.snapshot();
    Engine::restore(&world, good.clone()).unwrap();
    let broken: Vec<fn(&mut BattleState)> = vec![
        |b| b.army = "warden".into(),
        |b| b.sides[0].stacks[0].count = 7,
        |b| b.sides[1].stacks[0].count = 9,
        |b| b.sides[1].stacks.pop().map(|_| ()).unwrap(),
        |b| b.round = 8,
        |b| b.sides[0].morale = 101,
        |b| b.sides[1].stacks[0].remainder = 18,
        |b| b.allies.push("warden".into()),
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        let Stance::Battle(battle) = &mut snapshot.state.combat.as_mut().unwrap().stance else {
            panic!("not in a battle");
        };
        corrupt(battle);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn a_beaten_army_is_recorded_and_its_spoils_stay_owed() {
    let mut world = marches();
    world
        .characters
        .iter_mut()
        .find(|c| c.id == "outlaws")
        .unwrap()
        .army
        .as_mut()
        .unwrap()
        .repeatable = false;
    // Loot no market or effect moves: a token only this victory gives.
    let outlaws = world.characters.iter_mut().find(|c| c.id == "outlaws");
    outlaws.unwrap().army.as_mut().unwrap().loot = vec![ItemStack {
        item: "keep_token".into(),
        quantity: 1,
    }];
    let mut engine = before_the_outlaws(
        &world,
        &[
            ("levy", 3, healthy(6)),
            ("bowmen", 1, healthy(4)),
            ("riders", 1, healthy(3)),
        ],
    );
    engine.execute(Engage("outlaws".into())).unwrap();
    engine.execute(Autoresolve).unwrap();
    assert!(combat(&engine).defeated.contains("outlaws"));
    assert!(matches!(
        engine.execute(Engage("outlaws".into())),
        Err(EngineError::NotHere(_))
    ));
    let good = engine.snapshot();
    Engine::restore(&world, good.clone()).unwrap();
    // The token the outlaws gave up cannot vanish.
    let mut eaten = good.clone();
    eaten.state.player.inventory.remove("keep_token");
    assert!(Engine::restore(&world, eaten).is_err());
}
