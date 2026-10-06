//! Diplomacy between factions and the player's standing on authored tracks.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

const TERMS: &str = "Let me carry terms to the fen camps.";
const TEST: &str = "Test.";

fn pair(a: &str, b: &str) -> [Id; 2] {
    [a.into(), b.into()]
}

fn at_war(a: &str, b: &str) -> Condition {
    Condition::AtWar {
        factions: pair(a, b),
    }
}

fn change(track: &str, faction: Option<&str>, by: i32) -> Effect {
    Effect::ChangeStanding {
        track: track.into(),
        faction: faction.map(Into::into),
        by,
    }
}

/// The marches with the keep already trusting the player, so the steward
/// offers terms, and one more steward choice applying `effects`.
fn trusted(effects: Vec<Effect>) -> WorldSpec {
    let mut world = marches();
    let favour = world.world.standing.iter_mut().find(|t| t.id == "favour");
    favour.unwrap().starts.insert("keep".into(), 20);
    let steward = world.dialogues.iter_mut().find(|d| d.id == "steward");
    steward.unwrap().nodes[0].choices.push(DialogueChoice {
        text: TEST.into(),
        effects,
        ..Default::default()
    });
    assert_eq!(world.diagnostics(), []);
    world
}

/// Talks to the steward at Hollin Keep, reached at noon on the first day.
fn at_the_keep(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    engine.execute(Travel("hollin_keep".into())).unwrap();
    engine.execute(Talk("steward".into())).unwrap();
    engine
}

fn choose(engine: &mut Engine<'_>, text: &str) -> Result<Vec<Event>, EngineError> {
    let choices = engine.dialogue_choices();
    let number = choices.iter().position(|c| c.text == text).unwrap() + 1;
    engine.execute(ChooseDialogue(number))
}

fn standing(engine: &Engine<'_>, track: &str, faction: Option<&str>) -> i32 {
    engine.state().standing[track].get(faction).unwrap()
}

/// Only the faction events, in order.
fn changes(events: Vec<Event>) -> Vec<Event> {
    events
        .into_iter()
        .filter(|e| {
            matches!(
                e,
                Event::StandingChanged { .. } | Event::WarDeclared { .. } | Event::PeaceMade { .. }
            )
        })
        .collect()
}

#[test]
fn diplomacy_and_standing_start_as_authored() {
    let world = marches();
    let engine = Engine::new(&world).unwrap();
    // Authored as keep and fen; kept in one order, read in either.
    assert_eq!(engine.state().at_war, [pair("fen", "keep")].into());
    assert!(engine.holds(&at_war("keep", "fen")));
    assert!(engine.holds(&at_war("fen", "keep")));
    assert!(!engine.holds(&at_war("fen", "guild")));
    assert!(!engine.holds(&at_war("guild", "keep")));
    assert_eq!(engine.state().standing["renown"], Standing::Global(0));
    assert_eq!(
        engine.state().standing["favour"],
        Standing::Factions([("fen".into(), -40), ("guild".into(), 0), ("keep".into(), 0)].into())
    );
}

#[test]
fn terms_make_peace_and_conditions_follow() {
    let world = trusted(vec![]);
    let mut engine = at_the_keep(&world);
    let outlaws = world.character("outlaws").unwrap().requires.clone();
    assert!(engine.allows(outlaws.as_ref()));
    let events = changes(choose(&mut engine, TERMS).unwrap());
    assert_eq!(
        events,
        [
            Event::PeaceMade {
                factions: pair("fen", "keep")
            },
            Event::StandingChanged {
                track: "favour".into(),
                faction: Some("fen".into()),
                by: 30,
                value: -10,
            },
            Event::StandingChanged {
                track: "renown".into(),
                faction: None,
                by: 15,
                value: 15,
            },
        ]
    );
    // Only the fen's favour moved; the keep's and the guild's stay.
    assert_eq!(standing(&engine, "favour", Some("keep")), 20);
    assert_eq!(standing(&engine, "favour", Some("guild")), 0);
    // At peace, the outlaws are gone and the steward has no terms to offer.
    assert!(engine.state().at_war.is_empty());
    assert!(!engine.holds(&at_war("fen", "keep")));
    assert!(!engine.allows(outlaws.as_ref()));
    engine.execute(Talk("steward".into())).unwrap();
    assert!(engine.dialogue_choices().iter().all(|c| c.text != TERMS));
}

