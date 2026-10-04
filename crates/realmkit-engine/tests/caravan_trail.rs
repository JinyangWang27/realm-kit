//! The caravan slice: from the caravan's arrival at Thornwick,
//! through The Lost Wagons, to choosing a lead on The Buried Road.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

/// Commands in a walkthrough's words, separated by `;`.
fn script(text: &str) -> Vec<Command> {
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|step| {
            let (verb, arg) = step.split_once(' ').unwrap_or((step, ""));
            match verb {
                "talk" | "examine" => Talk(arg.into()),
                "choose" => ChooseDialogue(arg.parse().unwrap()),
                "travel" => Travel(arg.into()),
                "engage" => Engage(arg.into()),
                "attack" => Attack(arg.into()),
                "rest" => Rest,
                "wait" => Wait(arg.parse().unwrap()),
                _ => panic!("unknown step {step}"),
            }
        })
        .collect()
}

fn run(engine: &mut Engine<'_>, text: &str) -> Vec<Event> {
    script(text)
        .into_iter()
        .flat_map(|c| {
            engine
                .execute(c.clone())
                .unwrap_or_else(|e| panic!("{c:?}: {e}"))
        })
        .collect()
}

/// The caravan arrives, the ostler's bales are carried in, and the evening
/// brings word that the Hadda caravan is overdue: Dravin's quest is taken.
const OPENING: &str = "talk iselt; choose 1; travel thornwick; examine city_gate; choose 1;
    talk iselt; choose 1; talk moss; choose 1; examine caravan_bales; choose 1;
    talk moss; choose 1; rest; talk dravin; choose 1; choose 1";
const CAMP: &str = "travel abandoned_camp; examine cold_camp; choose 1";
const FIGHT: &str = "travel bandit_ridge; engage rask; attack lookout; attack lookout;
    attack rask; attack rask; attack rask; attack rask";
/// The paved branch, from the camp or the ridge.
const FORK: &str = "travel impossible_fork; examine fork_stones; choose 1";
/// From the fork: the ring, then home to Dravin.
const RING: &str = "travel old_stones; examine stone_ring; choose 1; travel impossible_fork; travel abandoned_camp;
    travel thornwick; talk dravin; choose 1";
/// Two of the three readings, then The Buried Road and one lead.
const READINGS: &str = "talk ama; choose 1; talk garrow; choose 1";
const BURIED_ROAD: &str = "talk tenko; choose 2; talk ama; choose 1";

fn start<'w>(world: &'w WorldSpec, background: &str) -> Engine<'w> {
    Engine::start(world, 7, &[background.into()]).unwrap()
}

fn known(engine: &Engine<'_>) -> Vec<String> {
    let view = engine.map_view().unwrap();
    view.places.into_iter().map(|p| p.location).collect()
}

fn reached(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::OutcomeReached { .. }))
        .count()
}

