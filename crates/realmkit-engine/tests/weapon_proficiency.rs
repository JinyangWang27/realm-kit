//! Weapon proficiency at the sword yard: families share rank, related ones
//! borrow part of it, unrelated ones nothing; meaningful use teaches within a
//! foe's budget; a committed attack keeps the timing it began with.

use realmkit_engine::{Command::*, Intent as I, *};
use realmkit_spec::*;

const YARD: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sword-yard");

fn yard() -> WorldSpec {
    WorldSpec::load(YARD).unwrap()
}

fn rules(world: &mut WorldSpec) -> &mut WeaponProficiency {
    let combat = world.world.combat.as_mut().unwrap();
    combat.weapon_proficiency.as_mut().unwrap()
}

fn take_up(engine: &mut Engine<'_>, item: &str) {
    let gear = &engine.state().combat.as_ref().unwrap().gear;
    let id = *gear.iter().find(|(_, g)| g.item == item).unwrap().0;
    if !gear[&id].equipped {
        engine.execute(Equip(id)).unwrap();
    }
}

fn training<'a>(engine: &'a Engine<'_>) -> &'a WeaponTraining {
    &engine.state().combat.as_ref().unwrap().weapon_training
}

/// A playthrough restored with `family` and `discipline` at `xp`, as one foe
/// could have taught them.
fn trained<'w>(world: &'w WorldSpec, family: &str, discipline: &str, xp: u64) -> Engine<'w> {
    let engine = Engine::new(world).unwrap();
    let mut snapshot = engine.snapshot();
    let t = &mut snapshot.state.combat.as_mut().unwrap().weapon_training;
    t.families.insert(family.into(), xp);
    t.disciplines.insert(discipline.into(), xp);
    t.taught.insert("sparring_partner".into(), xp);
    Engine::restore(world, snapshot).unwrap()
}

fn player_action(engine: &Engine<'_>) -> RealtimeAction {
    let Stance::Realtime(fight) = &engine.state().combat.as_ref().unwrap().stance else {
        panic!("not fighting");
    };
    fight.fighters[0].action.clone()
}

fn step_of(engine: &Engine<'_>) -> u64 {
    let Stance::Realtime(fight) = &engine.state().combat.as_ref().unwrap().stance else {
        panic!("not fighting");
    };
    fight.step
}

/// Steps until the player's attack is striking; the steps it took.
fn to_active(engine: &mut Engine<'_>) -> u64 {
    let mut steps = 0;
    while !matches!(
        player_action(engine),
        RealtimeAction::Attack {
            phase: AttackPhase::Active,
            ..
        }
    ) {
        engine.execute(Intent(I::Advance(1))).unwrap();
        steps += 1;
    }
    steps
}

fn contact(engine: &mut Engine<'_>) -> Vec<Event> {
    engine
        .execute(Intent(I::Contact {
            attacker: "student".into(),
            target: None,
        }))
        .unwrap()
}

fn to_ready(engine: &mut Engine<'_>) -> u64 {
    let mut steps = 0;
    while player_action(engine) != RealtimeAction::Ready {
        engine.execute(Intent(I::Advance(1))).unwrap();
        steps += 1;
    }
    steps
}

/// One whole basic attack that lands; the events of its contact.
fn hit(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    to_active(engine);
    let events = contact(engine);
    to_ready(engine);
    events
}

fn fight(engine: &mut Engine<'_>, foe: &str) {
    engine.execute(EngageRealtime(foe.into())).unwrap();
}

fn leave(engine: &mut Engine<'_>) {
    engine.execute(Intent(I::Retreat)).unwrap();
    engine.execute(Intent(I::Advance(12))).unwrap();
    assert!(matches!(
        engine.state().combat.as_ref().unwrap().stance,
        Stance::Exploring
    ));
}

#[test]
fn two_compatible_swords_share_one_rank() {
    let world = yard();
    let mut engine = trained(&world, "balanced_sword", "one_handed", 60);
    let eastern = engine.weapon_standing().unwrap();
    assert_eq!(eastern.family, "balanced_sword");
    assert_eq!(eastern.rank.direct, 60);
    take_up(&mut engine, "arming_sword");
    assert_eq!(engine.weapon_standing().unwrap(), eastern);
}

