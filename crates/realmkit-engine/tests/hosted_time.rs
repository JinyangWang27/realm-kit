//! Hosted world time: a world whose host decides every duration. Travel and
//! rest take no time of their own, and `Timed` gives them the host's, as one
//! command. Everything the time brings is still RealmKit's, on its one clock.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

/// The marches with the host deciding every duration: no wait, no rest, no
/// road times.
fn hosted(mut world: WorldSpec) -> WorldSpec {
    let time = world.world.time.as_mut().unwrap();
    time.hosted = true;
    time.wait = None;
    time.rest = None;
    for road in &mut world.world.roads {
        road.minutes = 0;
    }
    world.validate().unwrap();
    world
}

fn timed(action: Command, minutes: u64) -> Command {
    Timed {
        action: Box::new(action),
        minutes,
    }
}

fn now(engine: &Engine<'_>) -> u64 {
    engine.state().time.unwrap()
}

/// The same play in the authored marches and in the hosted marches, with
/// the host giving each journey and rest the authored time: every event and
/// all state agree, the thaw and Wenna's moves crossed on the road included.
#[test]
fn a_timed_journey_or_rest_is_the_authored_one_of_another_world() {
    let authored = marches();
    let world = hosted(marches());
    let mut own = Engine::new_with_seed(&authored, 7).unwrap();
    let mut host = Engine::new_with_seed(&world, 7).unwrap();
    let steps = [
        (
            Travel("ashmere".into()),
            timed(Travel("ashmere".into()), 120),
        ),
        (
            Travel("greyford".into()),
            timed(Travel("greyford".into()), 120),
        ),
        (Rest, timed(Rest, 480)),
        (Wait(1_440), PassTime(1_440)),
        (
            Travel("ashmere".into()),
            timed(Travel("ashmere".into()), 120),
        ),
        (
            Travel("vellmarket".into()),
            timed(Travel("vellmarket".into()), 180),
        ),
    ];
    for (index, (theirs, ours)) in steps.into_iter().enumerate() {
        let expected = own.execute(theirs).unwrap();
        let events = host.execute(ours).unwrap();
        // A wait settles once and PassTime every minute; only their
        // `eventful` reports may differ.
        let comparable = |events: Vec<Event>| -> Vec<Event> {
            events
                .into_iter()
                .map(|e| match e {
                    Event::TimePassed { minutes, now, .. } => Event::TimePassed {
                        minutes,
                        now,
                        eventful: false,
                    },
                    e => e,
                })
                .collect()
        };
        assert_eq!(comparable(events), comparable(expected), "step {index}");
        assert_eq!(host.state(), own.state(), "step {index}");
    }
    assert_eq!(host.state().player.location, "vellmarket");
    assert!(host.state().flags.contains("thaw"));
}

#[test]
fn untimed_travel_and_rest_take_no_time_in_a_hosted_world() {
    let world = hosted(marches());
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let events = engine.execute(Travel("ashmere".into())).unwrap();
    assert_eq!(now(&engine), 480);
    assert_eq!(engine.state().player.location, "ashmere");
    assert!(!events.iter().any(|e| matches!(e, Event::TimePassed { .. })));
    engine.execute(Travel("greyford".into())).unwrap();
    let events = engine.execute(Rest).unwrap();
    assert!(events.contains(&Event::Rested));
    assert_eq!(now(&engine), 480);
    // No authored wait: the host passes time itself.
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Wait(_))));
    assert!(matches!(
        engine.execute(Wait(60)),
        Err(EngineError::NoWaiting)
    ));
}

/// A journey arrives first: the player is at the destination for the whole
/// span, which passes on the road, and the destination is seen once at the
/// end.
#[test]
fn a_timed_journey_moves_the_player_then_passes_its_time_on_the_road() {
    let world = hosted(marches());
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    // Across the thaw at day 2, 14:00.
    let events = engine
        .execute(timed(Travel("ashmere".into()), 2_000))
        .unwrap();
    let at = |wanted: &dyn Fn(&Event) -> bool| events.iter().position(wanted).unwrap();
    let moved = at(&|e| matches!(e, Event::Moved { .. }));
    let thaw = at(&|e| {
        *e == Event::StoryFlagSet {
            flag: "thaw".into(),
        }
    });
    let passed = at(&|e| matches!(e, Event::TimePassed { .. }));
    let viewed = at(&|e| matches!(e, Event::LocationViewed { .. }));
    assert!(
        moved < thaw && thaw < passed && passed < viewed,
        "{events:?}"
    );
    assert_eq!(now(&engine), 480 + 2_000);
    // Comings and goings at the destination are not seen from the road.
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::CharacterArrived { .. } | Event::CharacterLeft { .. }
    )));
}

