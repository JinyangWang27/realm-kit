//! Roads whose time varies: drawn from the travel stream as the player sets
//! out, never before.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;
use std::collections::BTreeMap;

/// The marches, with the road to Ashmere taking 90 minutes once in eight,
/// 2 h six times and 4 h once.
fn varied() -> WorldSpec {
    let mut world = marches();
    let road = world
        .world
        .roads
        .iter_mut()
        .find(|r| r.id == "greyford-ashmere")
        .unwrap();
    road.minutes = 0;
    road.durations = [(90, 1), (120, 6), (240, 1)]
        .map(|(minutes, weight)| TravelTime { minutes, weight })
        .to_vec();
    world
}

/// The minutes a successful travel took, from its `TimePassed`.
fn took(events: &[Event]) -> u64 {
    let passed = events.iter().find_map(|e| match e {
        Event::TimePassed { minutes, .. } => Some(*minutes),
        _ => None,
    });
    passed.unwrap()
}

fn travel_stream(engine: &Engine<'_>) -> u64 {
    engine.state().rng.unwrap().travel.unwrap()
}

#[test]
fn a_fixed_road_takes_its_minutes_and_draws_nothing() {
    let world = varied();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(travel_stream(&engine), 7 ^ 0x7472_6176_656c);
    // Only a world with a road whose time varies keeps a travel stream.
    let fixed = marches();
    let rng = Engine::new_with_seed(&fixed, 7)
        .unwrap()
        .state()
        .rng
        .unwrap();
    assert_eq!(rng.travel, None);
    let before = travel_stream(&engine);
    let events = engine.execute(Travel("hollin_keep".into())).unwrap();
    assert_eq!(took(&events), 240);
    assert_eq!(engine.state().time, Some(480 + 240));
    assert_eq!(travel_stream(&engine), before);
}

#[test]
fn seeds_draw_every_travel_time_by_weight_and_a_seed_replays_exactly() {
    let world = varied();
    let mut counts = BTreeMap::new();
    for seed in 0..80 {
        let mut engine = Engine::new_with_seed(&world, seed).unwrap();
        let events = engine.execute(Travel("ashmere".into())).unwrap();
        let minutes = took(&events);
        assert_eq!(engine.state().time, Some(480 + minutes));
        *counts.entry(minutes).or_insert(0) += 1;
    }
    assert_eq!(counts.keys().copied().collect::<Vec<_>>(), [90, 120, 240]);
    assert!(counts[&120] > counts[&90] + counts[&240], "{counts:?}");
    // The same seed and commands give the same journeys, there and back.
    let run = || {
        let mut engine = Engine::new_with_seed(&world, 11).unwrap();
        let mut events = Vec::new();
        for to in ["ashmere", "greyford", "ashmere", "greyford"] {
            events.extend(engine.execute(Travel(to.into())).unwrap());
        }
        (events, engine.state().clone())
    };
    assert_eq!(run(), run());
}

#[test]
fn a_save_keeps_the_next_draw_before_and_after_travel() {
    let world = varied();
    let mut engine = Engine::new_with_seed(&world, 3).unwrap();
    let roundtrip = |engine: &Engine<'_>| {
        let json = serde_json::to_string(&engine.snapshot()).unwrap();
        serde_json::from_str::<SaveSnapshot>(&json).unwrap()
    };
    for _ in 0..3 {
        let mut resumed = Engine::restore(&world, roundtrip(&engine)).unwrap();
        for to in ["ashmere", "greyford"] {
            assert_eq!(
                resumed.execute(Travel(to.into())).unwrap(),
                engine.execute(Travel(to.into())).unwrap()
            );
        }
        assert_eq!(resumed.state(), engine.state());
    }
}

#[test]
fn looking_at_the_road_draws_nothing() {
    let world = varied();
    let mut engine = Engine::new_with_seed(&world, 5).unwrap();
    let mut untouched = Engine::new_with_seed(&world, 5).unwrap();
    let before = engine.state().clone();
    let view = engine.map_view().unwrap();
    let road = |id: &str| view.roads.iter().find(|r| r.road == id).unwrap();
    assert_eq!(
        (
            road("greyford-ashmere").minutes,
            road("greyford-ashmere").longest
        ),
        (90, Some(240))
    );
    assert_eq!(
        (
            road("greyford-hollin").minutes,
            road("greyford-hollin").longest
        ),
        (240, None)
    );
    assert!(engine
        .actions()
        .iter()
        .any(|a| a.command == Travel("ashmere".into()) && a.available));
    engine.journal();
    for command in [Look, Map, Status, Inventory, Quests] {
        engine.execute(command).unwrap();
    }
    assert_eq!(engine.state().rng, before.rng);
    assert_eq!(
        engine.execute(Travel("ashmere".into())).unwrap(),
        untouched.execute(Travel("ashmere".into())).unwrap()
    );
}

