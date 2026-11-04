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
    for character in &mut bare.characters {
        if let Some(combat) = &mut character.combat {
            combat.weapon_challenge = None;
        }
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

/// What one hit on `foe` teaches, at character `level`, with `sword` XP in the
/// balanced sword and `item` in hand.
fn one_hit(world: &WorldSpec, level: usize, sword: u64, item: &str, foe: &str) -> u64 {
    let mut snapshot = Engine::new(world).unwrap().snapshot();
    let c = snapshot.state.combat.as_mut().unwrap();
    (c.level, c.xp) = (level, level as u64 - 1);
    if sword > 0 {
        let t = &mut c.weapon_training;
        t.families.insert("balanced_sword".into(), sword);
        t.disciplines.insert("one_handed".into(), sword);
        t.taught.insert("straw_man".into(), sword);
    }
    let mut engine = Engine::restore(world, snapshot).unwrap();
    take_up(&mut engine, item);
    fight(&mut engine, foe);
    let before = training(&engine).taught.values().sum::<u64>();
    hit(&mut engine);
    training(&engine).taught.values().sum::<u64>() - before
}

#[test]
fn training_follows_weapon_rank_and_foe_challenge_not_character_level() {
    let mut world = yard();
    rules(&mut world).xp.target_budget = 1000;
    let combat = world.world.combat.as_mut().unwrap();
    let first = combat.levels[0].clone();
    for xp in 1..=5 {
        combat.levels.push(Level {
            xp,
            ..first.clone()
        });
    }
    world.validate().unwrap();
    let sword = "plain_sword";
    // The recruit is level 1, so challenge 20 at this world's 20 per level.
    // A high-level novice still learns in full, where level falloff gave nothing.
    assert_eq!(one_hit(&world, 6, 0, sword, "recruit"), 10);
    // A level-1 expert does not: 20 past the challenge of a margin of 40, then all of it.
    assert_eq!(one_hit(&world, 1, 40, sword, "recruit"), 5);
    assert_eq!(one_hit(&world, 1, 60, sword, "recruit"), 0);
    // Level changes none of that.
    assert_eq!(one_hit(&world, 6, 40, sword, "recruit"), 5);
    // An authored challenge overrides the foe's level.
    assert_eq!(one_hit(&world, 1, 60, sword, "veteran"), 10);
    // Effective rank counts, so a master of a related family finds it easier
    // than a stranger: 36 borrowed is 16 past, 24/40 of 10. A spear knows nothing.
    assert_eq!(one_hit(&world, 1, 60, "rapier", "recruit"), 6);
    assert_eq!(one_hit(&world, 1, 60, "spear", "recruit"), 10);
}

#[test]
fn the_last_of_a_budget_is_partial_and_nothing_follows_it() {
    let world = yard();
    let mut snapshot = Engine::new(&world).unwrap().snapshot();
    let t = &mut snapshot.state.combat.as_mut().unwrap().weapon_training;
    t.families.insert("balanced_sword".into(), 55);
    t.disciplines.insert("one_handed".into(), 55);
    t.taught.insert("sparring_partner".into(), 55);
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    fight(&mut engine, "sparring_partner");
    let events = hit(&mut engine);
    assert!(events.contains(&Event::WeaponTrained {
        family: "balanced_sword".into(),
        amount: 5
    }));
    let events = hit(&mut engine);
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::WeaponTrained { .. })));
    assert_eq!(training(&engine).taught["sparring_partner"], 60);
}

