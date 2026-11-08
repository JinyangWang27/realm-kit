//! Carried mass: derived from counted items and owned pieces, never saved;
//! and dropping a piece, the one way to stop owning it.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

/// The arena with a mass on every item, in grams.
fn weighed() -> WorldSpec {
    let mut world = arena();
    for item in &mut world.items {
        item.mass_g = Some(match item.id.as_str() {
            "rat_tail" => 30,
            "healing_draught" => 250,
            "practice_sword" => 900,
            "buckler" => 2_000,
            "greatsword" => 3_000,
            "leather_vest" => 2_500,
            "iron_mail" => 9_000,
            "charm_of_warding" => 20,
            other => panic!("unweighed {other}"),
        });
    }
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    world
}

/// Starting gear: sword 900 + buckler 2000 + vest 2500.
const STARTING: u64 = 5_400;

fn mass(engine: &Engine<'_>) -> u64 {
    engine.carried_mass().unwrap()
}

fn piece_of(engine: &Engine<'_>, item: &str) -> u64 {
    *combat(engine)
        .gear
        .iter()
        .find(|(_, g)| g.item == item)
        .unwrap()
        .0
}

/// Clears the den and walks back to the gate.
fn beat_the_pack(engine: &mut Engine<'_>) {
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    fight_out(engine);
    engine.execute(Move(South)).unwrap();
}

#[test]
fn unweighed_items_add_nothing() {
    let world = arena();
    let engine = Engine::new(&world).unwrap();
    assert_eq!(mass(&engine), 0);
}

#[test]
fn worn_and_packed_pieces_each_count_once() {
    let world = weighed();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(mass(&engine), STARTING);
    let sword = piece_of(&engine, "practice_sword");
    // Into the pack and back: the same things are carried.
    engine.execute(Unequip(sword)).unwrap();
    assert_eq!(mass(&engine), STARTING);
    engine.execute(Equip(sword)).unwrap();
    assert_eq!(mass(&engine), STARTING);
}

#[test]
fn counted_items_weigh_by_quantity_and_leave_when_used() {
    let world = weighed();
    let mut engine = Engine::new(&world).unwrap();
    engine
        .execute(Buy {
            good: "healing_draught".into(),
            quantity: 1,
        })
        .unwrap();
    assert_eq!(mass(&engine), STARTING + 250);
    // A stack weighs its quantity times one unit.
    let mut stacked = engine.snapshot();
    stacked
        .state
        .player
        .inventory
        .insert("healing_draught".into(), 4);
    let stacked = Engine::restore(&world, stacked).unwrap();
    assert_eq!(mass(&stacked), STARTING + 4 * 250);
    // Drinking one spends it, and its weight.
    let mut hurt = engine.snapshot();
    hurt.state.combat.as_mut().unwrap().vitals.hp = 1;
    let mut hurt = Engine::restore(&world, hurt).unwrap();
    hurt.execute(Use("healing_draught".into())).unwrap();
    assert!(!hurt
        .state()
        .player
        .inventory
        .contains_key("healing_draught"));
    assert_eq!(mass(&hurt), STARTING);
}

#[test]
fn a_load_too_heavy_to_count_is_refused_on_load() {
    let world = weighed();
    let mut snapshot = Engine::new(&world).unwrap().snapshot();
    snapshot
        .state
        .player
        .inventory
        .insert("healing_draught".into(), u64::MAX / 100);
    match Engine::restore(&world, snapshot) {
        Err(EngineError::InvalidSave(why)) => assert!(why.contains("carried mass"), "{why}"),
        other => panic!("{:?}", other.map(|_| ())),
    }
}