#[test]
fn a_related_family_borrows_part_and_an_unrelated_one_nothing() {
    let world = yard();
    let mut engine = trained(&world, "balanced_sword", "one_handed", 60);
    take_up(&mut engine, "rapier");
    let rapier = engine.weapon_standing().unwrap();
    // 60% of direct 60.
    assert_eq!(
        rapier.rank,
        FamilyStanding {
            direct: 0,
            borrowed: 36,
            effective: 36
        }
    );
    take_up(&mut engine, "spear");
    let spear = engine.weapon_standing().unwrap();
    assert_eq!(spear.rank.effective, 0);
    assert_eq!(spear.handling, 0, "spear handling is not one-handed");
    assert_eq!((spear.windup_cut, spear.recovery_cut), (0, 0));
}

#[test]
fn direct_rank_always_helps_and_borrowing_is_bounded() {
    let mut world = yard();
    rules(&mut world).xp.target_budget = 1000;
    let engine = Engine::new(&world).unwrap();
    let mut last = 0;
    for xp in [0_u64, 20, 40, 60, 80, 100] {
        let mut snapshot = engine.snapshot();
        let t = &mut snapshot.state.combat.as_mut().unwrap().weapon_training;
        t.families.insert("balanced_sword".into(), 80);
        if xp > 0 {
            t.families.insert("thrusting_sword".into(), xp);
        }
        t.disciplines.insert("one_handed".into(), 80 + xp);
        t.taught.insert("sparring_partner".into(), 80 + xp);
        let e = Engine::restore(&world, snapshot).unwrap();
        let rank = e.family_standing("thrusting_sword").unwrap();
        assert_eq!(rank.borrowed, 48, "direct ranks of others only");
        assert_eq!(rank.effective, rank.direct + (100 - rank.direct) * 48 / 100);
        assert!(rank.effective >= last && rank.effective <= 100);
        last = rank.effective;
    }
    assert_eq!(last, 100);
}

#[test]
fn no_chains_and_no_stacking_of_donors() {
    let mut world = yard();
    let families = &mut rules(&mut world).families;
    families.push(WeaponFamily {
        id: "chained".into(),
        name: "Chained".into(),
        affinities: Default::default(),
    });
    // Spear → thrusting → chained, and both swords' donors → chained.
    families[1].affinities.insert("chained".into(), 50);
    families[2].affinities.insert("thrusting_sword".into(), 50);
    families[0].affinities.insert("chained".into(), 25);
    world.validate().unwrap();
    let engine = Engine::new(&world).unwrap();
    let mut snapshot = engine.snapshot();
    let t = &mut snapshot.state.combat.as_mut().unwrap().weapon_training;
    t.families.insert("balanced_sword".into(), 40);
    t.families.insert("spear".into(), 20);
    t.disciplines.insert("one_handed".into(), 60);
    t.taught.insert("sparring_partner".into(), 60);
    let e = Engine::restore(&world, snapshot).unwrap();
    // Thrusting sword borrows 24 (60% of sword 40), yet teaches nothing on:
    // chained takes the best direct donor, sword at 25% of 40, alone.
    assert_eq!(e.family_standing("thrusting_sword").unwrap().borrowed, 24);
    assert_eq!(e.family_standing("chained").unwrap().borrowed, 10);
}

#[test]
fn a_master_is_quicker_only_in_this_attack_and_rank_gained_waits_for_the_next() {
    let mut world = yard();
    rules(&mut world).xp.target_budget = 1000;
    // One hit short of rank 80 in both layers, from rank 60.
    let mut engine = trained(&world, "balanced_sword", "one_handed", 79);
    let speed = engine.player_stats().unwrap().speed;
    let before = engine.weapon_standing().unwrap();
    assert_eq!(before.rank.direct, 60);
    fight(&mut engine, "sparring_partner");
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    assert_eq!(to_active(&mut engine), 29, "30 steps less 6%");
    // This contact lifts the family and handling a whole rank.
    let events = contact(&mut engine);
    assert!(events.contains(&Event::WeaponRankUp {
        family: "balanced_sword".into(),
        rank: 80
    }));
    let after = engine.weapon_standing().unwrap();
    assert!(after.recovery_cut > before.recovery_cut);
    let active_left = to_recovery(&mut engine);
    assert_eq!(active_left, 6);
    // Recovery is the attack's own 9%, not the 12% it has just earned.
    assert_eq!(to_ready(&mut engine), 28);
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    assert_eq!(to_active(&mut engine), 28, "the next attack has the 8%");
    contact(&mut engine);
    to_recovery(&mut engine);
    assert_eq!(to_ready(&mut engine), 27, "and the 12%");
    assert_eq!(
        engine.player_stats().unwrap().speed,
        speed,
        "no global speed bonus"
    );
}