#[test]
fn standing_gates_choices() {
    // Without the keep's trust, the steward hides the terms.
    let world = marches();
    let engine = at_the_keep(&world);
    assert!(engine.dialogue_choices().iter().all(|c| c.text != TERMS));
    let favour = Condition::Standing {
        track: "favour".into(),
        faction: Some("keep".into()),
        at_least: 20,
    };
    assert!(!engine.holds(&favour));
    let world = trusted(vec![]);
    assert!(at_the_keep(&world).holds(&favour));
}

#[test]
fn war_and_peace_change_once_in_either_order() {
    let world = trusted(vec![
        Effect::DeclareWar {
            factions: pair("keep", "guild"),
        },
        Effect::DeclareWar {
            factions: pair("guild", "keep"),
        },
        Effect::MakePeace {
            factions: pair("guild", "keep"),
        },
        Effect::MakePeace {
            factions: pair("keep", "guild"),
        },
        Effect::DeclareWar {
            factions: pair("guild", "keep"),
        },
    ]);
    let mut engine = at_the_keep(&world);
    let war = Event::WarDeclared {
        factions: pair("guild", "keep"),
    };
    let peace = Event::PeaceMade {
        factions: pair("guild", "keep"),
    };
    assert_eq!(
        changes(choose(&mut engine, TEST).unwrap()),
        [war.clone(), peace, war]
    );
    assert_eq!(
        engine.state().at_war,
        [pair("fen", "keep"), pair("guild", "keep")].into()
    );
    assert!(engine.holds(&at_war("keep", "guild")));
}

#[test]
fn standing_clamps_to_its_bounds() {
    let world = trusted(vec![
        change("renown", None, 600),
        change("renown", None, 600),
        change("renown", None, 600),
        change("favour", Some("fen"), -100),
        change("renown", None, -1_000),
        change("renown", None, -1_000),
    ]);
    let mut engine = at_the_keep(&world);
    let changed = |track: &str, faction: Option<&str>, by, value| Event::StandingChanged {
        track: track.into(),
        faction: faction.map(Into::into),
        by,
        value,
    };
    // A change the bound absorbs entirely reports nothing.
    assert_eq!(
        changes(choose(&mut engine, TEST).unwrap()),
        [
            changed("renown", None, 600, 600),
            changed("renown", None, 400, 1_000),
            changed("favour", Some("fen"), -60, -100),
            changed("renown", None, -1_000, 0),
        ]
    );
    assert_eq!(standing(&engine, "renown", None), 0);
    assert_eq!(standing(&engine, "favour", Some("fen")), -100);
    assert_eq!(standing(&engine, "favour", Some("keep")), 20);
}

#[test]
fn a_refused_effect_list_changes_no_standing_or_war() {
    let world = trusted(vec![
        change("renown", None, 10),
        change("favour", Some("guild"), 10),
        Effect::DeclareWar {
            factions: pair("keep", "guild"),
        },
        Effect::PayCurrency { amount: 1_000_000 },
    ]);
    let mut engine = at_the_keep(&world);
    let before = engine.state().clone();
    assert!(choose(&mut engine, TEST).is_err());
    assert_eq!(engine.state(), &before);
}

#[test]
fn saves_keep_standing_and_wars_and_replay_matches() {
    let world = trusted(vec![]);
    let play = |engine: &mut Engine<'_>| {
        let mut events = choose(engine, TERMS).unwrap();
        events.extend(engine.execute(Wait(1_800)).unwrap());
        events
    };
    let mut engine = at_the_keep(&world);
    let events = play(&mut engine);
    // The thaw sets the fen against the guild a day later.
    assert!(events.contains(&Event::WarDeclared {
        factions: pair("fen", "guild")
    }));
    let json = engine.snapshot().to_json();
    let text = String::from_utf8(json.clone()).unwrap();
    assert!(text.contains("\"at_war\"") && text.contains("\"factions\""));
    let restored = Engine::restore(&world, SaveSnapshot::from_json(&json).unwrap()).unwrap();
    assert_eq!(restored.state(), engine.state());
    // The same seed and commands give the same events and state.
    let mut again = at_the_keep(&world);
    assert_eq!(play(&mut again), events);
    assert_eq!(again.state(), engine.state());
}

