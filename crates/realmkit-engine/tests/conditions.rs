//! Condition trees and effect lists: `all`, `any`, `not`, item conditions,
//! and choices whose effects apply in order or not at all.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn choices(engine: &Engine<'_>) -> Vec<String> {
    engine
        .dialogue_choices()
        .into_iter()
        .map(String::from)
        .collect()
}

fn at_the_copyist(world: &WorldSpec) -> Engine<'_> {
    let mut engine = reader(world);
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    engine
}

#[test]
fn a_not_condition_hides_a_choice_once_its_effects_have_run() {
    let world = archive();
    let mut engine = at_the_copyist(&world);
    assert_eq!(
        choices(&engine),
        [
            "Have you seen the river map?",
            "Could I borrow a pen?",
            "Nothing. Sorry to disturb you."
        ]
    );
    let events = engine.execute(ChooseDialogue(2)).unwrap();
    // Both effects, in authored order, then the next line.
    assert_eq!(
        events[..2],
        [
            Event::StoryFlagSet {
                flag: "pen_borrowed".into()
            },
            Event::ItemReceived {
                item: "pen".into(),
                quantity: 1
            }
        ]
    );
    assert_eq!(engine.state().player.inventory["pen"], 1);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());

    engine.execute(Talk("copyist".into())).unwrap();
    // Borrowed once, so only returning it is offered; the item condition holds.
    assert_eq!(
        choices(&engine),
        [
            "Have you seen the river map?",
            "Here is your pen back.",
            "Nothing. Sorry to disturb you."
        ]
    );
    let events = engine.execute(ChooseDialogue(2)).unwrap();
    assert!(events.contains(&Event::ItemsSpent {
        item: "pen".into(),
        quantity: 1
    }));
    assert!(!engine.state().player.inventory.contains_key("pen"));
    engine.execute(Talk("copyist".into())).unwrap();
    assert_eq!(choices(&engine).len(), 2);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn an_any_condition_holds_while_one_branch_does() {
    let world = archive();
    let mut engine = reader(&world);
    let question = "Is the vault always locked?".to_string();
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(!choices(&engine).contains(&question));
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(choices(&engine).contains(&question));
    // Ready is the other branch.
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(South)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(choices(&engine).contains(&question));
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(!choices(&engine).contains(&question));
}

#[test]
fn a_failing_effect_refuses_the_whole_choice() {
    let mut world = archive();
    let copyist = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "copyist")
        .unwrap();
    // Setting the flag comes first, but taking a pen nobody has fails.
    copyist.nodes[0].choices[1].effects = vec![
        Effect::SetFlag {
            flag: "pen_borrowed".into(),
        },
        Effect::TakeItems {
            items: vec![ItemStack {
                item: "pen".into(),
                quantity: 1,
            }],
        },
    ];
    let mut engine = at_the_copyist(&world);
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(ChooseDialogue(2)),
        Err(EngineError::NotEnoughMaterials(item)) if item == "pen"
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn conditions_compose_and_evaluate_without_changing_anything() {
    let world = archive();
    let mut engine = at_the_copyist(&world);
    engine.execute(ChooseDialogue(2)).unwrap();
    let flag = |flag: &str| Condition::Flag { flag: flag.into() };
    let pen = Condition::Item {
        item: "pen".into(),
        quantity: 1,
    };
    let before = engine.state().clone();
    assert!(engine.holds(&Condition::All {
        of: vec![flag("pen_borrowed"), pen.clone()]
    }));
    assert!(!engine.holds(&Condition::All {
        of: vec![flag("pen_borrowed"), flag("map_found")]
    }));
    assert!(engine.holds(&Condition::Any {
        of: vec![flag("map_found"), pen.clone()]
    }));
    assert!(engine.holds(&Condition::Not {
        condition: Box::new(flag("vault_open"))
    }));
    assert!(!engine.holds(&Condition::Item {
        item: "pen".into(),
        quantity: 2
    }));
    assert!(engine.allows(None));
    assert_eq!(engine.state(), &before);
}

#[test]
fn items_effects_hand_over_are_not_bound_to_quest_rewards_in_saves() {
    let world = archive();
    let mut engine = at_the_copyist(&world);
    engine.execute(ChooseDialogue(2)).unwrap();
    let mut snapshot = engine.snapshot();
    // A choice can run again under other content, so any count of a loose item loads.
    snapshot.state.player.inventory.insert("pen".into(), 3);
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    // Quest rewards are still bound: the key is granted only on completion.
    snapshot
        .state
        .player
        .inventory
        .insert("vault_key".into(), 1);
    assert!(matches!(
        Engine::restore(&world, snapshot),
        Err(EngineError::InvalidSave(_))
    ));
}

#[test]
fn an_item_that_effects_only_take_keeps_its_upper_bound_in_saves() {
    let mut world = archive();
    // Pell takes the key back; nothing but the quest ever grants it.
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices.insert(
        0,
        DialogueChoice {
            text: "Return the key.".into(),
            next: None,
            requires: Some(Condition::Item {
                item: "vault_key".into(),
                quantity: 1,
            }),
            effects: vec![Effect::TakeItems {
                items: vec![ItemStack {
                    item: "vault_key".into(),
                    quantity: 1,
                }],
            }],
        },
    );
    let engine = reader(&world);
    let mut forged = engine.snapshot();
    forged.state.player.inventory.insert("vault_key".into(), 5);
    assert!(matches!(
        Engine::restore(&world, forged),
        Err(EngineError::InvalidSave(_))
    ));
}

#[test]
fn at_least_holds_from_its_count_of_conditions() {
    let world = archive();
    let mut engine = at_the_copyist(&world);
    let flag = |flag: &str| Condition::Flag { flag: flag.into() };
    let not = |c| Condition::Not {
        condition: Box::new(c),
    };
    let of = vec![
        flag("pen_borrowed"),
        not(flag("vault_open")),
        flag("map_found"),
    ];
    let at_least = |count| Condition::AtLeast {
        count,
        of: of.clone(),
    };
    // Only the `not` holds at first.
    assert!(engine.holds(&at_least(1)));
    assert!(!engine.holds(&at_least(2)));
    engine.execute(ChooseDialogue(2)).unwrap();
    assert!(engine.holds(&at_least(2)));
    assert!(!engine.holds(&at_least(3)));
    // Nested under `not`, `all` and `any` like any condition.
    assert!(engine.holds(&not(at_least(3))));
    assert!(!engine.holds(&Condition::All {
        of: vec![at_least(2), at_least(3)]
    }));
    assert!(engine.holds(&Condition::Any {
        of: vec![at_least(3), at_least(2)]
    }));
}
