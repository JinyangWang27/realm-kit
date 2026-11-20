//! The `move_player` effect: a dialogue choice may move the player to an
//! authored place; nothing else may.
mod common;

use common::*;
use realmkit_spec::*;

fn move_player(to: &str) -> Effect {
    Effect::MovePlayer { to: to.into() }
}

/// The demo with `effect` on Mara's first greeting choice.
fn demo_choosing(effect: Effect) -> WorldSpec {
    let mut world = demo();
    world.dialogues[0].nodes[0].choices[0].effects = vec![effect];
    world
}

#[test]
fn the_effect_reads_from_json_by_its_kind() {
    let effect: Effect = serde_json::from_str(r#"{"kind":"move_player","to":"ruins"}"#).unwrap();
    assert_eq!(effect, move_player("ruins"));
}

#[test]
fn a_dialogue_choice_may_move_the_player_to_an_authored_place() {
    let world = demo_choosing(move_player("ruins"));
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
}

#[test]
fn a_place_that_does_not_exist_is_a_missing_reference() {
    let world = demo_choosing(move_player("atlantis"));
    assert_eq!(codes(&world), ["missing_reference"]);
}

#[test]
fn a_start_option_cannot_move_the_player() {
    let mut world = demo();
    world.world.start_questions.push(StartQuestion {
        id: "origin".into(),
        name: "Origin".into(),
        text: "Where did you wake?".into(),
        options: vec![StartOption {
            id: "chapel".into(),
            text: "In the chapel.".into(),
            effects: vec![move_player("ruins")],
        }],
    });
    assert_eq!(codes(&world), ["invalid_effect"]);
}

#[test]
fn a_scheduled_event_cannot_move_the_player() {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/turning-ford"
    ))
    .unwrap();
    world.world.events[0].effects = vec![move_player("ford")];
    assert_eq!(codes(&world), ["invalid_effect"]);
}
