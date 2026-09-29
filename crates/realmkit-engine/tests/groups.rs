//! Packs, repeatable fights, fleeing, yielding and XP falloff.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn at(world: &WorldSpec, direction: Direction) -> Engine<'_> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(Move(direction)).unwrap();
    engine
}

#[test]
fn a_pack_fights_together_and_every_member_must_fall() {
    let world = arena();
    let mut engine = at(&world, North);
    let events = engine.execute(Engage("black_wolf".into())).unwrap();
    // Engaging either wolf brings both, in the den's authored order.
    assert!(events.contains(&Event::EncounterStarted {
        opponents: vec!["grey_wolf".into(), "black_wolf".into()]
    }));
    // The faster wolves opened; the player is next, then both wolves in order.
    assert_eq!(engine.turn_order(3), ["fighter", "grey_wolf", "black_wolf"]);
    let bites = events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .count();
    assert_eq!(bites, 2);
    // Felling the first wolf leaves the fight running against the second.
    let mut first = Vec::new();
    while engine.encounter().unwrap().participants[1].hp > 0 {
        first.extend(engine.execute(Attack("grey_wolf".into())).unwrap());
    }
    assert!(engine.encounter().is_some());
    assert!(!engine.turn_order(4).contains(&"grey_wolf".to_string()));
    assert!(matches!(
        engine.execute(Attack("grey_wolf".into())),
        Err(EngineError::NotHere(_))
    ));
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    let defeated = &combat(&engine).defeated;
    assert!(defeated.contains("grey_wolf") && defeated.contains("black_wolf"));
    assert_eq!(combat(&engine).xp, 16);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn xp_falls_off_with_level_difference_as_in_the_simulator() {
    // scripts/combat_sim XpRules.for_kill(100, player, monster) for monster − player.
    for (diff, expected) in [
        (-6, 0),
        (-5, 0),
        (-4, 60),
        (-1, 90),
        (0, 100),
        (2, 120),
        (4, 140),
        (9, 140),
    ] {
        let (player, monster) = if diff < 0 {
            (10, (10 + diff) as usize)
        } else {
            (10, 10 + diff as usize)
        };
        assert_eq!(
            xp_for_defeat(100, player, monster),
            expected,
            "difference {diff}"
        );
    }
    assert_eq!(xp_for_defeat(7, 1, 2), 7);
    assert_eq!(xp_for_defeat(u64::MAX, 1, 9), u64::MAX);
}

#[test]
fn a_repeatable_group_can_be_fought_again_and_rewards_every_victory() {
    let world = arena();
    let mut engine = at(&world, East);
    for round in 1..=3 {
        engine.execute(Engage("rat".into())).unwrap();
        let events = fight_out(&mut engine);
        assert!(events.contains(&Event::ItemReceived {
            item: "rat_tail".into(),
            quantity: 1
        }));
        assert_eq!(engine.state().player.inventory["rat_tail"], round);
    }
    // Never recorded, and 4 XP for each same-level rat.
    assert!(combat(&engine).defeated.is_empty());
    assert_eq!((combat(&engine).xp, combat(&engine).level), (12, 2));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn nothing_is_learned_from_opponents_five_levels_below() {
    // One 5-XP rat lifts the player from level 1 to 6; the next rat is five below.
    let mut world = arena();
    combatant(&mut world, "rat").xp = 5;
    for (level, entry) in world
        .world
        .combat
        .as_mut()
        .unwrap()
        .levels
        .iter_mut()
        .enumerate()
    {
        entry.xp = level as u64;
    }
    let mut engine = at(&world, East);
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!(combat(&engine).level, 6);
    engine.execute(Engage("rat".into())).unwrap();
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::ExperienceGranted { amount: 0 }));
    assert_eq!(combat(&engine).xp, 5);
}

#[test]
fn fleeing_resolves_at_the_players_next_turn_and_grants_nothing() {
    let world = arena();
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    let hp = vitals(&engine).hp;
    let events = engine.execute(Flee).unwrap();
    assert_eq!(events[0], Event::FleeStarted);
    // Both wolves act during the wind-up, then the escape happens.
    let bites = events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .count();
    assert_eq!(bites, 2);
    assert_eq!(
        events.last(),
        Some(&Event::EncounterEnded {
            outcome: Outcome::Fled
        })
    );
    assert!(vitals(&engine).hp < hp);
    assert!(engine.encounter().is_none() && combat(&engine).defeated.is_empty());
    assert_eq!(combat(&engine).xp, 0);
    // The wolves are whole again next time.
    engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(engine.encounter().unwrap().participants[1..]
        .iter()
        .all(|p| p.hp == 24));
}

