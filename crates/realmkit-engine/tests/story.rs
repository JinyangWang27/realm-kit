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
fn two_outcomes_at_once_refuse_the_command() {
    let mut world = chained();
    world.world.outcomes = vec![
        outcome("map_home", map_home()),
        outcome(
            "vault_open",
            Condition::Flag {
                flag: "vault_open".into(),
            },
        ),
    ];
    let mut engine = reader(&world);
    let error = solve_last_refused(&mut engine);
    assert!(
        matches!(error, EngineError::AmbiguousOutcome(ids) if ids == ["map_home", "vault_open"])
    );
    assert_eq!(engine.state().outcome, None);
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Ready);
}

/// Plays up to Pell's thanks, which must be refused.
fn solve_last_refused(engine: &mut Engine<'_>) -> EngineError {
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
    engine.execute(ChooseDialogue(1)).unwrap_err()
}

#[test]
fn no_outcome_may_hold_at_the_start() {
    let mut world = chained();
    world.world.outcomes = vec![outcome(
        "scholarly",
        Condition::Flag {
            flag: "river_scholar".into(),
        },
    )];
    assert!(matches!(
        Engine::start(&world, 0, &["scholar".into()]),
        Err(EngineError::OutcomeAtStart)
    ));
    assert!(Engine::start(&world, 0, &["reader".into()]).is_ok());
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