#[test]
fn a_family_specific_skill_refuses_other_weapons_whatever_they_allow() {
    let mut world = yard();
    rules(&mut world).xp.target_budget = 1000;
    // The spear allows what the skill requires, and its rank would meet the gate.
    let spear = world.items.iter_mut().find(|i| i.id == "spear").unwrap();
    let gear = spear.equipment.as_mut().unwrap();
    gear.capabilities.push("blade".into());
    let mut engine = trained(&world, "spear", "polearm", 80);
    take_up(&mut engine, "spear");
    assert_eq!(engine.weapon_standing().unwrap().rank.direct, 80);
    let wrong_weapon = |r: Result<(), EngineError>| matches!(r, Err(EngineError::Unwielded(_)));
    assert!(wrong_weapon(engine.skill_usable("master_cut")));
    assert!(wrong_weapon(engine.skill_usable("feint")));
    // Nor by the menu, or in either kind of fight.
    let offered = |engine: &Engine<'_>, id: &str| {
        engine.actions().iter().any(|a| {
            a.available && matches!(&a.command, Intent(I::Skill { skill, .. }) if skill == id)
        })
    };
    fight(&mut engine, "sparring_partner");
    assert!(!offered(&engine, "master_cut"));
    let before = engine.snapshot();
    assert!(matches!(
        engine.execute(Intent(I::Skill {
            skill: "master_cut".into(),
            target: None
        })),
        Err(EngineError::Unwielded(_))
    ));
    assert_eq!(engine.snapshot(), before);
    leave(&mut engine);
    engine.execute(Engage("sparring_partner".into())).unwrap();
    assert!(matches!(
        engine.execute(UseSkill {
            skill: "master_cut".into(),
            target: "sparring_partner".into()
        }),
        Err(EngineError::Unwielded(_))
    ));

    // Its own family is fine, and bare hands are no family.
    let mut engine = trained(&world, "balanced_sword", "one_handed", 80);
    engine.skill_usable("master_cut").unwrap();
    let gear = &engine.state().combat.as_ref().unwrap().gear;
    let worn = *gear.iter().find(|(_, g)| g.equipped).unwrap().0;
    engine.execute(Unequip(worn)).unwrap();
    assert!(wrong_weapon(engine.skill_usable("master_cut")));
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
    // Both swords may make the cut here, so only its direct gate stands.
    let cut = combat.skills.iter_mut().find(|s| s.id == "master_cut");
    let mastery = cut.unwrap().mastery.as_mut().unwrap();
    mastery.families.push("thrusting_sword".into());
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

/// A yard whose `lunge` technique doubles the player's speed at its second
/// rank, which one use reaches.
fn fleet_yard() -> WorldSpec {
    let mut world = yard();
    let combat = world.world.combat.as_mut().unwrap();
    let shout = combat.skills.iter().find(|s| s.id == "shout").unwrap();
    let lunge = Skill {
        id: "lunge".into(),
        requires: vec!["blade".into()],
        mastery: None,
        ..shout.clone()
    };
    combat.skills.push(lunge);
    let rank = |xp, skill: Option<&str>, speed| TechniqueRank {
        name: "Rank".into(),
        xp,
        skill: skill.map(Into::into),
        passive: [(Stat::Speed, speed)].into_iter().collect(),
        requires: None,
    };
    combat.techniques.push(Technique {
        id: "fleet".into(),
        name: "Fleet".into(),
        ranks: vec![rank(0, Some("lunge"), 0), rank(1, Some("lunge"), 100)],
        xp_share_percent: 0,
    });
    combat.player_techniques.push(TechniqueGrant {
        technique: "fleet".into(),
        rank: None,
        xp: 0,
    });
    world.validate().unwrap();
    world
}

fn lunge(engine: &mut Engine<'_>) {
    engine
        .execute(Intent(I::Skill {
            skill: "lunge".into(),
            target: None,
        }))
        .unwrap();
}

#[test]
fn speed_gained_mid_attack_waits_for_the_next_attack() {
    let world = fleet_yard();
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    lunge(&mut engine);
    assert_eq!(to_active(&mut engine), 30);
    let slow = engine.player_stats().unwrap().speed;
    let events = contact(&mut engine);
    assert!(events.contains(&Event::TechniqueRankUp {
        technique: "fleet".into(),
        rank: 2
    }));
    assert_eq!(engine.player_stats().unwrap().speed, slow + 100);
    // Commitment holds through the save, the rest of its phases and the reload.
    let snapshot = serde_json::to_string(&engine.snapshot()).unwrap();
    let mut restored = Engine::restore(&world, serde_json::from_str(&snapshot).unwrap()).unwrap();
    for engine in [&mut engine, &mut restored] {
        assert_eq!(to_recovery(engine), 6);
        assert_eq!(to_ready(engine), 30, "the recovery it committed to");
    }
    assert_eq!(engine.snapshot(), restored.snapshot());
    // The next attack is the quicker one: half of each phase.
    lunge(&mut engine);
    assert_eq!(to_active(&mut engine), 15);
    assert_eq!(to_recovery(&mut engine), 3);
    assert_eq!(to_ready(&mut engine), 15);
}