#[test]
fn saves_with_impossible_standing_or_wars_are_refused() {
    let world = marches();
    let engine = Engine::new(&world).unwrap();
    type Tamper = fn(&mut GameState);
    let cases: Vec<Tamper> = vec![
        // Out of bounds.
        |s| {
            s.standing.insert("renown".into(), Standing::Global(1_001));
        },
        // The wrong scope, or a faction missing.
        |s| {
            s.standing.insert("favour".into(), Standing::Global(0));
        },
        |s| {
            let Standing::Factions(values) = s.standing.get_mut("favour").unwrap() else {
                unreachable!()
            };
            values.remove("guild");
        },
        // An unknown track, and a track missing.
        |s| {
            s.standing.insert("honour".into(), Standing::Global(0));
        },
        |s| {
            s.standing.remove("renown");
        },
        // No effect ever changes the keep and the guild's diplomacy.
        |s| {
            s.at_war.insert(pair("guild", "keep"));
        },
        // Pairs are kept in canonical order, between different factions.
        |s| {
            s.at_war.clear();
            s.at_war.insert(pair("keep", "fen"));
        },
        |s| {
            s.at_war.insert(pair("fen", "fen"));
        },
    ];
    for (index, tamper) in cases.into_iter().enumerate() {
        let mut snapshot = engine.snapshot();
        tamper(&mut snapshot.state);
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "case {index}"
        );
    }
    // Before the thaw, the fen and the guild cannot be at war yet.
    let mut snapshot = engine.snapshot();
    snapshot.state.at_war.insert(pair("fen", "guild"));
    assert!(Engine::restore(&world, snapshot.clone()).is_err());
    snapshot.state.time = Some(2_280);
    snapshot.state.flags.insert("thaw".into());
    assert!(Engine::restore(&world, snapshot).is_ok());
}

#[test]
fn worlds_without_them_keep_no_faction_state() {
    // Factions alone add no standing or diplomacy.
    let mut world = demo();
    world.world.factions = vec![Faction {
        id: "watch".into(),
        name: "The Watch".into(),
    }];
    for world in [demo(), world] {
        let engine = Engine::new(&world).unwrap();
        assert!(engine.state().standing.is_empty() && engine.state().at_war.is_empty());
        let text = String::from_utf8(engine.snapshot().to_json()).unwrap();
        assert!(!text.contains("standing") && !text.contains("at_war"));
        let mut snapshot = engine.snapshot();
        snapshot
            .state
            .standing
            .insert("renown".into(), Standing::Global(0));
        assert!(Engine::restore(&world, snapshot).is_err());
        let mut snapshot = engine.snapshot();
        snapshot.state.at_war.insert(pair("a", "b"));
        assert!(Engine::restore(&world, snapshot).is_err());
    }
}

#[test]
fn a_save_without_a_track_is_refused_before_conditions_read_it() {
    // A breakthrough gated on renown, with the XP waiting at it: checking
    // the technique reads the gate.
    let mut world = sect();
    world.world.standing = vec![StandingTrack {
        id: "renown".into(),
        name: "Renown".into(),
        scope: StandingScope::Global,
        min: 0,
        max: 100,
        start: 0,
        starts: Default::default(),
        thresholds: vec![],
    }];
    let combat = world.world.combat.as_mut().unwrap();
    combat.player_techniques[0].xp = 30;
    combat.techniques[0].ranks[2].requires = Some(Condition::Standing {
        track: "renown".into(),
        faction: None,
        at_least: 50,
    });
    assert_eq!(world.diagnostics(), []);
    let mut snapshot = Engine::new(&world).unwrap().snapshot();
    snapshot.state.standing.clear();
    assert!(matches!(
        Engine::restore(&world, snapshot),
        Err(EngineError::InvalidSave(_))
    ));
}
