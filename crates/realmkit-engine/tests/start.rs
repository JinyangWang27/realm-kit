//! Start questions: answers applied before the first turn, and saved.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn answer<'w>(world: &'w WorldSpec, choice: &str) -> Engine<'w> {
    Engine::start(world, 0, &[choice.into()]).unwrap()
}

#[test]
fn an_answer_shapes_the_start_before_the_first_turn() {
    let world = archive();
    let mut engine = answer(&world, "apprentice");
    let state = engine.state();
    assert_eq!(state.turn, 0);
    assert_eq!(state.start_choices, ["apprentice"]);
    assert!(state.flags.contains("apprentice") && state.flags.contains("pen_borrowed"));
    assert_eq!(state.player.inventory.get("pen"), Some(&1));
    // Later conditions read the answer like any other state.
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    let choices = texts(&engine);
    assert!(choices.contains(&"I was apprenticed to a copyist too.".into()));
    assert!(!choices.contains(&"Could I borrow a pen?".into()));
    // Another answer leaves the start as authored.
    let reader = reader(&world);
    assert!(reader.state().flags.is_empty());
    assert!(reader.state().player.inventory.is_empty());
}

#[test]
fn every_question_needs_one_of_its_options() {
    let world = archive();
    for choices in [
        vec![],
        vec!["nobody".into()],
        vec!["reader".into(), "reader".into()],
    ] {
        assert!(
            matches!(
                Engine::start(&world, 0, &choices),
                Err(EngineError::StartChoices)
            ),
            "{choices:?}"
        );
    }
    assert!(matches!(
        Engine::new(&world),
        Err(EngineError::StartChoices)
    ));
    // A world that asks nothing takes no answers.
    assert!(matches!(
        Engine::start(&demo(), 0, &["reader".into()]),
        Err(EngineError::StartChoices)
    ));
}

#[test]
fn saves_keep_the_answers_and_reject_forged_ones() {
    let world = archive();
    let engine = answer(&world, "scholar");
    let resumed = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(resumed.state(), engine.state());
    for forged in [
        vec![],
        vec!["nobody".into()],
        vec!["scholar".into(), "reader".into()],
    ] {
        let mut snapshot = engine.snapshot();
        snapshot.state.start_choices = forged;
        assert!(matches!(
            Engine::restore(&world, snapshot),
            Err(EngineError::InvalidSave(_))
        ));
    }
    // A flag only another answer sets could not have been set.
    let mut snapshot = engine.snapshot();
    snapshot.state.flags.insert("apprentice".into());
    assert!(Engine::restore(&world, snapshot.clone()).is_err());
    snapshot.state.start_choices = vec!["apprentice".into()];
    snapshot.state.flags.remove("river_scholar");
    // An answer's flags are never cleared, so claiming it without them fails.
    assert!(Engine::restore(&world, snapshot.clone()).is_err());
    snapshot.state.flags.insert("pen_borrowed".into());
    assert!(Engine::restore(&world, snapshot).is_ok());
}

#[test]
fn an_answer_that_opens_a_breakthrough_gate_starts_past_it() {
    let mut world = sect();
    let mut grant = world.combat().unwrap().player_techniques[0].clone();
    grant.xp = 30;
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .player_techniques
        .clear();
    world.world.start_questions = vec![StartQuestion {
        id: "past".into(),
        name: "Past".into(),
        text: "What did you study?".into(),
        options: vec![StartOption {
            id: "scripture".into(),
            text: "The breath scripture, to its third layer.".into(),
            effects: vec![
                Effect::GrantTechnique(grant),
                Effect::SetFlag {
                    flag: "scripture_found".into(),
                },
            ],
        }],
    }];
    let engine = Engine::start(&world, 0, &["scripture".into()]).unwrap();
    assert_eq!(combat(&engine).techniques["azure_breath"].rank, 3);
    // The third layer's MP is there from the start, full.
    let vitals = engine.player_vitals().unwrap();
    assert_eq!(vitals.mp, engine.player_stats().unwrap().mp);
    // A technique an answer taught must stay known in saves.
    let mut snapshot = engine.snapshot();
    snapshot.state.combat.as_mut().unwrap().techniques.clear();
    assert!(Engine::restore(&world, snapshot).is_err());
}

#[test]
fn a_technique_granted_at_the_start_fills_the_mp_its_passive_gives() {
    let mut world = sect();
    let grant = world.combat().unwrap().player_techniques[0].clone();
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .player_techniques
        .clear();
    world.world.start_questions = vec![StartQuestion {
        id: "past".into(),
        name: "Past".into(),
        text: "Who taught you?".into(),
        options: vec![
            StartOption {
                id: "taught".into(),
                text: "A master.".into(),
                effects: vec![Effect::GrantTechnique(grant)],
            },
            StartOption {
                id: "untaught".into(),
                text: "Nobody.".into(),
                effects: vec![],
            },
        ],
    }];
    let taught = Engine::start(&world, 0, &["taught".into()]).unwrap();
    let untaught = Engine::start(&world, 0, &["untaught".into()]).unwrap();
    assert_eq!(
        taught.player_vitals().unwrap().mp,
        taught.player_stats().unwrap().mp
    );
    assert!(taught.player_vitals().unwrap().mp > untaught.player_vitals().unwrap().mp);
    assert!(!combat(&taught).techniques.is_empty());
    assert!(combat(&untaught).techniques.is_empty());
}
