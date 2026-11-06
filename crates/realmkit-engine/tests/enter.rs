//! Host-reported presence: `Enter` brings the player into a location the
//! host says they reached across its continuous ground, with no road, no
//! exit and no time.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::Direction;

#[test]
fn entering_needs_no_road_and_passes_no_time() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(engine.state().player.location, "greyford");
    // No road joins Greyford and Vellmarket.
    assert!(matches!(
        engine.execute(Travel("vellmarket".into())),
        Err(EngineError::NoRoad(_))
    ));
    let time = engine.state().time;
    let turn = engine.state().turn;
    let events = engine.execute(Enter("vellmarket".into())).unwrap();
    assert_eq!(
        events,
        [
            Event::Moved {
                from: "greyford".into(),
                to: "vellmarket".into()
            },
            Event::LocationViewed {
                location: "vellmarket".into()
            },
        ]
    );
    assert_eq!(engine.state().player.location, "vellmarket");
    assert_eq!(engine.state().time, time, "entering passes no time");
    assert_eq!(engine.state().turn, turn + 1);
    assert!(engine.visited("greyford") && engine.visited("vellmarket"));
    // Entering is never offered: the host reports it.
    assert!(offered(&engine).iter().all(|(c, _)| !matches!(c, Enter(_))));
}

#[test]
fn a_closed_road_does_not_close_the_ground_beside_it() {
    let world = marches();
    let road = world
        .world
        .roads
        .iter()
        .find(|r| r.requires.is_some())
        .expect("the marches have a road with a condition")
        .clone();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let [a, b] = [road.between[0].clone(), road.between[1].clone()];
    if engine.state().player.location != a {
        engine.execute(Enter(a.clone())).unwrap();
    }
    assert!(matches!(
        engine.execute(Travel(b.clone())),
        Err(EngineError::RoadBlocked { .. })
    ));
    engine.execute(Enter(b.clone())).unwrap();
    assert_eq!(engine.state().player.location, b);
}

#[test]
fn an_unknown_place_can_be_entered_and_is_known_while_there() {
    let world = caravan_trail();
    let mut engine = Engine::start(&world, 7, &["caravan_guard".into()]).unwrap();
    assert!(!engine.knows("abandoned_camp"));
    engine.execute(Enter("abandoned_camp".into())).unwrap();
    assert!(engine.knows("abandoned_camp"));
    assert!(engine.visited("abandoned_camp"));
    // Being there grants nothing else: no evidence, no phase.
    assert!(engine.journal().evidence.is_empty());
}

#[test]
fn entering_is_refused_for_no_place_or_the_place_already_here() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Enter("atlantis".into())),
        Err(EngineError::UnknownLocation(_))
    ));
    assert!(matches!(
        engine.execute(Enter("greyford".into())),
        Err(EngineError::AlreadyThere(_))
    ));
    assert_eq!(engine.state(), &before, "a refusal changes nothing");
}

#[test]
fn entering_ends_a_conversation_as_leaving_by_road_does() {
    let world = caravan_trail();
    let mut engine = Engine::start(&world, 7, &["caravan_guard".into()]).unwrap();
    let speaker = engine
        .actions()
        .into_iter()
        .find_map(|a| match a.command {
            Talk(id) => Some(id),
            _ => None,
        })
        .expect("someone to talk to at the start");
    engine.execute(Talk(speaker)).unwrap();
    assert!(engine.state().dialogue.is_some());
    engine.execute(Enter("thornwick".into())).unwrap();
    assert!(engine.state().dialogue.is_none());
}

#[test]
fn entering_is_refused_in_a_fight() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(Direction::North)).unwrap();
    engine.execute(Engage("black_wolf".into())).unwrap();
    let other = world
        .locations
        .iter()
        .find(|l| l.id != engine.state().player.location)
        .map(|l| l.id.clone());
    let other = other.expect("the arena has more than one place");
    assert!(matches!(
        engine.execute(Enter(other)),
        Err(EngineError::InEncounter)
    ));
}
