//! The overland map query and its panel.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

#[test]
fn the_map_shows_every_place_and_road_with_the_player() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let view = engine.map_view().unwrap();
    assert_eq!(view.here, "greyford");
    let places: Vec<_> = view
        .places
        .iter()
        .map(|p| (p.location.as_str(), p.point.x, p.point.y, p.point.kind))
        .collect();
    assert_eq!(
        places,
        [
            ("greyford", 1000, 1000, PlaceKind::Town),
            ("ashmere", 1080, 1110, PlaceKind::Village),
            ("hollin_keep", 1180, 840, PlaceKind::Castle),
            ("vellmarket", 1300, 1080, PlaceKind::Town),
        ]
    );
    let roads: Vec<_> = view
        .roads
        .iter()
        .map(|r| (r.between[0].as_str(), r.between[1].as_str(), r.minutes))
        .collect();
    assert_eq!(
        roads,
        [
            ("greyford", "ashmere", 120),
            ("greyford", "hollin_keep", 240),
            ("hollin_keep", "vellmarket", 300),
            ("ashmere", "vellmarket", 180),
        ]
    );
    assert!(view.exits.is_empty());
    // Looking at the map spends no turn and moves no clock.
    assert!(offered(&engine).contains(&(Map, true)));
    let before = engine.state().clone();
    assert_eq!(engine.execute(Map).unwrap(), [Event::MapViewed]);
    assert_eq!(engine.state(), &before);
    engine.execute(Travel("ashmere".into())).unwrap();
    assert_eq!(engine.map_view().unwrap().here, "ashmere");
}

#[test]
fn exits_between_positioned_places_are_one_way_links() {
    let mut world = demo();
    for (i, l) in world.locations.iter_mut().enumerate() {
        l.map = Some(MapPoint {
            x: i as u32,
            y: 0,
            kind: PlaceKind::Waypoint,
        });
    }
    let engine = Engine::new(&world).unwrap();
    let view = engine.map_view().unwrap();
    let expected: Vec<_> = world
        .locations
        .iter()
        .flat_map(|l| {
            l.exits
                .iter()
                .map(|(d, e)| (l.id.clone(), e.destination.clone(), *d))
        })
        .collect();
    let exits: Vec<_> = view
        .exits
        .into_iter()
        .map(|e| (e.from, e.to, e.direction))
        .collect();
    assert!(!exits.is_empty());
    assert_eq!(exits, expected);
}

#[test]
fn a_world_without_positions_has_no_map() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(engine.map_view(), None);
    assert!(!offered(&engine).iter().any(|(c, _)| *c == Map));
    assert!(matches!(engine.execute(Map), Err(EngineError::NoMap)));
}

#[test]
fn an_unknown_place_is_off_the_map_and_off_the_roads() {
    let mut world = marches();
    let keep = world
        .locations
        .iter_mut()
        .find(|l| l.id == "hollin_keep")
        .unwrap();
    keep.known_when = Some(Condition::Flag {
        flag: "letter_delivered".into(),
    });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let view = engine.map_view().unwrap();
    assert!(!view.places.iter().any(|p| p.location == "hollin_keep"));
    assert!(!view
        .roads
        .iter()
        .any(|r| r.between.contains(&"hollin_keep".to_string())));
    assert_eq!(view.roads.len(), 2);
    // A hidden road is no spoiler: not even offered as closed.
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| *c == Travel("hollin_keep".into())));
    assert!(matches!(
        engine.execute(Travel("hollin_keep".into())),
        Err(EngineError::NoRoad(_))
    ));
    // Once the player hears of the keep, it is on the map and the road.
    let mut snapshot = engine.snapshot();
    snapshot.state.flags.insert("letter_delivered".into());
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    assert_eq!(engine.map_view().unwrap().places.len(), 4);
    assert!(offered(&engine).contains(&(Travel("hollin_keep".into()), true)));
    engine.execute(Travel("hollin_keep".into())).unwrap();
}