#[test]
fn the_golden_path_fights_and_closes_the_chapter() {
    let world = caravan_trail();
    let mut engine = start(&world, "caravan_guard");
    let mut events = run(&mut engine, OPENING);
    events.extend(run(&mut engine, &format!("{CAMP}; {FIGHT}")));
    assert!(engine.state().flags.contains("bandits_beaten"));
    events.extend(run(
        &mut engine,
        &format!("examine bandit_spoils; choose 1; {FORK}; {RING}; {READINGS}; {BURIED_ROAD}"),
    ));
    let state = engine.state();
    assert_eq!(state.phases, ["road", "thornwick", "lost_wagons", "leads"]);
    assert_eq!(state.outcome.as_deref(), Some("first_chapter"));
    assert_eq!(reached(&events), 1);
    assert_eq!(state.quests["lost_wagons"], QuestStatus::Completed);
    assert_eq!(state.quests["buried_road"], QuestStatus::Ready);
    // Each phase was announced once, in order.
    let phases: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::PhaseEntered { phase } => Some(phase.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(phases, ["thornwick", "lost_wagons", "leads"]);
}

#[test]
fn the_same_seed_choices_and_commands_replay_identically() {
    let world = caravan_trail();
    let play = || {
        let mut engine = start(&world, "caravan_guard");
        let all = format!("{OPENING}; {CAMP}; {FIGHT}; examine bandit_spoils; choose 1; {FORK}; {RING}; {READINGS}; {BURIED_ROAD}");
        let events = run(&mut engine, &all);
        (events, engine.state().clone())
    };
    assert_eq!(play(), play());
}

#[test]
fn bribery_intimidation_and_the_survivor_each_find_the_ring() {
    let world = caravan_trail();
    // The merchant's child pays Rask from the silver they started with.
    let mut merchant = start(&world, "merchant_child");
    run(
        &mut merchant,
        &format!("{OPENING}; {CAMP}; travel bandit_ridge; talk rask; choose 1; choose 1"),
    );
    assert!(merchant.state().evidence.contains("bandit_testimony"));
    assert!(merchant.state().flags.contains("bandits_paid"));
    assert!(merchant.present_here().is_empty());
    // The guard needs no silver: a look is enough.
    let mut guard = start(&world, "caravan_guard");
    run(
        &mut guard,
        &format!("{OPENING}; {CAMP}; travel bandit_ridge; talk rask; choose 2; choose 1"),
    );
    assert!(guard.state().evidence.contains("bandit_testimony"));
    assert!(guard.state().flags.contains("bandits_cowed"));
    // The scribe skips the ridge entirely and hears the survivor instead.
    let mut scribe = start(&world, "temple_scribe");
    run(
        &mut scribe,
        &format!("{OPENING}; {CAMP}; {FORK}; talk ennick; choose 1"),
    );
    for mut engine in [merchant, guard, scribe] {
        // The bribe and the threat reach the fork by the ridge's own road.
        if engine.state().player.location == "bandit_ridge" {
            run(&mut engine, FORK);
        }
        run(&mut engine, &format!("{RING}; {READINGS}; {BURIED_ROAD}"));
        assert_eq!(engine.state().outcome.as_deref(), Some("first_chapter"));
        assert!(!engine
            .state()
            .combat
            .as_ref()
            .unwrap()
            .defeated
            .contains("rask"));
    }
}

#[test]
fn the_start_choice_shapes_play_and_survives_saves() {
    let world = caravan_trail();
    let merchant = start(&world, "merchant_child");
    assert_eq!(merchant.state().start_choices, ["merchant_child"]);
    assert_eq!(merchant.state().player.inventory.get("silver"), Some(&2));
    let saved = Engine::restore(&world, merchant.snapshot()).unwrap();
    assert_eq!(saved.state(), merchant.state());
    // Only a guard may threaten Rask; anyone with two silver may pay him.
    let mut scribe = start(&world, "temple_scribe");
    run(
        &mut scribe,
        &format!("{OPENING}; {CAMP}; travel bandit_ridge; talk rask"),
    );
    assert_eq!(
        scribe.dialogue_choices(),
        [
            "Two silver for what you know about the Hadda caravan.",
            "Walk away."
        ]
    );
}

#[test]
fn the_main_quest_waits_for_arrival_and_the_evening() {
    let world = caravan_trail();
    let mut engine = start(&world, "caravan_guard");
    run(&mut engine, "talk iselt; choose 1; travel thornwick");
    let locked = |engine: &mut Engine<'_>| match engine.execute(AcceptQuest("lost_wagons".into())) {
        Err(EngineError::QuestLocked(_)) => true,
        Ok(_) => false,
        Err(other) => panic!("{other}"),
    };
    assert!(locked(&mut engine));
    let journal = engine.journal();
    assert!(!journal.quests.iter().any(|q| q.quest == "lost_wagons"));
    // Through the gate and delivered, but the caravan is not overdue yet.
    run(
        &mut engine,
        "examine city_gate; choose 1; talk iselt; choose 1",
    );
    assert_eq!(engine.state().phases.last().unwrap(), "thornwick");
    assert!(locked(&mut engine));
    // World time brings the news; it moves no story phase.
    let before = engine.state().time.unwrap();
    run(&mut engine, "wait 480");
    assert!(engine.state().time.unwrap() > before);
    assert!(engine.state().flags.contains("caravan_overdue"));
    assert_eq!(engine.state().phases.last().unwrap(), "thornwick");
    assert!(!locked(&mut engine));
}

#[test]
fn travel_takes_world_time_and_rest_does_not_move_the_story() {
    let world = caravan_trail();
    let mut engine = start(&world, "caravan_guard");
    assert_eq!(engine.state().time, Some(360));
    run(&mut engine, "talk iselt; choose 1; travel thornwick");
    assert_eq!(engine.state().time, Some(600));
    let phases = engine.state().phases.clone();
    run(&mut engine, "rest; wait 60");
    assert_eq!(engine.state().time, Some(1140));
    assert_eq!(engine.state().phases, phases);
}