fn to_recovery(engine: &mut Engine<'_>) -> u64 {
    let mut steps = 0;
    while matches!(
        player_action(engine),
        RealtimeAction::Attack {
            phase: AttackPhase::Active,
            ..
        }
    ) {
        engine.execute(Intent(I::Advance(1))).unwrap();
        steps += 1;
    }
    steps
}

#[test]
fn a_novice_and_a_world_without_proficiency_keep_authored_timing() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    assert_eq!(to_active(&mut engine), 30);

    let mut bare = yard();
    let combat = bare.world.combat.as_mut().unwrap();
    combat.weapon_proficiency = None;
    for skill in &mut combat.skills {
        skill.mastery = None;
    }
    for item in &mut bare.items {
        item.equipment.as_mut().unwrap().weapon = None;
    }
    bare.validate().unwrap();
    let mut engine = Engine::new(&bare).unwrap();
    assert!(engine.weapon_standing().is_none());
    fight(&mut engine, "sparring_partner");
    let events = hit(&mut engine);
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::WeaponTrained { .. })));
    assert_eq!(training(&engine), &WeaponTraining::default());
}

#[test]
fn a_hit_trains_the_family_and_handling_by_weight_with_nothing_lost() {
    let mut world = yard();
    rules(&mut world).xp.hit = 7;
    let mut engine = Engine::new(&world).unwrap();
    take_up(&mut engine, "spear");
    fight(&mut engine, "sparring_partner");
    let events = hit(&mut engine);
    assert!(events.contains(&Event::WeaponTrained {
        family: "spear".into(),
        amount: 7
    }));
    let t = training(&engine);
    // 4.9 and 2.1: the spare point to the larger remainder.
    assert_eq!(t.disciplines["polearm"], 5);
    assert_eq!(t.disciplines["two_handed"], 2);
    assert_eq!(t.families["spear"], 7);
    assert!(!t.families.contains_key("balanced_sword"));
}

#[test]
fn only_meaningful_use_teaches() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    // A swing that never lands, and a yard dummy.
    fight(&mut engine, "sparring_partner");
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    to_active(&mut engine);
    to_ready(&mut engine);
    assert_eq!(training(&engine), &WeaponTraining::default());
    leave(&mut engine);
    fight(&mut engine, "straw_man");
    hit(&mut engine);
    assert_eq!(training(&engine), &WeaponTraining::default());
    leave(&mut engine);
    // A skill that needs no weapon, then bare hands.
    fight(&mut engine, "sparring_partner");
    engine
        .execute(Intent(I::Skill {
            skill: "shout".into(),
            target: None,
        }))
        .unwrap();
    to_active(&mut engine);
    contact(&mut engine);
    to_ready(&mut engine);
    assert_eq!(training(&engine), &WeaponTraining::default());
    leave(&mut engine);
    let worn = engine.state().combat.as_ref().unwrap().gear.iter();
    let id = worn.filter(|(_, g)| g.equipped).map(|(i, _)| *i).next();
    engine.execute(Unequip(id.unwrap())).unwrap();
    fight(&mut engine, "sparring_partner");
    assert!(engine.weapon_standing().is_none());
    hit(&mut engine);
    assert_eq!(training(&engine), &WeaponTraining::default());
}

#[test]
fn a_foe_teaches_within_its_budget_however_the_fights_restart() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    // Budget 60 at 10 a hit, across fights and a reload.
    for round in 0..8 {
        fight(&mut engine, "sparring_partner");
        hit(&mut engine);
        leave(&mut engine);
        if round == 3 {
            let snapshot = engine.snapshot();
            engine = Engine::restore(&world, snapshot).unwrap();
        }
    }
    assert_eq!(training(&engine).taught["sparring_partner"], 60);
    assert_eq!(training(&engine).families["balanced_sword"], 60);
    // Someone else still has something to teach.
    fight(&mut engine, "veteran");
    hit(&mut engine);
    assert_eq!(training(&engine).families["balanced_sword"], 70);
}

