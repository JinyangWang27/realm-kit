//! World time: roads that take time, waiting, scheduled events and
//! characters who move.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn now(engine: &Engine<'_>) -> u64 {
    engine.state().time.unwrap()
}

fn travel(engine: &mut Engine<'_>, to: &str) -> Vec<Event> {
    engine.execute(Travel(to.into())).unwrap()
}

fn wenna<'e>(engine: &'e Engine<'_>) -> &'e str {
    &engine.state().whereabouts["wenna"]
}

#[test]
fn a_road_takes_its_time_and_a_closed_one_changes_nothing() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(now(&engine), 480);
    engine.execute(Talk("wenna".into())).unwrap();
    let events = travel(&mut engine, "ashmere");
    assert_eq!(
        events,
        [
            Event::Moved {
                from: "greyford".into(),
                to: "ashmere".into()
            },
            Event::TimePassed {
                minutes: 120,
                now: 600
            },
            Event::LocationViewed {
                location: "ashmere".into()
            }
        ]
    );
    // Travelling ends a conversation, like moving.
    assert_eq!(engine.state().dialogue, None);
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Travel("vellmarket".into())),
        Err(EngineError::RoadBlocked { road }) if road == "fen-causeway"
    ));
    assert!(matches!(
        engine.execute(Travel("hollin_keep".into())),
        Err(EngineError::NoRoad(to)) if to == "hollin_keep"
    ));
    assert_eq!(engine.state(), &before);
    // Roads go both ways.
    travel(&mut engine, "greyford");
    assert_eq!(
        (engine.state().player.location.as_str(), now(&engine)),
        ("greyford", 720)
    );
}

#[test]
fn an_event_happens_when_time_reaches_its_minute_and_not_before() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    travel(&mut engine, "ashmere");
    // Day 2, 14:00 is minute 2,280; one minute short does nothing.
    engine.execute(Wait(2_280 - 600 - 1)).unwrap();
    assert!(!engine.state().flags.contains("thaw"));
    let events = engine.execute(Wait(1)).unwrap();
    assert!(events.contains(&Event::StoryFlagSet {
        flag: "thaw".into()
    }));
    assert_eq!(now(&engine), 2_280);
    // A one-shot event never happens again, and opens the causeway.
    let events = engine.execute(Wait(3 * 1_440)).unwrap();
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::StoryFlagSet { .. })));
    travel(&mut engine, "vellmarket");
    assert_eq!(engine.state().player.location, "vellmarket");
}

#[test]
fn a_mover_moves_once_a_day_and_is_found_only_where_it_is() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(wenna(&engine), "greyford");
    // Her first move is on day 2 at 06:00; waiting to 05:59 leaves her be.
    engine.execute(Wait(1_800 - 480 - 1)).unwrap();
    assert_eq!(wenna(&engine), "greyford");
    let mut places = Vec::new();
    for _ in 0..12 {
        engine.execute(Wait(1_440)).unwrap();
        places.push(wenna(&engine).to_string());
        let here = engine.state().player.location.clone();
        let talk = engine
            .actions()
            .iter()
            .any(|a| a.command == Talk("wenna".into()));
        assert_eq!(talk, wenna(&engine) == here);
    }
    // Every place she may be comes up, and only those.
    for place in ["greyford", "ashmere", "vellmarket"] {
        assert!(places.iter().any(|p| p == place), "{place} in {places:?}");
    }
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn arrivals_and_departures_at_the_players_location_are_reported() {
    let world = marches();
    let mut seen = (false, false);
    for seed in 0..20 {
        let mut engine = Engine::new_with_seed(&world, seed).unwrap();
        // Waiting in Greyford over her first move.
        let events = engine.execute(Wait(1_800 - 480)).unwrap();
        let left = Event::CharacterLeft {
            character: "wenna".into(),
        };
        assert_eq!(
            events.contains(&left),
            wenna(&engine) != "greyford",
            "seed {seed}"
        );
        seen.0 |= events.contains(&left);
        // Back to Greyford from wherever she is, she can only arrive.
        let events = engine.execute(Wait(1_440)).unwrap();
        let arrived = Event::CharacterArrived {
            character: "wenna".into(),
        };
        if events.contains(&arrived) {
            assert_eq!(wenna(&engine), "greyford");
            seen.1 = true;
        }
    }
    assert_eq!(seen, (true, true));
}

#[test]
fn the_same_seed_and_commands_give_the_same_world_and_others_differ() {
    let world = marches();
    let run = |seed| {
        let mut engine = Engine::new_with_seed(&world, seed).unwrap();
        let mut events = Vec::new();
        for _ in 0..10 {
            events.extend(engine.execute(Wait(1_440)).unwrap());
        }
        (engine.state().clone(), events)
    };
    assert_eq!(run(3), run(3));
    assert!((0..10).any(|seed| run(seed).0.whereabouts != run(3).0.whereabouts));
}

