//! Story phases and quest prerequisites at play.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn phase(id: &str) -> Phase {
    Phase {
        id: id.into(),
        name: id.into(),
    }
}

/// The archive in three phases; Pell's thanks skip straight to the last.
/// A second quest of Pell's needs the first done, or the last phase.
fn chained() -> WorldSpec {
    let mut w = archive();
    w.world.phases = vec![phase("visit"), phase("search"), phase("vault")];
    let pell = w.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    let request = &mut pell.nodes[1].choices[0].effects;
    request.push(Effect::EnterPhase {
        phase: "search".into(),
    });
    let thanks = &mut pell.nodes[0].choices[1].effects;
    thanks.push(Effect::EnterPhase {
        phase: "vault".into(),
    });
    thanks.push(Effect::AcceptQuest {
        quest: "catalogue".into(),
    });
    let mut second = w.quests[0].clone();
    second.id = "catalogue".into();
    second.requires = Some(Condition::Quest {
        quest: "lost_map".into(),
        status: QuestStatus::Completed,
    });
    w.quests.push(second);
    w
}

fn solve(engine: &mut Engine<'_>) -> Vec<Event> {
    for command in [
        Talk("archivist".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Move(North),
        Talk("copyist".into()),
        ChooseDialogue(1),
        Move(South),
        Talk("archivist".into()),
    ] {
        engine.execute(command).unwrap();
    }
    engine.execute(ChooseDialogue(1)).unwrap()
}

#[test]
fn phases_move_only_forward_through_story_effects() {
    let world = chained();
    let mut engine = reader(&world);
    assert_eq!(engine.state().phases, ["visit"]);
    let vault = Condition::Phase {
        phase: "vault".into(),
    };
    assert!(!engine.holds(&vault));
    let events = solve(&mut engine);
    assert!(events.contains(&Event::PhaseEntered {
        phase: "vault".into()
    }));
    // Passing through a phase reaches it too.
    assert_eq!(engine.state().phases, ["visit", "search", "vault"]);
    assert!(engine.holds(&Condition::Phase {
        phase: "search".into()
    }));
    assert!(engine.holds(&vault));
    let saved = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(saved.state(), engine.state());
}

#[test]
fn an_earlier_phase_changes_nothing() {
    let mut world = chained();
    // Asking again after the search began would go back a phase.
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[2].effects.push(Effect::EnterPhase {
        phase: "visit".into(),
    });
    let mut engine = reader(&world);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::PhaseEntered { .. })));
    assert_eq!(engine.state().phases, ["visit", "search"]);
}

#[test]
fn a_quest_waits_for_its_prerequisites() {
    let world = chained();
    let mut engine = reader(&world);
    let mut early = world.clone();
    early.quests[1].giver = "archivist".into();
    let mut eager = Engine::start(&early, 0, &["reader".into()]).unwrap();
    assert!(matches!(
        eager.execute(AcceptQuest("catalogue".into())),
        Err(EngineError::QuestLocked(id)) if id == "catalogue"
    ));
    let events = solve(&mut engine);
    assert!(events.contains(&Event::QuestAccepted {
        quest: "catalogue".into()
    }));
}

#[test]
fn saves_keep_to_phases_and_prerequisites() {
    let world = chained();
    let fresh = reader(&world).snapshot();
    let mut solved = reader(&world);
    solve(&mut solved);
    let solved = solved.snapshot();
    type Change = fn(&mut GameState);
    let bad: Vec<Change> = vec![
        |s| s.phases.clear(),
        |s| s.phases = vec!["search".into()],
        |s| s.phases.push("missing".into()),
        |s| s.phases.push("vault".into()),
        |s| {
            s.phases
                .extend(["search".into(), "vault".into(), "vault".into()])
        },
        // Taken up before the first quest was done.
        |s| {
            s.quests.insert("catalogue".into(), QuestStatus::Active);
        },
    ];
    for change in bad {
        let mut snapshot = fresh.clone();
        change(&mut snapshot.state);
        assert!(Engine::restore(&world, snapshot).is_err());
    }
    assert!(Engine::restore(&world, solved).is_ok());
}

fn outcome(id: &str, when: Condition) -> RouteOutcome {
    RouteOutcome {
        id: id.into(),
        name: id.into(),
        text: "The end.".into(),
        when,
    }
}

fn map_home() -> Condition {
    Condition::Quest {
        quest: "lost_map".into(),
        status: QuestStatus::Completed,
    }
}