#[test]
fn much_weaker_foes_teach_less_and_then_nothing() {
    let mut world = yard();
    let combat = world.world.combat.as_mut().unwrap();
    let first = combat.levels[0].clone();
    for xp in 1..=5 {
        combat.levels.push(Level {
            xp,
            ..first.clone()
        });
    }
    world.validate().unwrap();
    let at_level = |level: usize| {
        let mut snapshot = Engine::new(&world).unwrap().snapshot();
        let c = snapshot.state.combat.as_mut().unwrap();
        (c.level, c.xp) = (level, level as u64 - 1);
        let mut engine = Engine::restore(&world, snapshot).unwrap();
        fight(&mut engine, "sparring_partner");
        hit(&mut engine);
        training(&engine).families.get("balanced_sword").copied()
    };
    // One level above the foe, 90%; five above, nothing at all.
    assert_eq!(at_level(2), Some(9));
    assert_eq!(at_level(6), None);
}

#[test]
fn a_parry_and_a_raised_guard_teach_but_a_miss_does_not() {
    let mut world = yard();
    let partner = world
        .characters
        .iter_mut()
        .find(|c| c.id == "sparring_partner");
    partner.unwrap().combat.as_mut().unwrap().stats.speed = 200;
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    let foe_active = |e: &Engine<'_>| {
        let Stance::Realtime(f) = &e.state().combat.as_ref().unwrap().stance else {
            panic!()
        };
        matches!(
            f.fighters[1].action,
            RealtimeAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            }
        )
    };
    while !foe_active(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    engine.execute(Intent(I::Parry)).unwrap();
    let events = engine
        .execute(Intent(I::Contact {
            attacker: "sparring_partner".into(),
            target: None,
        }))
        .unwrap();
    assert!(events.contains(&Event::WeaponTrained {
        family: "balanced_sword".into(),
        amount: 5
    }));
    // A blow on open ground teaches the one it hits nothing.
    to_ready(&mut engine);
    while !foe_active(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    let events = engine
        .execute(Intent(I::Contact {
            attacker: "sparring_partner".into(),
            target: None,
        }))
        .unwrap();
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::WeaponTrained { .. })));
    // A raised guard stops the next.
    engine.execute(Intent(I::Guard)).unwrap();
    engine.execute(Intent(I::Advance(2))).unwrap();
    while !foe_active(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    let events = engine
        .execute(Intent(I::Contact {
            attacker: "sparring_partner".into(),
            target: None,
        }))
        .unwrap();
    assert!(events.contains(&Event::WeaponTrained {
        family: "balanced_sword".into(),
        amount: 5
    }));
    assert_eq!(training(&engine).families["balanced_sword"], 10);
}

#[test]
fn a_guard_that_prevents_no_damage_teaches_nothing() {
    let quick = |world: &mut WorldSpec| {
        let partner = world
            .characters
            .iter_mut()
            .find(|c| c.id == "sparring_partner");
        partner.unwrap().combat.as_mut().unwrap().stats.speed = 200;
    };
    // A guard that lets everything through.
    let mut leaky = yard();
    quick(&mut leaky);
    let combat = leaky.world.combat.as_mut().unwrap();
    combat.realtime.as_mut().unwrap().guard_percent = 100;
    // Worn immunity: nothing reaches the guard to stop.
    let mut immune = yard();
    quick(&mut immune);
    let sword = immune.items.iter_mut().find(|i| i.id == "plain_sword");
    let gear = sword.unwrap().equipment.as_mut().unwrap();
    gear.modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    for world in [&leaky, &immune] {
        world.validate().unwrap();
        let mut engine = Engine::new(world).unwrap();
        fight(&mut engine, "sparring_partner");
        engine.execute(Intent(I::Guard)).unwrap();
        engine.execute(Intent(I::Advance(2))).unwrap();
        let foe_active = |e: &Engine<'_>| {
            let Stance::Realtime(f) = &e.state().combat.as_ref().unwrap().stance else {
                panic!()
            };
            matches!(
                f.fighters[1].action,
                RealtimeAction::Attack {
                    phase: AttackPhase::Active,
                    connected: false,
                    ..
                }
            )
        };
        while !foe_active(&engine) {
            engine.execute(Intent(I::Advance(1))).unwrap();
        }
        engine
            .execute(Intent(I::Contact {
                attacker: "sparring_partner".into(),
                target: None,
            }))
            .unwrap();
        assert_eq!(training(&engine), &WeaponTraining::default());
    }
}