#[test]
fn loot_adds_its_pieces_and_dropping_one_takes_it_away() {
    let world = weighed();
    let mut engine = Engine::new(&world).unwrap();
    beat_the_pack(&mut engine);
    let looted = mass(&engine);
    assert!(looted >= STARTING + 3_000, "{looted}");
    let greatsword = piece_of(&engine, "greatsword");
    let events = engine.execute(Drop(greatsword)).unwrap();
    assert!(events.contains(&Event::Dropped {
        gear: greatsword,
        item: "greatsword".into()
    }));
    assert_eq!(mass(&engine), looted - 3_000);
    assert!(!combat(&engine).gear.contains_key(&greatsword));
    let dropped = &combat(&engine).dropped[&greatsword];
    assert_eq!(
        (dropped.item.as_str(), dropped.equipped),
        ("greatsword", false)
    );
    // Gone for good: it cannot be dropped or worn again.
    assert!(
        matches!(engine.execute(Drop(greatsword)), Err(EngineError::NoSuchGear(id)) if id == greatsword)
    );
    assert!(
        matches!(engine.execute(Equip(greatsword)), Err(EngineError::NoSuchGear(id)) if id == greatsword)
    );
    // The save keeps the drop, and loads with the same load.
    let restored = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(mass(&restored), mass(&engine));
    assert_eq!(restored.state(), engine.state());
}

#[test]
fn a_worn_piece_or_one_in_a_fight_is_not_dropped() {
    let world = weighed();
    let mut engine = Engine::new(&world).unwrap();
    let sword = piece_of(&engine, "practice_sword");
    assert!(matches!(engine.execute(Drop(sword)), Err(EngineError::DropWorn(id)) if id == sword));
    engine.execute(Unequip(sword)).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(matches!(
        engine.execute(Drop(sword)),
        Err(EngineError::InEncounter)
    ));
    assert_eq!(mass(&engine), STARTING);
}

#[test]
fn the_same_commands_give_the_same_load() {
    let world = weighed();
    let play = || {
        let mut engine = Engine::new_with_seed(&world, 7).unwrap();
        beat_the_pack(&mut engine);
        let greatsword = piece_of(&engine, "greatsword");
        engine.execute(Drop(greatsword)).unwrap();
        (mass(&engine), engine.snapshot())
    };
    let (first, second) = (play(), play());
    assert_eq!(first.0, second.0);
    assert_eq!(
        serde_json::to_string(&first.1).unwrap(),
        serde_json::to_string(&second.1).unwrap()
    );
}

#[test]
fn saves_with_impossible_drops_or_loads_are_refused() {
    let world = weighed();
    let mut engine = Engine::new(&world).unwrap();
    let sword = piece_of(&engine, "practice_sword");
    engine.execute(Unequip(sword)).unwrap();
    engine.execute(Drop(sword)).unwrap();
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        // An ID never issued.
        |s| {
            let combat = s.state.combat.as_mut().unwrap();
            let mut piece = combat.dropped.values().next().unwrap().clone();
            piece.item = "buckler".into();
            combat.dropped.insert(combat.next_gear, piece);
        },
        // The ID of a piece still owned.
        |s| {
            let combat = s.state.combat.as_mut().unwrap();
            let owned = *combat.gear.keys().next().unwrap();
            let piece = combat.gear[&owned].clone();
            combat.dropped.insert(owned, piece);
        },
        // A drop of something that is not equipment.
        |s| {
            let combat = s.state.combat.as_mut().unwrap();
            combat
                .dropped
                .values_mut()
                .for_each(|p| p.item = "rat_tail".into());
        },
        // A piece dropped while worn, or at a tier it does not have.
        |s| {
            let combat = s.state.combat.as_mut().unwrap();
            combat.dropped.values_mut().for_each(|p| p.equipped = true);
        },
        |s| {
            let combat = s.state.combat.as_mut().unwrap();
            combat.dropped.values_mut().for_each(|p| p.tier = 9);
        },
        // The drop forgotten: a granted piece is missing.
        |s| {
            s.state.combat.as_mut().unwrap().dropped.clear();
        },
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(&mut snapshot);
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
fn item_masses_are_validated() {
    for bad in [0, MASS_BOUND + 1] {
        let mut world = weighed();
        world.items[0].mass_g = Some(bad);
        let codes: Vec<_> = world.diagnostics().into_iter().map(|d| d.code).collect();
        assert_eq!(codes, ["invalid_mass"], "{bad}");
    }
    let mut world = weighed();
    world.items[0].mass_g = Some(MASS_BOUND);
    assert!(world.diagnostics().is_empty());
}
