//! Fights a host runs itself: the engine authorizes them, suspends the world,
//! and commits one checked settlement through the ordinary personal-fight end.

use realmkit_engine::{Command::*, Intent as I, *};
use realmkit_spec::*;
use std::collections::BTreeMap;

fn world(name: &str) -> WorldSpec {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/");
    WorldSpec::load(format!("{root}{name}")).unwrap()
}

/// The first hostile fighter offered a real-time fight here.
fn foe(engine: &Engine<'_>) -> Id {
    engine
        .actions()
        .iter()
        .find_map(|a| match &a.command {
            EngageRealtime(id) if a.available => Some(id.clone()),
            _ => None,
        })
        .expect("a real-time fight is offered")
}

fn begun<'w>(world: &'w WorldSpec) -> (Engine<'w>, HostedFight) {
    let mut engine = Engine::new(world).unwrap();
    let id = foe(&engine);
    engine.execute(HostFight(id)).unwrap();
    let fight = engine.hosted_fight().unwrap().clone();
    (engine, fight)
}

fn account(engine: &Engine<'_>, fight: &HostedFight, outcome: Outcome) -> FightSettlement {
    FightSettlement {
        fight: fight.id,
        outcome,
        fallen: fight.opponents.clone(),
        vitals: engine.player_vitals().unwrap(),
        worn: worn(engine),
        companions: BTreeMap::new(),
    }
}

fn worn(engine: &Engine<'_>) -> Vec<u64> {
    let gear = &engine.state().combat.as_ref().unwrap().gear;
    gear.iter()
        .filter(|(_, g)| g.equipped)
        .map(|(id, _)| *id)
        .collect()
}

#[test]
fn a_hosted_fight_suspends_the_world_and_settles_exactly_once() {
    let world = world("practice-yard");
    let (mut engine, fight) = begun(&world);
    assert!(engine.realtime().is_none(), "no engine-run fight");
    let held = engine.state().clone();
    for command in [
        Rest,
        PassTime(1),
        Intent(I::Advance(1)),
        Engage(fight.opponents[0].clone()),
        HostFight(fight.opponents[0].clone()),
    ] {
        assert!(engine.execute(command).is_err());
    }
    assert_eq!(*engine.state(), held, "a refused command changes nothing");
    assert!(engine.actions().iter().all(|a| !matches!(
        a.command,
        Rest | EngageRealtime(_) | Engage(_) | Travel(_) | Talk(_)
    )));

    let settlement = account(&engine, &fight, Outcome::Victory);
    let events = engine.execute(SettleFight(settlement.clone())).unwrap();
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    let combat = engine.state().combat.as_ref().unwrap();
    assert_eq!(combat.stance, Stance::Exploring);
    let settled = engine.state().clone();
    assert!(matches!(
        engine.execute(SettleFight(settlement)),
        Err(EngineError::NotFighting)
    ));
    assert_eq!(*engine.state(), settled, "nothing granted twice");
}

#[test]
fn a_settlement_rewards_as_the_engines_own_fight_does() {
    // Fought to the fall, so the victory has someone to reward.
    let mut world = world("practice-yard");
    for group in &mut world.world.combat.as_mut().unwrap().groups {
        group.yield_share = None;
    }
    let (mut hosted, fight) = begun(&world);
    let settled = account(&hosted, &fight, Outcome::Victory);
    hosted.execute(SettleFight(settled)).unwrap();

    let mut own = Engine::new(&world).unwrap();
    own.execute(EngageRealtime(foe(&own))).unwrap();
    let mut hits = 0;
    while own.realtime().is_some() {
        let player = world.world.player.clone();
        let _ = own.execute(Intent(I::Attack { target: None }));
        let _ = own.execute(Intent(I::Contact {
            attacker: player,
            target: None,
        }));
        if own.realtime().is_some() {
            own.execute(Intent(I::Advance(1))).unwrap();
        }
        hits += 1;
        assert!(hits < 100_000, "the fight never ended");
    }
    let (a, b) = (hosted.state(), own.state());
    let (ca, cb) = (a.combat.as_ref().unwrap(), b.combat.as_ref().unwrap());
    assert!(!ca.defeated.is_empty(), "someone fell");
    assert_eq!(ca.xp, cb.xp);
    assert_eq!(ca.defeated, cb.defeated);
    assert_eq!(a.player.inventory, b.player.inventory);
    assert_eq!(a.flags, b.flags);
    assert_eq!(a.quests, b.quests);
}

#[test]
fn an_account_the_world_could_not_have_is_refused_whole() {
    let world = world("practice-yard");
    let (mut engine, fight) = begun(&world);
    let fair = account(&engine, &fight, Outcome::Victory);
    let held = engine.state().clone();
    let bad = [
        FightSettlement {
            fight: fight.id + 1,
            ..fair.clone()
        },
        FightSettlement {
            fallen: vec!["nobody".into()],
            ..fair.clone()
        },
        FightSettlement {
            fallen: vec![fight.opponents[0].clone(), fight.opponents[0].clone()],
            ..fair.clone()
        },
        FightSettlement {
            worn: vec![9_999],
            ..fair.clone()
        },
        FightSettlement {
            vitals: Vitals {
                hp: 1_000_000,
                ..fair.vitals
            },
            ..fair.clone()
        },
        FightSettlement {
            vitals: Vitals {
                hp: 0,
                ..fair.vitals
            },
            ..fair.clone()
        },
        FightSettlement {
            companions: BTreeMap::from([(
                "nobody".into(),
                SettledCompanion {
                    vitals: None,
                    order: TacticalOrder::Hold,
                },
            )]),
            ..fair.clone()
        },
    ];
    for settlement in bad {
        let refused = engine.execute(SettleFight(settlement.clone()));
        assert!(
            matches!(refused, Err(EngineError::InvalidSettlement(_))),
            "{settlement:?}: {refused:?}"
        );
        assert_eq!(*engine.state(), held, "{settlement:?}");
    }
    engine.execute(SettleFight(fair)).unwrap();
}