#[test]
fn a_no_flee_group_cannot_be_fled_and_opponent_skills_follow_their_level() {
    let world = open_pit();
    let mut engine = at(&world, Down);
    engine.execute(Engage("ogre".into())).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| *c == Flee));
    let before = engine.state().clone();
    assert!(matches!(engine.execute(Flee), Err(EngineError::NoFlee)));
    assert_eq!(engine.state(), &before);
    // The level-4 ogre has smash (level 3) but not crush (level 5).
    let mut used = Vec::new();
    while engine.encounter().is_some() && !engine.is_dead() {
        for event in engine.execute(Attack("ogre".into())).unwrap() {
            if let Event::DamageReceived { skill: Some(s), .. } = event {
                used.push(s);
            }
        }
    }
    assert!(used.contains(&"smash".to_string()));
    assert!(!used.contains(&"crush".to_string()));
}

#[test]
fn a_yielding_opponent_ends_the_fight_alive_and_sets_the_victory_flags() {
    let world = arena();
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::Yielded {
        character: "holt".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::EnemyDefeated { .. })));
    assert!(engine.state().flags.contains("spar_won"));
    // Not recorded, no reward: the sergeant can spar again.
    assert!(combat(&engine).defeated.is_empty());
    assert_eq!(combat(&engine).xp, 0);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    engine.execute(Engage("holt".into())).unwrap();
}

#[test]
fn a_player_who_yields_keeps_playing_with_the_defeat_flags_set() {
    let mut world = arena();
    combatant(&mut world, "holt").stats.patk = STAT_BOUND;
    let mut engine = at(&world, West);
    // A hit never takes a yielder below 1 HP, so an overwhelming blow yields.
    // Equal speeds: the player's side acts first, then Holt strikes.
    engine.execute(Engage("holt".into())).unwrap();
    let events = engine.execute(Attack("holt".into())).unwrap();
    assert!(events.contains(&Event::Yielded {
        character: "fighter".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Yielded
    }));
    assert!(!events.contains(&Event::PlayerDied));
    assert_eq!(vitals(&engine).hp, 1);
    assert!(engine.state().flags.contains("spar_lost"));
    engine.execute(Move(East)).unwrap();
    engine.execute(Rest).unwrap();
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn group_encounters_save_exactly_and_reject_a_wrong_cast() {
    let world = arena();
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    engine.execute(Attack("grey_wolf".into())).unwrap();
    let good = engine.snapshot();
    let mut resumed = Engine::restore(&world, good.clone()).unwrap();
    assert_eq!(fight_out(&mut resumed), fight_out(&mut engine));
    let broken: Vec<fn(&mut Encounter)> = vec![
        |e| e.participants.truncate(2),
        |e| e.participants.swap(1, 2),
        |e| e.participants[1].yielded = true,
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        let Stance::Fighting(encounter) = &mut snapshot.state.combat.as_mut().unwrap().stance
        else {
            unreachable!()
        };
        corrupt(encounter);
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn a_pack_is_rewarded_at_the_level_the_player_fought_at() {
    // The first wolf's 8 XP reaches level 2; the second still pays in full,
    // whatever order the den lists them in.
    let mut world = arena();
    world.world.combat.as_mut().unwrap().levels[1].xp = 8;
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!((combat(&engine).xp, combat(&engine).level), (16, 2));
}

#[test]
fn a_save_after_one_sparring_partner_yields_still_loads() {
    // A second partner keeps the fight going after the first yields, while the
    // yielder's turn time falls behind the schedule.
    let mut world = arena();
    let mut second = world
        .characters
        .iter()
        .find(|c| c.id == "holt")
        .unwrap()
        .clone();
    second.id = "corporal".into();
    second.name = "Corporal Wren".into();
    // Weak enough that the player outlasts Holt.
    second.combat.as_mut().unwrap().stats.patk = 1;
    world.characters.push(second);
    world.locations[3].characters.push("corporal".into());
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    while !engine.encounter().unwrap().participants[1].yielded {
        engine.execute(Attack("holt".into())).unwrap();
    }
    engine.execute(Attack("corporal".into())).unwrap();
    let encounter = engine.encounter().unwrap();
    assert!(encounter.participants[1].next_time < encounter.now);
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    // Yielding is exactly "at or below the threshold", and nobody dies.
    let broken: Vec<fn(&mut Encounter)> = vec![
        |e| e.participants[2].yielded = true,
        |e| e.participants[1].yielded = false,
        |e| (e.participants[2].hp, e.participants[2].yielded) = (0, false),
        |e| e.participants[1].hp = 0,
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        let Stance::Fighting(encounter) = &mut snapshot.state.combat.as_mut().unwrap().stance
        else {
            unreachable!()
        };
        corrupt(encounter);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn untouched_participants_at_a_full_yield_share_save_and_load() {
    // At a 100% share everyone starts within the threshold, but nobody has
    // yielded before the first hit; the player acts first on the tie.
    let mut world = arena();
    world.world.combat.as_mut().unwrap().groups[2].yield_share = Some(100);
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    assert!(!engine.encounter().unwrap().participants[1].yielded);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    // The first touch ends the bout.
    let events = engine.execute(Attack("holt".into())).unwrap();
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
}

#[test]
fn splitmix64_produces_its_published_sequence() {
    let mut state = 0;
    let outputs: Vec<u64> = (0..3).map(|_| splitmix64(&mut state)).collect();
    assert_eq!(
        outputs,
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f
        ]
    );
}
