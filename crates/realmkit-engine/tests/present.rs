//! Present facts are authored, bounded, deterministic and independent of time batching.
mod common;

use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn ford() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/turning-ford"
    ))
    .unwrap()
}

fn choose(engine: &mut Engine<'_>, who: &str) -> Vec<Event> {
    engine.execute(Talk(who.into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap()
}

fn changed(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    choose(&mut engine, "clerk");
    choose(&mut engine, "envoy");
    engine
}

#[test]
fn causal_chain_closes_one_route_and_opens_another_without_leaking_history() {
    let world = ford();
    let mut engine = changed(&world);
    assert_eq!(engine.state().control["ford"], "reed");
    let events = engine.execute(Wait(2880)).unwrap();
    assert_eq!(engine.state().quests["testimony"], QuestStatus::Closed);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::QuestClosed { .. }))
            .count(),
        1
    );
    assert_eq!(engine.character_location("clerk"), None);
    assert_eq!(engine.character_location("envoy"), Some("refuge"));
    assert_eq!(engine.state().departed["clerk"].minute, 2880);
    assert_eq!(engine.state().standing["trust"].get(None), Some(-2));
    assert_eq!(engine.chronicle().len(), 1);
    assert!(!engine
        .actions()
        .iter()
        .any(|a| matches!(&a.command, Talk(id) if id == "clerk" || id == "envoy")));
    let before = engine.snapshot();
    for command in [
        Talk("clerk".into()),
        CompleteQuest("testimony".into()),
        AcceptQuest("testimony".into()),
        Engage("clerk".into()),
    ] {
        assert!(engine.execute(command).is_err());
        assert_eq!(engine.snapshot(), before);
    }
    choose(&mut engine, "notice");
    assert_eq!(
        engine
            .chronicle()
            .iter()
            .map(|(e, _)| e.id.as_str())
            .collect::<Vec<_>>(),
        ["transfer", "dispersal"]
    );
    engine.execute(Move(Direction::East)).unwrap();
    choose(&mut engine, "keeper");
    choose(&mut engine, "keeper");
    assert_eq!(engine.state().quests["salvage"], QuestStatus::Completed);
    assert!(engine
        .execute(PassTime(1440))
        .unwrap()
        .iter()
        .all(|e| !matches!(e, Event::QuestClosed { .. })));
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn testimony_completed_before_the_loss_stays_completed() {
    let world = ford();
    let mut engine = changed(&world);
    choose(&mut engine, "clerk");
    engine.execute(Wait(2880)).unwrap();
    assert_eq!(engine.state().quests["testimony"], QuestStatus::Completed);
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn recording_again_keeps_the_first_minute_and_never_refuses_other_effects() {
    let mut world = ford();
    world.world.events.push(WorldEvent {
        id: "repeat".into(),
        schedule: Schedule {
            at: 10,
            every: Some(10),
        },
        requires: None,
        effects: vec![
            Effect::Record {
                entry: "transfer".into(),
            },
            Effect::SetFlag {
                flag: "heard".into(),
            },
        ],
    });
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Wait(20)).unwrap();
    assert_eq!(engine.state().chronicle.len(), 1);
    assert_eq!(engine.state().chronicle[0].minute, 10);
    assert!(engine.state().flags.contains("heard"));
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn pending_consequence_survives_save_reload_byte_for_byte() {
    let world = ford();
    let mut uninterrupted = changed(&world);
    uninterrupted.execute(Wait(1440)).unwrap();
    let bytes = serde_json::to_vec(&uninterrupted.snapshot()).unwrap();
    let mut restored = Engine::restore(&world, serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert_eq!(
        uninterrupted.execute(Wait(1440)).unwrap(),
        restored.execute(Wait(1440)).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&uninterrupted.snapshot()).unwrap(),
        serde_json::to_vec(&restored.snapshot()).unwrap()
    );
}

#[test]
fn every_cut_produces_a_byte_identical_snapshot_including_the_counter() {
    let mut world = ford();
    // A second elapsed chain and a fleeting closure at an empty minute,
    // between scheduled occurrences. Neither may be skipped by a long wait.
    world.world.chronicle.push(ChronicleEntry {
        id: "echo".into(),
        text: "News reaches the hills.".into(),
        known_when: None,
    });
    world.world.events.push(WorldEvent {
        id: "echo".into(),
        schedule: Schedule {
            at: 60,
            every: Some(60),
        },
        requires: Some(Condition::Elapsed {
            entry: "dispersal".into(),
            minutes: 60,
        }),
        effects: vec![Effect::Record {
            entry: "echo".into(),
        }],
    });
    world.quests[1].void_when = Some(Condition::TimeOfDay { from: 17, to: 18 });
    world.quests[1].void_reason = Some("The window passed.".into());
    for duration in [
        1, 16, 17, 18, 59, 60, 61, 1439, 1440, 2879, 2880, 2881, 2940, 3000,
    ] {
        let mut expected = changed(&world);
        expected.execute(Wait(duration)).unwrap();
        let bytes = serde_json::to_vec(&expected.snapshot()).unwrap();
        for cut in [1, 7, 59, 60, 601] {
            let mut engine = changed(&world);
            let mut left = duration;
            while left > 0 {
                let step = left.min(cut);
                engine.execute(PassTime(step)).unwrap();
                left -= step;
            }
            assert_eq!(
                serde_json::to_vec(&engine.snapshot()).unwrap(),
                bytes,
                "duration={duration}, cut={cut}"
            );
            Engine::restore(&world, engine.snapshot()).unwrap();
        }
    }
}

#[test]
fn new_save_fields_reject_unknown_ids_future_dates_and_inconsistent_states() {
    let world = ford();
    let mut engine = changed(&world);
    engine.execute(Wait(2880)).unwrap();
    type Corrupt = fn(&mut GameState);
    let cases: Vec<Corrupt> = vec![
        |s| {
            s.stations.insert("unknown".into(), "ford".into());
        },
        |s| {
            s.stations.insert("envoy".into(), "unknown".into());
        },
        |s| {
            s.departed.get_mut("clerk").unwrap().minute += 1;
        },
        |s| {
            s.departed.insert("you".into(), s.departed["clerk"].clone());
        },
        |s| {
            s.chronicle[0].entry = "unknown".into();
        },
        |s| {
            s.chronicle[0].minute = 3000;
        },
        |s| {
            s.chronicle.push(s.chronicle[0].clone());
        },
        |s| {
            s.chronicle.swap(0, 1);
        },
        |s| {
            s.control.insert("ford".into(), "unknown".into());
        },
        |s| {
            s.control.insert("unknown".into(), "reed".into());
        },
        |s| {
            s.control.clear();
        },
        |s| {
            s.quests.insert("testimony".into(), QuestStatus::Active);
        },
        |s| {
            s.quests.insert("salvage".into(), QuestStatus::Closed);
        },
    ];
    for (index, corrupt) in cases.into_iter().enumerate() {
        let mut save = engine.snapshot();
        corrupt(&mut save.state);
        assert!(Engine::restore(&world, save).is_err(), "corruption {index}");
    }
}

#[test]
fn fate_is_irreversible_and_first_reason_and_minute_are_kept() {
    let mut world = ford();
    world.world.events.extend([
        WorldEvent {
            id: "again".into(),
            schedule: Schedule {
                at: 2881,
                every: None,
            },
            requires: None,
            effects: vec![Effect::Depart {
                character: "clerk".into(),
                reason: DepartureReason::Dead,
            }],
        },
        WorldEvent {
            id: "cannot_return".into(),
            schedule: Schedule {
                at: 2882,
                every: None,
            },
            requires: None,
            effects: vec![
                Effect::SetFlag {
                    flag: "testified".into(),
                },
                Effect::Relocate {
                    character: "clerk".into(),
                    to: "refuge".into(),
                },
            ],
        },
    ]);
    let mut engine = changed(&world);
    engine.execute(Wait(2882)).unwrap();
    assert_eq!(
        engine.state().departed["clerk"],
        Departure {
            reason: DepartureReason::Exiled,
            minute: 2880
        }
    );
    assert!(!engine.state().flags.contains("testified"));
    assert!(engine.holds(&Condition::Departed {
        character: "clerk".into()
    }));
    assert!(engine.holds(&Condition::Elapsed {
        entry: "dispersal".into(),
        minutes: 2
    }));
    assert!(!engine.holds(&Condition::Elapsed {
        entry: "dispersal".into(),
        minutes: 3
    }));
    assert!(engine.holds(&Condition::Controls {
        location: "ford".into(),
        faction: "reed".into()
    }));
}

#[test]
fn departed_merchants_close_the_market_and_fight_saves_cannot_contain_departures() {
    let mut world = common::marches();
    let start = world.world.time.as_ref().unwrap().start;
    world.world.events.push(WorldEvent {
        id: "hild_leaves".into(),
        schedule: Schedule {
            at: start + 1,
            every: None,
        },
        requires: None,
        effects: vec![Effect::Depart {
            character: "hild".into(),
            reason: DepartureReason::Left,
        }],
    });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Market).unwrap();
    engine.execute(Wait(1)).unwrap();
    assert!(engine.execute(Market).is_err());
    assert!(engine
        .execute(Buy {
            good: "wool".into(),
            quantity: 1
        })
        .is_err());
    Engine::restore(&world, engine.snapshot()).unwrap();

    let world = common::demo();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(Direction::North)).unwrap();
    engine.execute(HostFight("wolf".into())).unwrap();
    let mut save = engine.snapshot();
    save.state.departed.insert(
        "wolf".into(),
        Departure {
            reason: DepartureReason::Destroyed,
            minute: 0,
        },
    );
    assert!(Engine::restore(&world, save).is_err());
}