#[test]
fn a_refused_timed_command_changes_nothing() {
    let world = hosted(marches());
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let before = engine.state().clone();
    let cases = [
        (timed(Travel("ashmere".into()), 0), "InvalidDuration"),
        (
            timed(Travel("ashmere".into()), DURATION_BOUND + 1),
            "InvalidDuration",
        ),
        (timed(Wait(60), 60), "Untimed"),
        (timed(PassTime(60), 60), "Untimed"),
        (timed(Talk("wenna".into()), 60), "Untimed"),
        (timed(timed(Rest, 60), 60), "Untimed"),
        (timed(Travel("vellmarket".into()), 60), "NoRoad"),
    ];
    for (command, error) in cases {
        let refused = engine.execute(command.clone()).unwrap_err();
        assert!(
            format!("{refused:?}").starts_with(error),
            "{command:?}: {refused:?}"
        );
        assert_eq!(engine.state(), &before, "{command:?}");
    }
    // A blocked road refuses before any time passes.
    engine.execute(Travel("ashmere".into())).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(timed(Travel("vellmarket".into()), 180)),
        Err(EngineError::RoadBlocked { .. })
    ));
    assert_eq!(engine.state(), &before);
    // Resting needs a safe place, timed or not.
    assert!(matches!(
        engine.execute(timed(Rest, 480)),
        Err(EngineError::NotSafe)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn only_a_hosted_world_takes_the_hosts_durations() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let before = engine.state().clone();
    for command in [timed(Travel("ashmere".into()), 5), timed(Rest, 5)] {
        assert!(matches!(
            engine.execute(command),
            Err(EngineError::NotHosted)
        ));
    }
    assert_eq!(engine.state(), &before);
}

#[test]
fn a_timed_command_is_refused_in_a_battle() {
    let world = hosted(marches());
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let mut snapshot = engine.snapshot();
    snapshot
        .state
        .retinue
        .as_mut()
        .unwrap()
        .roster
        .entry("levy".into())
        .or_default()
        .insert(
            1,
            Squad {
                healthy: 6,
                wounded: 0,
                xp: 0,
            },
        );
    engine = Engine::restore(&world, snapshot).unwrap();
    engine.execute(Engage("outlaws".into())).unwrap();
    assert!(engine.battle().is_some());
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(timed(Travel("greyford".into()), 60)),
        Err(EngineError::InEncounter)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn timed_commands_cross_the_wire_and_replay_exactly() {
    assert_eq!(
        serde_json::to_string(&timed(Rest, 480)).unwrap(),
        r#"{"timed":{"action":"rest","minutes":480}}"#
    );
    let journey = timed(Travel("ashmere".into()), 120);
    let wire = serde_json::to_string(&journey).unwrap();
    assert_eq!(
        wire,
        r#"{"timed":{"action":{"travel":"ashmere"},"minutes":120}}"#
    );
    assert_eq!(serde_json::from_str::<Command>(&wire).unwrap(), journey);

    let world = hosted(marches());
    let commands = [
        journey,
        PassTime(30),
        timed(Travel("greyford".into()), 90),
        timed(Rest, 600),
    ];
    let mut first = Engine::new_with_seed(&world, 7).unwrap();
    let mut second = Engine::new_with_seed(&world, 7).unwrap();
    for command in commands {
        assert_eq!(
            first.execute(command.clone()).unwrap(),
            second.execute(command).unwrap()
        );
    }
    assert_eq!(first.state(), second.state());
    assert_eq!(now(&first), 480 + 120 + 30 + 90 + 600);
    let restored = Engine::restore(&world, first.snapshot()).unwrap();
    assert_eq!(restored.state(), first.state());
}
