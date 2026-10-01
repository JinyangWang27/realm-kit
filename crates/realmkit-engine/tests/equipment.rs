//! Individual pieces worn in slots: bonuses, penalties and modifiers.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn pieces(engine: &Engine<'_>) -> Vec<(u64, String, bool)> {
    combat(engine)
        .gear
        .iter()
        .map(|(id, g)| (*id, g.item.clone(), g.equipped))
        .collect()
}

fn item<'w>(world: &'w mut WorldSpec, id: &str) -> &'w mut Equipment {
    world
        .items
        .iter_mut()
        .find(|i| i.id == id)
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
}

/// Clears the den and returns the pieces the wolves dropped.
fn beat_the_pack(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    let events = fight_out(engine);
    engine.execute(Move(South)).unwrap();
    events
}

#[test]
fn starting_gear_is_worn_and_counts_in_effective_stats() {
    let world = arena();
    let engine = Engine::new(&world).unwrap();
    assert_eq!(
        pieces(&engine),
        [
            (1, "practice_sword".into(), true),
            (2, "buckler".into(), true),
            (3, "leather_vest".into(), true),
        ]
    );
    let stats = engine.player_stats().unwrap();
    assert_eq!((stats.patk, stats.pdef), (12 + 2, 8 + 2 + 2));
}

#[test]
fn loot_arrives_as_individual_pieces_and_two_hands_displace_both() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    let events = beat_the_pack(&mut engine);
    assert!(events.contains(&Event::ItemReceived {
        item: "greatsword".into(),
        quantity: 1
    }));
    assert!(!engine.state().player.inventory.contains_key("greatsword"));
    assert_eq!(
        &pieces(&engine)[3..],
        [
            (4, "charm_of_warding".into(), false),
            (5, "greatsword".into(), false)
        ]
    );
    // The pack's pieces are offered to wear.
    assert!(offered(&engine).contains(&(Equip(5), true)));
    let events = engine.execute(Equip(5)).unwrap();
    assert_eq!(
        events,
        [
            Event::Unequipped { gear: 1 },
            Event::Unequipped { gear: 2 },
            Event::Equipped { gear: 5 }
        ]
    );
    let stats = engine.player_stats().unwrap();
    // Level 2 after the pack (13 attack, 9 defence), greatsword on, buckler off.
    assert_eq!((stats.patk, stats.pdef), (13 + 8, 9 + 2));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn a_heavy_weapon_slows_the_basic_attack_that_uses_it() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment = vec!["greatsword".into()];
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    let before = engine.encounter().unwrap().participants[0].next_time;
    engine.execute(Attack("holt".into())).unwrap();
    // 130% of a basic action at speed 100: 1,300 ticks instead of 1,000.
    let after = engine.encounter().unwrap().participants[0].next_time;
    assert_eq!(after - before, 1_300);
}

#[test]
fn armour_speed_penalties_add_up_and_leave_at_least_1() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment =
        vec!["iron_mail".into(), "practice_sword".into()];
    let engine = Engine::new(&world).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 90);
    item(&mut world, "practice_sword").speed_penalty = 500;
    let engine = Engine::new(&world).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 1);
}

#[test]
fn worn_modifiers_multiply_and_immunity_wins() {
    let stats = |patk, satk| Stats {
        hp: 1,
        mp: 0,
        patk,
        pdef: 0,
        satk,
        sdef: 0,
        speed: 100,
    };
    let (a, d) = (stats(40, 20), stats(0, 0));
    // 45 normally; halved to 22; doubled to 90; immunity deals nothing, not 1.
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (1, 1)).unwrap(),
        45
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (1, 2)).unwrap(),
        22
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (2, 1)).unwrap(),
        90
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (0, 1)).unwrap(),
        0
    );
    // Worn: a physical ward and immunity on another piece means no bite lands.
    let mut world = arena();
    let charm = item(&mut world, "charm_of_warding");
    charm
        .modifiers
        .insert(Channel::Physical, Modifier { num: 1, den: 2 });
    item(&mut world, "buckler")
        .modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .player_equipment
        .push("charm_of_warding".into());
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    let events = engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .all(|e| matches!(e, Event::DamageReceived { amount: 0, .. })));
    assert_eq!(vitals(&engine).hp, 60);
}

