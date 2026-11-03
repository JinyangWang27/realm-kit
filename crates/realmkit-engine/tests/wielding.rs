//! What the player wields decides which skills they can use: a skill that
//! `requires` capabilities is usable only while worn gear, or an empty
//! slot's bare hands, provides all of them, in real time and turn-based
//! fights alike, in menus, in `skill_usable` and in saves.

use realmkit_engine::{Command::*, Intent as I, *};
use realmkit_spec::*;

const RANGE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/archery-range");

/// The archery range, where a bow and a staff share the hands, with a staff
/// thrust, a bare-handed jab and an ordinary shout that needs nothing.
fn range() -> WorldSpec {
    let mut world = WorldSpec::load(RANGE).unwrap();
    let combat = world.world.combat.as_mut().unwrap();
    combat.bare.insert("hands".into(), vec!["fist".into()]);
    let shove = combat.skills[0].clone();
    for (id, requires) in [
        ("thrust", vec!["staff"]),
        ("jab", vec!["fist"]),
        ("shout", vec![]),
    ] {
        combat.skills.push(Skill {
            id: id.into(),
            requires: requires.into_iter().map(Into::into).collect(),
            ..shove.clone()
        });
        combat.player_skills.push(id.into());
    }
    world
}

fn piece(engine: &Engine<'_>, item: &str) -> u64 {
    let gear = &engine.state().combat.as_ref().unwrap().gear;
    *gear.iter().find(|(_, g)| g.item == item).unwrap().0
}

fn skill(id: &str) -> Command {
    Intent(I::Skill {
        skill: id.into(),
        target: None,
    })
}

fn unwielded(result: Result<Vec<Event>, EngineError>, id: &str) -> bool {
    matches!(result, Err(EngineError::Unwielded(s)) if s == id)
}

#[test]
fn a_skill_needs_what_it_requires_in_hand_or_bare_hands() {
    let world = range();
    let mut engine = Engine::new(&world).unwrap();
    // The bow is worn from the start: neither the staff nor a fist is in hand.
    assert!(engine.player_ranged().is_some());
    assert!(matches!(
        engine.skill_usable("thrust"),
        Err(EngineError::Unwielded(_))
    ));
    assert!(matches!(
        engine.skill_usable("jab"),
        Err(EngineError::Unwielded(_))
    ));
    engine
        .skill_usable("shout")
        .expect("a skill that requires nothing");
    engine.execute(EngageRealtime("brawler".into())).unwrap();
    let before = engine.snapshot();
    assert!(unwielded(engine.execute(skill("thrust")), "thrust"));
    assert!(unwielded(engine.execute(skill("jab")), "jab"));
    assert_eq!(engine.snapshot(), before, "a refused skill changes nothing");
    let offered = |engine: &Engine<'_>, id: &str| {
        engine.actions().iter().any(|a| {
            a.available && matches!(&a.command, Intent(I::Skill { skill, .. }) if skill == id)
        })
    };
    assert!(!offered(&engine, "thrust") && !offered(&engine, "jab"));
    assert!(offered(&engine, "shout"));

    // Taking up the staff allows the thrust, and still not the jab.
    let staff = piece(&engine, "staff");
    engine.execute(Intent(I::Equip(staff))).unwrap();
    engine.execute(Intent(I::Advance(3))).unwrap();
    engine.skill_usable("thrust").unwrap();
    assert!(offered(&engine, "thrust") && !offered(&engine, "jab"));
    engine.execute(skill("thrust")).unwrap();
}

#[test]
fn empty_hands_give_their_bare_capabilities() {
    let world = range();
    let mut engine = Engine::new(&world).unwrap();
    let bow = piece(&engine, "short_bow");
    engine.execute(Unequip(bow)).unwrap();
    engine.skill_usable("jab").unwrap();
    assert!(matches!(
        engine.skill_usable("thrust"),
        Err(EngineError::Unwielded(_))
    ));
    engine.execute(EngageRealtime("brawler".into())).unwrap();
    engine.execute(skill("jab")).unwrap();
}

#[test]
fn turn_based_fights_hold_the_same_rule() {
    let mut duel =
        WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel")).unwrap();
    let combat = duel.world.combat.as_mut().unwrap();
    let bolt = combat.skills.iter_mut().find(|s| s.id == "bolt").unwrap();
    bolt.requires = vec!["wand".into()];
    // A capability nothing provides is an authoring error.
    let codes: Vec<_> = duel.diagnostics().into_iter().map(|d| d.code).collect();
    assert!(codes.iter().any(|c| c == "missing_reference"), "{codes:?}");
    // With a wand in the world but not in hand, the engine refuses it where
    // the walkthrough uses it.
    duel.world.combat.as_mut().unwrap().slots = vec!["hands".into()];
    duel.items.push(Item {
        id: "wand".into(),
        name: "Wand".into(),
        description: "Willow.".into(),
        equipment: Some(Equipment {
            slots: vec!["hands".into()],
            capabilities: vec!["wand".into()],
            ..serde_json::from_str(r#"{"slots": []}"#).unwrap()
        }),
        consumable: None,
    });
    let mut engine = Engine::new(&duel).unwrap();
    assert!(matches!(
        engine.skill_usable("bolt"),
        Err(EngineError::Unwielded(_))
    ));
    engine.execute(Move(Direction::North)).unwrap();
    engine.execute(Engage("witch".into())).unwrap();
    let offered = engine
        .actions()
        .into_iter()
        .any(|a| a.available && matches!(&a.command, UseSkill { skill, .. } if skill == "bolt"));
    assert!(!offered);
    let used = engine.execute(UseSkill {
        skill: "bolt".into(),
        target: "witch".into(),
    });
    assert!(unwielded(used, "bolt"));
}

#[test]
fn a_bare_slot_must_be_a_declared_slot() {
    let mut world = range();
    let combat = world.world.combat.as_mut().unwrap();
    combat.bare.insert("tail".into(), vec!["sweep".into()]);
    let codes: Vec<_> = world.diagnostics().into_iter().map(|d| d.code).collect();
    assert!(codes.iter().any(|c| c == "missing_reference"), "{codes:?}");
    assert!(range().diagnostics().is_empty());
}

#[test]
fn a_save_never_holds_an_attack_with_a_skill_its_gear_forbids() {
    let world = range();
    let mut engine = Engine::new(&world).unwrap();
    let staff = piece(&engine, "staff");
    engine.execute(Equip(staff)).unwrap();
    engine.execute(EngageRealtime("brawler".into())).unwrap();
    engine.execute(skill("thrust")).unwrap();
    let good = engine.snapshot();
    Engine::restore(&world, good.clone()).expect("a thrust with the staff in hand");
    // A swap waits for the player to be free, so the bow cannot be in hand
    // while the thrust is still committed.
    let mut tampered = good;
    for gear in tampered.state.combat.as_mut().unwrap().gear.values_mut() {
        gear.equipped = gear.item == "short_bow";
    }
    assert!(Engine::restore(&world, tampered).is_err());
}