#[test]
fn gear_cannot_change_while_an_attack_is_committed() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    let gear = &engine.state().combat.as_ref().unwrap().gear;
    let spear = *gear.iter().find(|(_, g)| g.item == "spear").unwrap().0;
    let before = engine.snapshot();
    assert!(matches!(
        engine.execute(Intent(I::Equip(spear))),
        Err(EngineError::Committed)
    ));
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn a_parry_of_a_blow_that_could_do_nothing_teaches_nothing() {
    let mut world = yard();
    let partner = world
        .characters
        .iter_mut()
        .find(|c| c.id == "sparring_partner");
    partner.unwrap().combat.as_mut().unwrap().stats.speed = 200;
    let sword = world.items.iter_mut().find(|i| i.id == "plain_sword");
    let gear = sword.unwrap().equipment.as_mut().unwrap();
    gear.modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    while !foe_striking(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    engine.execute(Intent(I::Parry)).unwrap();
    let events = foe_contact(&mut engine).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Parried { .. })));
    assert_eq!(training(&engine), &WeaponTraining::default());
}

#[test]
fn one_blow_teaches_a_defence_once_however_often_it_is_reported() {
    let mut world = yard();
    let partner = world
        .characters
        .iter_mut()
        .find(|c| c.id == "sparring_partner");
    partner.unwrap().combat.as_mut().unwrap().stats.speed = 200;
    let trained_by = |engine: &Engine<'_>| training(engine).taught["sparring_partner"];
    // Parried: the attacker reels, and a second report has nothing to land.
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    while !foe_striking(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    engine.execute(Intent(I::Parry)).unwrap();
    foe_contact(&mut engine).unwrap();
    assert_eq!(trained_by(&engine), 5);
    assert!(matches!(
        foe_contact(&mut engine),
        Err(EngineError::NoContact(_))
    ));
    assert_eq!(trained_by(&engine), 5);
    // Guarded: the attack has landed, once.
    let mut engine = Engine::new(&world).unwrap();
    fight(&mut engine, "sparring_partner");
    engine.execute(Intent(I::Guard)).unwrap();
    while !foe_striking(&engine) {
        engine.execute(Intent(I::Advance(1))).unwrap();
    }
    foe_contact(&mut engine).unwrap();
    assert_eq!(trained_by(&engine), 5);
    assert!(matches!(
        foe_contact(&mut engine),
        Err(EngineError::NoContact(_))
    ));
    assert_eq!(trained_by(&engine), 5);
}

fn foe_striking(engine: &Engine<'_>) -> bool {
    let Stance::Realtime(fight) = &engine.state().combat.as_ref().unwrap().stance else {
        panic!("not fighting");
    };
    matches!(
        fight.fighters[1].action,
        RealtimeAction::Attack {
            phase: AttackPhase::Active,
            connected: false,
            ..
        }
    )
}

fn foe_contact(engine: &mut Engine<'_>) -> Result<Vec<Event>, EngineError> {
    engine.execute(Intent(I::Contact {
        attacker: "sparring_partner".into(),
        target: None,
    }))
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
    // At speed 1 these phases last 600 and 3,000 steps at the most.
    for (bad_active, bad_recovery) in [(0, 5), (5, 0), (601, 5), (5, 3001)] {
        refused(&|c| {
            let Stance::Realtime(fight) = &mut c.stance else {
                panic!()
            };
            let RealtimeAction::Attack {
                active_steps,
                recovery_steps,
                ..
            } = &mut fight.fighters[0].action
            else {
                panic!()
            };
            (*active_steps, *recovery_steps) = (bad_active, bad_recovery);
        });
    }
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
    assert!(!json.contains("weapon_training"));
    Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
}