#[test]
fn a_time_of_day_condition_follows_the_clock() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    travel(&mut engine, "hollin_keep");
    let steward = Talk("steward".into());
    let present = |engine: &Engine<'_>| engine.actions().iter().any(|a| a.command == steward);
    // 12:00: the steward sees petitioners until 20:00.
    assert!(present(&engine));
    engine.execute(Wait(7 * 60 + 59)).unwrap();
    assert!(present(&engine));
    engine.execute(Wait(1)).unwrap();
    assert!(!present(&engine));
    assert!(matches!(
        engine.execute(Talk("steward".into())),
        Err(EngineError::NotHere(_))
    ));
    // Back at 08:00 the next morning.
    engine.execute(Wait(12 * 60)).unwrap();
    assert!(present(&engine));
}

#[test]
fn waiting_needs_an_authored_wait_and_a_sensible_length() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let before = engine.state().clone();
    for minutes in [0, DURATION_BOUND + 1] {
        assert!(matches!(
            engine.execute(Wait(minutes)),
            Err(EngineError::InvalidWait)
        ));
    }
    assert_eq!(engine.state(), &before);
    let mut fixed = marches();
    fixed.world.time.as_mut().unwrap().wait = None;
    let mut engine = Engine::new_with_seed(&fixed, 7).unwrap();
    assert!(matches!(
        engine.execute(Wait(60)),
        Err(EngineError::NoWaiting)
    ));
    assert!(!engine
        .actions()
        .iter()
        .any(|a| matches!(a.command, Wait(_))));
    // A world without a clock has no time at all.
    let demo = demo();
    let mut engine = Engine::new(&demo).unwrap();
    assert_eq!(engine.state().time, None);
    assert!(matches!(
        engine.execute(Wait(60)),
        Err(EngineError::NoWaiting)
    ));
}

#[test]
fn events_and_movers_at_the_same_minute_go_in_schedule_order() {
    let mut world = marches();
    // A daily bell at Wenna's minute: events come before movers.
    world.world.flags.push("bell".into());
    world.world.events.push(WorldEvent {
        id: "bell".into(),
        schedule: Schedule {
            at: 1_800,
            every: Some(1_440),
        },
        requires: None,
        effects: vec![Effect::SetFlag {
            flag: "bell".into(),
        }],
    });
    for seed in 0..20 {
        let mut engine = Engine::new_with_seed(&world, seed).unwrap();
        let events = engine.execute(Wait(1_800 - 480)).unwrap();
        let bell = events.iter().position(|e| {
            *e == Event::StoryFlagSet {
                flag: "bell".into(),
            }
        });
        let left = events
            .iter()
            .position(|e| matches!(e, Event::CharacterLeft { .. }));
        if let Some(left) = left {
            assert!(bell.unwrap() < left);
            return;
        }
    }
    panic!("Wenna never left in 20 seeds");
}

#[test]
fn resting_takes_the_authored_time_in_a_world_with_a_clock() {
    let mut world = arena();
    world.world.time = Some(WorldTime {
        start: 0,
        clock: TextTemplate("Day {day}, {hour}:{minute}".into()),
        wait: None,
        rest: Some(480),
    });
    let mut engine = Engine::new(&world).unwrap();
    let events = engine.execute(Rest).unwrap();
    assert!(events.contains(&Event::TimePassed {
        minutes: 480,
        now: 480
    }));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn a_save_keeps_the_clock_and_the_whereabouts_and_rejects_impossible_ones() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    travel(&mut engine, "ashmere");
    engine.execute(Wait(2 * 1_440)).unwrap();
    let good = engine.snapshot();
    let json = serde_json::to_string(&good).unwrap();
    let mut resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(resumed.state(), engine.state());
    // Resumed play continues exactly as uninterrupted play does.
    assert_eq!(
        resumed.execute(Wait(5 * 1_440)).unwrap(),
        engine.execute(Wait(5 * 1_440)).unwrap()
    );
    assert_eq!(resumed.state(), engine.state());

    let mut early = good.clone();
    early.state.time = Some(100);
    let mut missing = good.clone();
    missing.state.time = None;
    let mut astray = good.clone();
    astray
        .state
        .whereabouts
        .insert("wenna".into(), "hollin_keep".into());
    let mut lost = good.clone();
    lost.state.whereabouts.clear();
    let mut unseeded = good.clone();
    unseeded.state.rng.as_mut().unwrap().world = None;
    // The thaw is set at minute 2,280; a save before then cannot have it.
    let mut fresh = Engine::new_with_seed(&world, 7).unwrap().snapshot();
    fresh.state.flags.insert("thaw".into());
    for (i, snapshot) in [early, missing, astray, lost, unseeded, fresh]
        .into_iter()
        .enumerate()
    {
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
fn time_never_passes_beyond_its_bound() {
    let mut world = marches();
    world.world.time.as_mut().unwrap().start = WORLD_TIME_BOUND - 60;
    world.world.events.clear();
    world.characters.retain(|c| c.moves.is_none());
    world.locations[0].characters.clear();
    let mut engine = Engine::new(&world).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Wait(61)),
        Err(EngineError::NumericLimit)
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(Wait(60)).unwrap();
}