#[test]
fn other_streams_drawing_first_never_change_the_journey() {
    let world = varied();
    let journeys = |wait: bool| {
        let mut engine = Engine::new_with_seed(&world, 21).unwrap();
        if wait {
            // Wenna moves, prices tick and merchants restock meanwhile.
            engine.execute(Wait(4 * 1_440)).unwrap();
        }
        let mut taken = Vec::new();
        for to in ["ashmere", "greyford", "ashmere"] {
            taken.push(took(&engine.execute(Travel(to.into())).unwrap()));
        }
        (taken, engine.state().rng.unwrap())
    };
    let (quiet, quiet_rng) = journeys(false);
    let (busy, busy_rng) = journeys(true);
    assert_ne!(quiet_rng.world, busy_rng.world);
    assert_eq!(quiet, busy);
    assert_eq!(quiet_rng.travel, busy_rng.travel);
}

#[test]
fn a_refused_journey_draws_nothing() {
    // The causeway varies too, and is closed until the thaw.
    let mut world = varied();
    let causeway = world
        .world
        .roads
        .iter_mut()
        .find(|r| r.id == "fen-causeway")
        .unwrap();
    causeway.minutes = 0;
    causeway.durations = [(150, 1), (210, 1)]
        .map(|(minutes, weight)| TravelTime { minutes, weight })
        .to_vec();
    let mut engine = Engine::new_with_seed(&world, 2).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Travel("vellmarket".into())),
        Err(EngineError::RoadBlocked { .. })
    ));
    assert_eq!(engine.state(), &before);
    // Every journey runs past the end of time: it draws, fails and is rolled
    // back whole.
    let mut world = varied();
    unscheduled(&mut world);
    world.world.time.as_mut().unwrap().start = WORLD_TIME_BOUND - 60;
    let mut engine = Engine::new_with_seed(&world, 2).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Travel("ashmere".into())),
        Err(EngineError::NumericLimit)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn what_falls_due_on_the_way_happens_only_on_a_long_enough_journey() {
    // The thaw comes at 09:20; a 1 h journey arrives before it, a 4 h one after.
    let mut world = varied();
    world.world.events[0].schedule.at = 560;
    let road = &mut world.world.roads[0];
    assert_eq!(road.id, "greyford-ashmere");
    road.durations = [(60, 1), (240, 1)]
        .map(|(minutes, weight)| TravelTime { minutes, weight })
        .to_vec();
    let mut seen = BTreeMap::new();
    for seed in 0..16 {
        let mut engine = Engine::new_with_seed(&world, seed).unwrap();
        let events = engine.execute(Travel("ashmere".into())).unwrap();
        let minutes = took(&events);
        let thawed = engine.state().flags.contains("thaw");
        assert_eq!(thawed, minutes == 240, "seed {seed}");
        if thawed {
            // Moved, then the thaw on the way, then the time, then arrival.
            let thaw = events
                .iter()
                .position(|e| {
                    *e == Event::StoryFlagSet {
                        flag: "thaw".into(),
                    }
                })
                .unwrap();
            assert!(matches!(events[0], Event::Moved { .. }));
            assert!(thaw < events.len() - 2);
            assert!(matches!(
                events[events.len() - 2],
                Event::TimePassed {
                    minutes: 240,
                    now: 720,
                    ..
                }
            ));
        }
        seen.insert(minutes, events);
    }
    assert_eq!(seen.len(), 2, "both journeys are drawn");
}

#[test]
fn a_save_must_carry_the_travel_stream_exactly_when_a_road_varies() {
    let world = varied();
    let mut missing = Engine::new_with_seed(&world, 7).unwrap().snapshot();
    missing.state.rng.as_mut().unwrap().travel = None;
    let fixed = marches();
    let mut stray = Engine::new_with_seed(&fixed, 7).unwrap().snapshot();
    stray.state.rng.as_mut().unwrap().travel = Some(1);
    for (world, snapshot) in [(&world, missing), (&fixed, stray)] {
        assert!(matches!(
            Engine::restore(world, snapshot),
            Err(EngineError::InvalidSave(_))
        ));
    }
}