#[test]
fn the_map_shows_places_as_the_player_learns_of_them() {
    let world = caravan_trail();
    let mut engine = start(&world, "temple_scribe");
    assert_eq!(known(&engine), ["south_road", "thornwick"]);
    run(&mut engine, OPENING);
    assert_eq!(
        known(&engine),
        ["south_road", "thornwick", "abandoned_camp", "bandit_ridge"]
    );
    run(&mut engine, "travel abandoned_camp");
    assert!(matches!(
        engine.execute(Travel("impossible_fork".into())),
        Err(EngineError::NoRoad(_))
    ));
    run(&mut engine, "examine cold_camp; choose 1");
    assert!(known(&engine).contains(&"impossible_fork".to_string()));
    run(
        &mut engine,
        "travel impossible_fork; examine fork_stones; choose 1",
    );
    // The fork alone does not say where the wagons went.
    assert!(!known(&engine).contains(&"old_stones".to_string()));
    run(&mut engine, "talk ennick; choose 1");
    assert_eq!(known(&engine).len(), 6);
    let view = engine.map_view().unwrap();
    assert!(view
        .roads
        .iter()
        .any(|r| r.road == "old-way" && r.minutes == 120));
}

#[test]
fn carrying_the_seal_is_not_knowing_what_it_means() {
    let world = caravan_trail();
    let mut engine = start(&world, "caravan_guard");
    run(
        &mut engine,
        &format!("{OPENING}; {CAMP}; {FIGHT}; examine bandit_spoils; choose 2"),
    );
    assert_eq!(
        engine.state().player.inventory.get("caravan_seal"),
        Some(&1)
    );
    assert!(!engine.state().evidence.contains("displaced_cargo"));
    assert_eq!(
        world.evidence("displaced_cargo").unwrap().item.as_deref(),
        Some("caravan_seal")
    );
    run(&mut engine, "examine bandit_spoils; choose 1");
    assert!(engine.state().evidence.contains("displaced_cargo"));
}

#[test]
fn buried_road_needs_two_readings() {
    let world = caravan_trail();
    let mut engine = start(&world, "caravan_guard");
    run(
        &mut engine,
        &format!("{OPENING}; {CAMP}; {FORK}; talk ennick; choose 1; {RING}"),
    );
    run(&mut engine, "talk ama; choose 1; talk tenko");
    assert!(!engine
        .dialogue_choices()
        .contains(&"Where does all this lead?"));
    assert!(matches!(
        engine.execute(AcceptQuest("buried_road".into())),
        Err(EngineError::QuestLocked(_))
    ));
    // Any second reading will do.
    run(&mut engine, "choose 1; talk tenko");
    assert_eq!(
        engine.dialogue_choices(),
        ["Where does all this lead?", "Good day."]
    );
}

#[test]
fn unchosen_leads_stay_open_after_the_chapter_closes() {
    let world = caravan_trail();
    let mut engine = start(&world, "temple_scribe");
    run(
        &mut engine,
        &format!("{OPENING}; {CAMP}; {FORK}; talk ennick; choose 1; {RING}; {READINGS}"),
    );
    let before = engine.snapshot();
    run(&mut engine, BURIED_ROAD);
    let state = engine.state();
    assert_eq!(state.quests["lead_shrines"], QuestStatus::Active);
    for lead in ["lead_milestones", "lead_hill_folk"] {
        assert_eq!(state.quests[lead], QuestStatus::Available, "{lead}");
        assert!(engine.journal().quests.iter().any(|q| q.quest == lead));
    }
    // Another lead can still be taken, and the outcome is not reached twice.
    let events = run(&mut engine, "talk garrow; choose 1");
    assert_eq!(engine.state().quests["lead_hill_folk"], QuestStatus::Active);
    assert_eq!(reached(&events), 0);
    assert_eq!(engine.state().outcome.as_deref(), Some("first_chapter"));
    // Loading the earlier save branches: a different lead, the same ending.
    let mut branch = Engine::restore(&world, before).unwrap();
    let events = run(&mut branch, "talk tenko; choose 2; talk tenko; choose 2");
    assert_eq!(
        branch.state().quests["lead_milestones"],
        QuestStatus::Active
    );
    assert_eq!(
        branch.state().quests["lead_shrines"],
        QuestStatus::Available
    );
    assert_eq!(reached(&events), 1);
}

#[test]
fn saves_at_the_camp_mid_fight_and_after_the_ending_resume_exactly() {
    let world = caravan_trail();
    let all = script(&format!(
        "{OPENING}; {CAMP}; {FIGHT}; examine bandit_spoils; choose 1; {FORK}; {RING}; {READINGS}; {BURIED_ROAD}"
    ));
    let mut straight = start(&world, "caravan_guard");
    let expected: Vec<Vec<Event>> = all
        .iter()
        .map(|c| straight.execute(c.clone()).unwrap())
        .collect();
    let mut engine = start(&world, "caravan_guard");
    for (i, command) in all.iter().enumerate() {
        // Save and reload through JSON at every step, mid-fight included.
        let bytes = engine.snapshot().to_json();
        engine = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
        assert_eq!(
            engine.execute(command.clone()).unwrap(),
            expected[i],
            "{command:?}"
        );
    }
    assert_eq!(engine.state(), straight.state());
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}