#[test]
fn borrowed_rank_opens_everyday_moves_and_not_direct_ones_or_unlearned_ones() {
    let mut world = yard();
    rules(&mut world).xp.target_budget = 1000;
    let combat = world.world.combat.as_mut().unwrap();
    combat.player_skills.retain(|s| s != "shout");
    let mut engine = trained(&world, "balanced_sword", "one_handed", 80);
    // A sword at direct 80 knows and may use both.
    engine.skill_usable("feint").unwrap();
    engine.skill_usable("master_cut").unwrap();
    take_up(&mut engine, "rapier");
    // Effective 48 from borrowing: the feint yes, the direct gate no.
    engine.skill_usable("feint").unwrap();
    assert!(matches!(
        engine.skill_usable("master_cut"),
        Err(EngineError::Unpracticed(_))
    ));
    fight(&mut engine, "sparring_partner");
    let before = engine.snapshot();
    assert!(matches!(
        engine.execute(Intent(I::Skill {
            skill: "master_cut".into(),
            target: None
        })),
        Err(EngineError::Unpracticed(_))
    ));
    assert_eq!(engine.snapshot(), before, "a refused skill changes nothing");
    // Rank teaches no move: one the player never learned stays unknown.
    assert!(matches!(
        engine.skill_usable("shout"),
        Err(EngineError::UnknownSkill(_))
    ));
}

#[test]
fn the_turn_based_layer_trains_alike_and_keeps_its_own_clock() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Engage("sparring_partner".into())).unwrap();
    let events = engine.execute(Attack("sparring_partner".into())).unwrap();
    assert!(events.contains(&Event::WeaponTrained {
        family: "balanced_sword".into(),
        amount: 10
    }));
}

#[test]
fn saves_and_replays_keep_a_commitment_and_refuse_impossible_training() {
    let world = yard();
    let script = |engine: &mut Engine<'_>| {
        fight(engine, "sparring_partner");
        hit(engine);
        hit(engine);
        engine.execute(Intent(I::Attack { target: None })).unwrap();
        to_active(engine);
        contact(engine);
    };
    let mut first = Engine::new(&world).unwrap();
    script(&mut first);
    let mut second = Engine::new(&world).unwrap();
    script(&mut second);
    assert_eq!(first.snapshot(), second.snapshot(), "replays are identical");

    // Saved mid-attack, it finishes as the unsaved one does.
    let json = serde_json::to_string(&first.snapshot()).unwrap();
    let mut restored = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    to_ready(&mut first);
    to_ready(&mut restored);
    assert_eq!(first.snapshot(), restored.snapshot());
    assert_eq!(step_of(&first), step_of(&restored));

    let refused = |edit: &dyn Fn(&mut CombatState)| {
        let mut snapshot = second.snapshot();
        edit(snapshot.state.combat.as_mut().unwrap());
        assert!(Engine::restore(&world, snapshot).is_err());
    };
    refused(&|c| {
        *c.weapon_training
            .families
            .get_mut("balanced_sword")
            .unwrap() += 1
    });
    refused(&|c| {
        c.weapon_training
            .taught
            .insert("sparring_partner".into(), 61);
    });
    refused(&|c| {
        c.weapon_training.families.insert("axe".into(), 1);
        c.weapon_training
            .disciplines
            .insert("one_handed".into(), 31);
        c.weapon_training
            .taught
            .insert("sparring_partner".into(), 31);
    });
    refused(&|c| {
        let Stance::Realtime(fight) = &mut c.stance else {
            panic!()
        };
        let RealtimeAction::Attack { recovery_cut, .. } = &mut fight.fighters[0].action else {
            panic!()
        };
        *recovery_cut = 15;
    });
}

#[test]
fn a_save_from_before_proficiency_still_loads_where_the_world_has_none() {
    let world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/fencing-hall"
    ))
    .unwrap();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(EngageRealtime("duelist".into())).unwrap();
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    let json = serde_json::to_string(&engine.snapshot()).unwrap();
    assert!(!json.contains("weapon_training") && !json.contains("recovery_cut"));
    Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
}