#[test]
fn a_hosted_fight_survives_a_save_and_a_forged_one_does_not() {
    let world = world("practice-yard");
    let (engine, fight) = begun(&world);
    let snapshot = engine.snapshot();
    let bytes = snapshot.to_json();
    let restored = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    assert_eq!(restored.hosted_fight(), Some(&fight));

    for forge in [
        (|f: &mut HostedFight| f.opponents.clear()) as fn(&mut HostedFight),
        |f| f.opponents = vec!["nobody".into()],
        |f| f.id = u64::MAX,
    ] {
        let mut forged = snapshot.clone();
        let Stance::Hosted(fight) = &mut forged.state.combat.as_mut().unwrap().stance else {
            unreachable!("saved mid-fight");
        };
        forge(fight);
        assert!(Engine::restore(&world, forged).is_err());
    }
}

#[test]
fn a_companion_keeps_what_the_fight_left_it_and_attack_orders_lapse() {
    let mut world = world("hollow-ford");
    world.world.combat.as_mut().unwrap().groups[0].repeatable = true;
    let mut engine = Engine::new(&world).unwrap();
    let companion = world
        .characters
        .iter()
        .find(|c| c.companion.is_some())
        .unwrap()
        .id
        .clone();
    for command in [
        Talk("mara".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Talk("wren".into()),
        ChooseDialogue(1),
        Travel("crossroads".into()),
        Talk("oake".into()),
        ChooseDialogue(1),
        Travel("ford".into()),
    ] {
        engine.execute(command).unwrap();
    }
    assert!(engine.companion(&companion).is_some());
    let mut record = engine.companion(&companion).unwrap().clone();
    let whole = record.vitals;
    let foe_id = foe(&engine);
    engine.execute(HostFight(foe_id)).unwrap();
    let fight = engine.hosted_fight().unwrap().clone();
    assert!(
        engine
            .execute(Tactic {
                companion: companion.clone(),
                order: TacticalOrder::Hold
            })
            .is_err(),
        "the host holds orders while it runs the fight"
    );
    record.vitals.hp = whole.hp - 1;
    let mut settlement = account(&engine, &fight, Outcome::Victory);
    settlement.companions.insert(
        companion.clone(),
        SettledCompanion {
            vitals: Some(record.vitals),
            order: TacticalOrder::Attack {
                target: fight.opponents[0].clone(),
            },
        },
    );
    engine.execute(SettleFight(settlement)).unwrap();
    let after = engine.companion(&companion).unwrap();
    assert_eq!(after.vitals.hp, whole.hp - 1);
    assert_eq!(after.order, TacticalOrder::Follow, "attack orders lapse");
}

#[test]
fn the_players_base_stats_leave_out_what_they_wear() {
    let world = world("sword-yard");
    let mut engine = Engine::new(&world).unwrap();
    let base = engine.player_base_stats().unwrap();
    let combat = engine.state().combat.as_ref().unwrap();
    let level = world.combat().unwrap().levels[combat.level - 1].stats;
    assert_eq!(base, level);
    // Whatever is worn or taken off, the base stays.
    for id in combat.gear.keys().copied().collect::<Vec<_>>() {
        let _ = engine.execute(Equip(id));
        assert_eq!(engine.player_base_stats().unwrap(), base);
    }
}

#[test]
fn where_the_host_keeps_weapon_training_the_engine_never_writes_or_gates_it() {
    let mut world = world("sword-yard");
    let revision = world.revision();
    let combat = world.world.combat.as_mut().unwrap();
    let rules = combat.weapon_proficiency.as_mut().unwrap();
    assert!(!rules.hosted);
    rules.hosted = true;
    assert_ne!(world.revision(), revision, "the rule is part of the rules");
    let mut engine = Engine::new(&world).unwrap();
    let foe = foe(&engine);
    engine.execute(Engage(foe)).unwrap();
    for _ in 0..200 {
        let Some(attack) = engine
            .actions()
            .into_iter()
            .find(|a| matches!(a.command, Attack(_)))
        else {
            break;
        };
        engine.execute(attack.command).unwrap();
    }
    let combat = engine.state().combat.as_ref().unwrap();
    assert_eq!(combat.weapon_training, WeaponTraining::default());
    assert_eq!(engine.weapon_standing(), None);

    // Every skill a rank would gate is left for the host to judge.
    for skill in world
        .combat()
        .unwrap()
        .skills
        .iter()
        .filter(|s| s.mastery.is_some())
    {
        assert!(!matches!(
            engine.skill_usable(&skill.id),
            Err(EngineError::Unpracticed(_))
        ));
    }

    // A save holding training of the engine's own is refused here.
    let mut forged = engine.snapshot();
    let training = &mut forged.state.combat.as_mut().unwrap().weapon_training;
    training.families.insert("balanced_sword".into(), 1);
    training.disciplines.insert("one_handed".into(), 1);
    training.taught.insert("sparring_partner".into(), 1);
    assert!(Engine::restore(&world, forged).is_err());
}