#[test]
fn equipping_is_refused_atomically_where_it_cannot_happen() {
    let mut world = arena();
    item(&mut world, "leather_vest")
        .bonuses
        .insert(Stat::Hp, 10);
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(vitals(&engine).hp, 70);
    let before = engine.state().clone();
    for (command, error) in [
        (Equip(9), "you have no equipment #9"),
        (Equip(1), "#1 is already equipped"),
        (Unequip(9), "you have no equipment #9"),
    ] {
        assert_eq!(engine.execute(command).unwrap_err().to_string(), error);
        assert_eq!(engine.state(), &before);
    }
    // Taking the vest off takes its HP with it.
    engine.execute(Unequip(3)).unwrap();
    assert_eq!(vitals(&engine).hp, 60);
    assert_eq!(
        engine.execute(Unequip(3)).unwrap_err().to_string(),
        "#3 is not equipped"
    );
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    assert!(matches!(
        engine.execute(Equip(3)),
        Err(EngineError::InEncounter)
    ));
}

#[test]
fn saves_reject_equipment_the_rules_could_not_produce() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    beat_the_pack(&mut engine);
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        // An ID at or past the counter.
        |s| {
            let c = s.state.combat.as_mut().unwrap();
            let piece = c.gear.remove(&5).unwrap();
            c.gear.insert(6, piece);
        },
        // Not equipment at all.
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&4)
                .unwrap()
                .item = "rat_tail".into()
        },
        // Two pieces in the same slot.
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&5)
                .unwrap()
                .equipped = true
        },
        // A piece nothing granted.
        |s| {
            let c = s.state.combat.as_mut().unwrap();
            c.gear.insert(
                6,
                Gear {
                    item: "iron_mail".into(),
                    equipped: false,
                    tier: 0,
                    enchantment: None,
                },
            );
            c.next_gear = 7;
        },
        // A starting piece gone.
        |s| {
            s.state.combat.as_mut().unwrap().gear.remove(&2);
        },
        // Equipment held as a count.
        |s| {
            s.state.combat.as_mut().unwrap().gear.remove(&5);
            s.state.player.inventory.insert("greatsword".into(), 1);
        },
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(&mut snapshot);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn the_projection_uses_a_heavy_weapons_basic_time() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment = vec!["greatsword".into()];
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    // Both open at 1,000; the greatsword's 130% puts the player's next turn
    // at 2,300, after Holt's at 2,000.
    assert_eq!(engine.turn_order(4), ["fighter", "holt", "holt", "fighter"]);
}

#[test]
fn a_save_wearing_an_unknown_item_is_rejected_not_a_crash() {
    let world = arena();
    let engine = Engine::new(&world).unwrap();
    for item in ["missing", "rat_tail"] {
        let mut snapshot = engine.snapshot();
        let piece = snapshot
            .state
            .combat
            .as_mut()
            .unwrap()
            .gear
            .get_mut(&1)
            .unwrap();
        piece.item = item.into();
        assert!(matches!(
            Engine::restore(&world, snapshot),
            Err(EngineError::InvalidSave(_))
        ));
    }
}

#[test]
fn many_worn_modifiers_combine_exactly() {
    // 28 more slots (32 in all) of alternating 10/9 and 9/10 wards multiply
    // to exactly 1, though each product alone overflows 64 bits.
    let warded = |modify: bool| {
        let mut world = arena();
        let slots: Vec<String> = (0..28).map(|i| format!("s{i}")).collect();
        let combat = world.world.combat.as_mut().unwrap();
        combat.slots.extend(slots.iter().cloned());
        combat.player_equipment = slots.iter().map(|s| format!("ward_{s}")).collect();
        for (i, slot) in slots.iter().enumerate() {
            let (num, den) = if i % 2 == 0 { (10, 9) } else { (9, 10) };
            let mut modifiers = std::collections::BTreeMap::new();
            if modify {
                modifiers.insert(Channel::Physical, Modifier { num, den });
            }
            world.items.push(Item {
                id: format!("ward_{slot}"),
                name: "Ward".into(),
                description: "A ward.".into(),
                equipment: Some(Equipment {
                    slots: vec![slot.clone()],
                    bonuses: Default::default(),
                    speed_penalty: 0,
                    basic_channel: None,
                    basic_time: None,
                    modifiers,
                    tiers: Vec::new(),
                }),
                consumable: None,
            });
        }
        world
    };
    let bites = |world: &WorldSpec| {
        let mut engine = Engine::new(world).unwrap();
        engine.execute(Move(North)).unwrap();
        engine.execute(Engage("grey_wolf".into())).unwrap()
    };
    assert_eq!(bites(&warded(true)), bites(&warded(false)));
}