#[test]
fn hosted_rest_uses_the_same_evolution_as_minutes_in_a_fully_rested_party() {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/hollow-ford"
    ))
    .unwrap();
    let time = world.world.time.as_mut().unwrap();
    time.hosted = true;
    time.wait = None;
    time.rest = None;
    for road in &mut world.world.roads {
        road.minutes = 0;
    }
    world.world.chronicle.push(ChronicleEntry {
        id: "news".into(),
        text: "A letter arrives.".into(),
        known_when: None,
    });
    world.world.events.push(WorldEvent {
        id: "news".into(),
        schedule: Schedule {
            at: 490,
            every: None,
        },
        requires: None,
        effects: vec![Effect::Record {
            entry: "news".into(),
        }],
    });
    let mut timed = Engine::new(&world).unwrap();
    let mut split = Engine::new(&world).unwrap();
    timed
        .execute(Timed {
            action: Box::new(Rest),
            minutes: 60,
        })
        .unwrap();
    for _ in 0..60 {
        split.execute(PassTime(1)).unwrap();
    }
    assert_eq!(
        serde_json::to_vec(&timed.snapshot()).unwrap(),
        serde_json::to_vec(&split.snapshot()).unwrap()
    );
}

#[test]
fn closure_observes_each_occurrence_even_when_the_next_one_reverses_control() {
    let mut world = ford();
    world.quests[0].void_when = Some(Condition::Controls {
        location: "ford".into(),
        faction: "reed".into(),
    });
    world.world.events = vec![
        WorldEvent {
            id: "take".into(),
            schedule: Schedule { at: 1, every: None },
            requires: None,
            effects: vec![Effect::SetControl {
                location: "ford".into(),
                faction: "reed".into(),
            }],
        },
        WorldEvent {
            id: "retake".into(),
            schedule: Schedule { at: 1, every: None },
            requires: None,
            effects: vec![Effect::SetControl {
                location: "ford".into(),
                faction: "stone".into(),
            }],
        },
    ];
    let mut engine = Engine::new(&world).unwrap();
    let events = engine.execute(Wait(60)).unwrap();
    assert_eq!(engine.state().control["ford"], "stone");
    assert_eq!(engine.state().quests["testimony"], QuestStatus::Closed);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::QuestClosed { .. }))
            .count(),
        1
    );
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn relocated_group_members_use_the_same_resolution_as_engagement_and_actions() {
    let mut world = common::arena();
    world.world.time =
        Some(serde_json::from_str(r#"{"start":0,"wait":1,"clock":"{hour}:{minute}"}"#).unwrap());
    let destination = world.world.start.clone();
    world.world.events.push(WorldEvent {
        id: "wolves_arrive".into(),
        schedule: Schedule { at: 1, every: None },
        requires: None,
        effects: vec![
            Effect::Relocate {
                character: "grey_wolf".into(),
                to: destination.clone(),
            },
            Effect::Relocate {
                character: "black_wolf".into(),
                to: destination,
            },
        ],
    });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Wait(1)).unwrap();
    assert!(engine
        .actions()
        .iter()
        .any(|a| a.command == Engage("grey_wolf".into())));
    let events = engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::EncounterStarted { opponents } if opponents.len() == 2 && opponents.contains(&"black_wolf".into()))));
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn closure_observes_the_answered_start_not_the_unanswered_intermediate_state() {
    let mut world = ford();
    world.world.start_questions.push(StartQuestion {
        id: "news".into(),
        name: "News".into(),
        text: "Have you heard?".into(),
        options: vec![StartOption {
            id: "yes".into(),
            text: "Yes.".into(),
            effects: vec![Effect::SetFlag {
                flag: "heard".into(),
            }],
        }],
    });
    world.quests[0].void_when = Some(Condition::Not {
        condition: Box::new(Condition::Flag {
            flag: "heard".into(),
        }),
    });
    let engine = Engine::start(&world, 7, &["yes".into()]).unwrap();
    assert_eq!(engine.state().quests["testimony"], QuestStatus::Available);
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn chronicle_query_revealed_later_returns_the_original_recorded_minute() {
    let world = ford();
    let mut engine = changed(&world);
    engine.execute(Wait(2880)).unwrap();
    assert!(!engine.chronicle().iter().any(|(e, _)| e.id == "dispersal"));
    engine.execute(Wait(123)).unwrap();
    choose(&mut engine, "notice");
    assert_eq!(engine.state().time, Some(3003));
    assert_eq!(
        engine
            .chronicle()
            .iter()
            .find(|(e, _)| e.id == "dispersal")
            .unwrap()
            .1,
        2880
    );
    let restored = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(restored.chronicle(), engine.chronicle());
}

#[test]
fn public_location_query_resolves_static_remote_moving_relocated_and_absent_actors() {
    let world = ford();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(engine.character_location("clerk"), Some("ford"));
    assert_eq!(engine.character_location("keeper"), Some("refuge"));
    assert_eq!(engine.character_location("unknown"), None);
    assert_eq!(engine.character_location("you"), None); // player uses state.player.location
    choose(&mut engine, "envoy");
    engine.execute(Wait(2880)).unwrap();
    assert_eq!(engine.character_location("clerk"), None);
    assert_eq!(engine.character_location("envoy"), Some("refuge"));
    assert_eq!(
        engine.state().departed["clerk"].reason,
        DepartureReason::Exiled
    );

    let world = common::marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(
        engine.character_location("wenna"),
        engine.state().whereabouts.get("wenna").map(String::as_str)
    );
    engine.execute(Wait(1440)).unwrap();
    assert_eq!(
        engine.character_location("wenna"),
        engine.state().whereabouts.get("wenna").map(String::as_str)
    );
}

#[test]
fn location_query_is_independent_of_visibility_and_prefers_a_local_static_placement() {
    let mut world = ford();
    // This fixture deliberately duplicates placement; remove the departure rule
    // whose target must, separately, have unique placement.
    world.world.events.clear();
    world
        .characters
        .iter_mut()
        .find(|c| c.id == "clerk")
        .unwrap()
        .requires = Some(Condition::Flag {
        flag: "heard".into(),
    });
    world
        .locations
        .iter_mut()
        .find(|l| l.id == "refuge")
        .unwrap()
        .characters
        .push("clerk".into());
    let mut engine = Engine::new(&world).unwrap();
    assert!(!engine.present_here().iter().any(|c| c.id == "clerk"));
    assert_eq!(engine.character_location("clerk"), Some("ford"));
    engine.execute(Move(Direction::East)).unwrap();
    assert_eq!(engine.character_location("clerk"), Some("refuge"));
}
