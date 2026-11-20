//! The `move_player` effect: a dialogue choice moves the player to another
//! place with no road between, as a scripted arrival. The party comes along,
//! no time passes, and a speaker left behind ends the conversation.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn hollow_ford() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/hollow-ford"
    ))
    .unwrap()
}

/// Hollow Ford with one more choice at the start of Mara's and of Wren's
/// greeting: "Walk on." with `effects`, returning to the greeting.
fn ford_walking_on(effects: Vec<Effect>) -> WorldSpec {
    let mut world = hollow_ford();
    for dialogue in &mut world.dialogues {
        let greeting = dialogue
            .nodes
            .iter_mut()
            .find(|n| n.id == "greeting")
            .unwrap();
        greeting.choices.push(DialogueChoice {
            text: "Walk on.".into(),
            next: Some("greeting".into()),
            effects: effects.clone(),
            ..Default::default()
        });
    }
    world
}

fn to(place: &str) -> Effect {
    Effect::MovePlayer { to: place.into() }
}

/// Takes the open conversation's choice with this text.
fn pick(engine: &mut Engine<'_>, text: &str) -> Result<Vec<Event>, EngineError> {
    let number = engine
        .dialogue_choices()
        .iter()
        .position(|c| c.text == text)
        .unwrap_or_else(|| panic!("no choice {text:?}"))
        + 1;
    engine.execute(ChooseDialogue(number))
}

/// Mara vouches, and Wren joins the party at the inn.
fn with_wren(engine: &mut Engine<'_>) {
    engine.execute(Talk("mara".into())).unwrap();
    pick(engine, "Who would come with me?").unwrap();
    pick(engine, "Thank you.").unwrap();
    engine.execute(Talk("wren".into())).unwrap();
    pick(engine, "Mara sent me. Ride with me to the ford.").unwrap();
    assert_eq!(engine.party(), ["wren"]);
}

#[test]
fn a_choice_moves_the_player_with_no_road_and_no_time() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let time = engine.state().time;
    engine.execute(Talk("mara".into())).unwrap();
    let events = pick(&mut engine, "Walk on.").unwrap();
    assert_eq!(
        events,
        [
            Event::Moved {
                from: "inn".into(),
                to: "crossroads".into()
            },
            Event::LocationViewed {
                location: "crossroads".into()
            },
            Event::DialogueEnded,
        ]
    );
    assert_eq!(engine.state().player.location, "crossroads");
    assert_eq!(engine.state().time, time, "the move passes no time");
    assert!(engine.visited("inn") && engine.visited("crossroads"));
    assert!(!engine.visited("ford"));
}

#[test]
fn the_same_seed_and_commands_replay_the_move_identically() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let play = || {
        let mut engine = Engine::new_with_seed(&world, 7).unwrap();
        let mut events = engine.execute(Talk("mara".into())).unwrap();
        events.extend(pick(&mut engine, "Walk on.").unwrap());
        (events, engine.snapshot())
    };
    assert_eq!(play(), play());
}

#[test]
fn a_later_effect_that_fails_leaves_the_player_where_they_were() {
    // Wren is not in the party, so she cannot leave it.
    let refused = Effect::LeaveParty {
        character: "wren".into(),
    };
    let world = ford_walking_on(vec![to("crossroads"), refused]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("mara".into())).unwrap();
    let before = engine.snapshot();
    assert!(pick(&mut engine, "Walk on.").is_err());
    assert_eq!(engine.snapshot(), before);
    assert_eq!(engine.state().player.location, "inn");
}

#[test]
fn the_party_comes_along_and_a_companion_speaker_keeps_talking() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    with_wren(&mut engine);
    engine.execute(Talk("wren".into())).unwrap();
    let events = pick(&mut engine, "Walk on.").unwrap();
    assert!(!events.contains(&Event::DialogueEnded), "{events:?}");
    assert_eq!(engine.state().player.location, "crossroads");
    assert_eq!(engine.character_location("wren"), Some("crossroads"));
    assert_eq!(engine.party(), ["wren"]);
    let open = engine.state().dialogue.as_ref().expect("Wren talks on");
    assert_eq!(
        (open.npc.as_str(), open.node.as_str()),
        ("wren", "greeting")
    );
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn a_speaker_left_behind_ends_the_conversation_and_stays() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("mara".into())).unwrap();
    pick(&mut engine, "Walk on.").unwrap();
    assert!(engine.state().dialogue.is_none());
    assert_eq!(engine.character_location("mara"), Some("inn"));
}

#[test]
fn the_new_place_offers_its_own_actions() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let talks = |engine: &Engine<'_>, who: &str| {
        offered(engine)
            .iter()
            .any(|(c, _)| matches!(c, Talk(id) if id == who))
    };
    assert!(talks(&engine, "mara") && !talks(&engine, "oake"));
    engine.execute(Talk("mara".into())).unwrap();
    pick(&mut engine, "Walk on.").unwrap();
    assert!(talks(&engine, "oake") && !talks(&engine, "mara"));
}

#[test]
fn moving_to_the_place_already_here_changes_nothing() {
    let world = ford_walking_on(vec![to("inn")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("mara".into())).unwrap();
    let events = pick(&mut engine, "Walk on.").unwrap();
    assert!(
        events.iter().all(|e| !matches!(
            e,
            Event::Moved { .. } | Event::LocationViewed { .. } | Event::DialogueEnded
        )),
        "{events:?}"
    );
    assert_eq!(engine.state().player.location, "inn");
    assert!(engine.state().dialogue.is_some(), "Mara is still here");
}

#[test]
fn a_fight_in_progress_refuses_the_choice() {
    let world = ford_walking_on(vec![to("inn")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("crossroads".into())).unwrap();
    engine.execute(Travel("ford".into())).unwrap();
    engine
        .execute(EngageRealtime("ford_captain".into()))
        .unwrap();
    let before = engine.snapshot();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::InEncounter)
    ));
    assert_eq!(engine.snapshot(), before);
}

#[test]
fn a_save_after_the_move_loads_and_a_forged_place_is_refused() {
    let world = ford_walking_on(vec![to("crossroads")]);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("mara".into())).unwrap();
    pick(&mut engine, "Walk on.").unwrap();
    let saved = engine.snapshot();
    let resumed = Engine::restore(&world, saved.clone()).unwrap();
    assert_eq!(resumed.state().player.location, "crossroads");
    let mut forged = saved.clone();
    forged.state.player.location = "atlantis".into();
    assert!(Engine::restore(&world, forged).is_err());
    let mut forged = saved;
    forged.state.visited.insert("atlantis".into());
    assert!(Engine::restore(&world, forged).is_err());
}
