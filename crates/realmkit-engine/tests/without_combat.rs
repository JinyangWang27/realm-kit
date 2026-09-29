//! A world without a combat block plays its story through dialogue and flags.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

/// Learns where the map is and returns to the reading room.
fn find_map(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(South)).unwrap();
    events
}

fn accept_map(engine: &mut Engine<'_>) {
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
}

#[test]
fn a_world_without_combat_plays_its_main_quest_by_talking() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(engine.state().combat, None);
    assert!(!engine.is_dead());
    accept_map(&mut engine);
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Active);
    let events = find_map(&mut engine);
    assert!(events.contains(&Event::QuestProgressed {
        quest: "lost_map".into()
    }));
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Ready);
    engine.execute(Talk("archivist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::QuestCompleted {
        quest: "lost_map".into()
    }));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::ExperienceGranted { .. })));
    engine.execute(Move(East)).unwrap();
    assert_eq!(engine.state().player.location, "vault");
    assert_eq!(engine.state().player.inventory["vault_key"], 1);
    assert_eq!(engine.state().combat, None);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn a_world_without_combat_offers_no_attack_and_refuses_one() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    assert_eq!(
        offered(&engine),
        vec![
            (Talk("copyist".into()), true),
            (Move(South), true),
            (Inventory, true),
            (Status, true),
            (Quests, true),
        ]
    );
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Attack("copyist".into())),
        Err(EngineError::NotFighting)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn a_flag_set_before_accepting_readies_the_quest_on_acceptance() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    find_map(&mut engine);
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Available);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::QuestProgressed {
        quest: "lost_map".into()
    }));
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Ready);
}

#[test]
fn a_world_without_combat_rejects_inconsistent_saves() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    accept_map(&mut engine);
    let good = engine.snapshot();
    let json = serde_json::to_string(&good).unwrap();
    let resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(resumed.state(), engine.state());
    let mut fighting = good.clone();
    fighting.state.combat = Engine::new(&demo()).unwrap().state().combat.clone();
    let mut ready = good.clone();
    ready
        .state
        .quests
        .insert("lost_map".into(), QuestStatus::Ready);
    let mut found = good.clone();
    found.state.flags.insert("map_found".into());
    for (i, snapshot) in [fighting, ready, found].into_iter().enumerate() {
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
fn a_defeated_character_can_no_longer_be_talked_to() {
    let mut world = demo();
    world.characters[2].dialogue = Some("mara".into());
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    assert_eq!(offered(&engine)[0], (Talk("wolf".into()), true));
    engine.execute(Engage("wolf".into())).unwrap();
    for _ in 0..3 {
        engine.execute(Attack("wolf".into())).unwrap();
    }
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Talk(_))));
    assert!(matches!(
        engine.execute(Talk("wolf".into())),
        Err(EngineError::NotHere(_))
    ));
}