#[test]
fn an_outcome_is_reached_once_and_play_goes_on() {
    let mut world = chained();
    world.world.outcomes = vec![outcome("map_home", map_home())];
    let mut engine = reader(&world);
    let events = solve(&mut engine);
    let reached = Event::OutcomeReached {
        outcome: "map_home".into(),
    };
    assert_eq!(events.last(), Some(&reached));
    assert_eq!(engine.state().outcome.as_deref(), Some("map_home"));
    // Play continues, and the outcome is never reached again.
    let more = engine.execute(Move(East)).unwrap();
    assert!(!more.contains(&reached));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn exclusive_outcomes_each_end_their_own_playthrough() {
    // The same deed ends differently for a former apprentice.
    let apprentice = || Condition::Flag {
        flag: "apprentice".into(),
    };
    let mut world = chained();
    world.world.outcomes = vec![
        outcome(
            "map_home",
            Condition::All {
                of: vec![
                    map_home(),
                    Condition::Not {
                        condition: Box::new(apprentice()),
                    },
                ],
            },
        ),
        outcome(
            "apprentice_home",
            Condition::All {
                of: vec![map_home(), apprentice()],
            },
        ),
    ];
    for (answer, ending) in [("reader", "map_home"), ("apprentice", "apprentice_home")] {
        let mut engine = Engine::start(&world, 0, &[answer.into()]).unwrap();
        let events = solve(&mut engine);
        assert!(events.contains(&Event::OutcomeReached {
            outcome: ending.into()
        }));
    }
}

#[test]
fn saves_record_an_outcome_exactly_when_one_was_reached() {
    let mut world = chained();
    world.world.outcomes = vec![outcome("map_home", map_home())];
    let mut solved = reader(&world);
    solve(&mut solved);
    let solved = solved.snapshot();
    let fresh = reader(&world).snapshot();
    let mut unrecorded = solved.clone();
    unrecorded.state.outcome = None;
    let mut unknown = solved.clone();
    unknown.state.outcome = Some("missing".into());
    let mut early = fresh.clone();
    early.state.outcome = Some("map_home".into());
    for snapshot in [unrecorded, unknown, early] {
        assert!(Engine::restore(&world, snapshot).is_err());
    }
}

#[test]
fn the_journal_shows_only_what_the_player_knows() {
    let mut world = chained();
    // The map is the main quest, though authored after its sequel.
    world.quests[0].main = true;
    world.quests.swap(0, 1);
    world.world.outcomes = vec![outcome("map_home", map_home())];
    let mut engine = reader(&world);
    let entry = |quest: &str, main, status| JournalQuest {
        quest: quest.into(),
        main,
        status,
    };
    // The catalogue waits on the map, so it is not listed yet.
    assert_eq!(
        engine.journal(),
        Journal {
            phase: Some("visit".into()),
            quests: vec![entry("lost_map", true, QuestStatus::Available)],
            evidence: vec![],
            outcome: None,
        }
    );
    solve(&mut engine);
    assert_eq!(
        engine.journal(),
        Journal {
            phase: Some("vault".into()),
            // Main quests first.
            quests: vec![
                entry("lost_map", true, QuestStatus::Completed),
                // Its objective, the map found, is already met.
                entry("catalogue", false, QuestStatus::Ready),
            ],
            evidence: vec!["stitched_map".into()],
            outcome: Some("map_home".into()),
        }
    );
}

#[test]
fn a_phase_behind_a_choice_this_playthrough_never_had_is_refused() {
    let mut world = chained();
    // Only a scholar's question to Pell moves the story to the search.
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[1].choices[0].effects.pop();
    let scholar = pell.nodes[0]
        .choices
        .iter_mut()
        .find(|c| c.text.starts_with("I study"))
        .unwrap();
    scholar.effects.push(Effect::EnterPhase {
        phase: "search".into(),
    });
    let mut reader = reader(&world).snapshot();
    reader.state.phases.push("search".into());
    assert!(Engine::restore(&world, reader).is_err());
    let mut scholar = Engine::start(&world, 0, &["scholar".into()])
        .unwrap()
        .snapshot();
    scholar.state.phases.push("search".into());
    assert!(Engine::restore(&world, scholar).is_ok());
}

#[test]
fn a_choice_that_accepts_a_locked_quest_waits_for_it() {
    // Pell offers the catalogue with no condition of her own; her thanks
    // no longer hand it over.
    let mut world = chained();
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[1].effects.pop();
    pell.nodes[0].choices.push(realmkit_spec::DialogueChoice {
        text: "Shall I catalogue the vault?".into(),
        next: None,
        requires: Some(Condition::Quest {
            quest: "catalogue".into(),
            status: QuestStatus::Available,
        }),
        effects: vec![Effect::AcceptQuest {
            quest: "catalogue".into(),
        }],
    });
    let offer = "Shall I catalogue the vault?";
    let mut engine = reader(&world);
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(!engine.dialogue_choices().contains(&offer));
    // Once the map is home the catalogue opens, and Pell offers it.
    solve(&mut engine);
    engine.execute(Talk("archivist".into())).unwrap();
    let choices = engine.dialogue_choices();
    let number = choices.iter().position(|c| *c == offer).unwrap() + 1;
    engine.execute(ChooseDialogue(number)).unwrap();
    assert_eq!(engine.state().quests["catalogue"], QuestStatus::Ready);
}
