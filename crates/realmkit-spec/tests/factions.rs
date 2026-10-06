//! Validation of factions, diplomacy and standing tracks.

mod common;

use common::*;
use realmkit_spec::*;

fn track<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut StandingTrack {
    w.world.standing.iter_mut().find(|t| t.id == id).unwrap()
}

/// A choice for the steward that requires `requires` and applies `effects`.
fn choice(w: &mut WorldSpec, requires: Option<Condition>, effects: Vec<Effect>) {
    let steward = w.dialogues.iter_mut().find(|d| d.id == "steward").unwrap();
    steward.nodes[0].choices.push(DialogueChoice {
        text: "Well?".into(),
        requires,
        effects,
        ..Default::default()
    });
}

fn standing(track: &str, faction: Option<&str>, at_least: i32) -> Option<Condition> {
    Some(Condition::Standing {
        track: track.into(),
        faction: faction.map(Into::into),
        at_least,
    })
}

fn change(track: &str, faction: Option<&str>, by: i32) -> Vec<Effect> {
    vec![Effect::ChangeStanding {
        track: track.into(),
        faction: faction.map(Into::into),
        by,
    }]
}

fn pair(a: &str, b: &str) -> [Id; 2] {
    [a.into(), b.into()]
}

#[test]
fn the_marches_factions_validate_and_roundtrip() {
    let world = marches();
    assert_eq!(world.world.factions.len(), 3);
    assert_eq!(
        world.character("steward").unwrap().faction.as_deref(),
        Some("keep")
    );
    let json = serde_json::to_string(&world.world).unwrap();
    assert_eq!(serde_json::from_str::<World>(&json).unwrap(), world.world);
    // A pair names one relation in either order.
    assert_eq!(faction_pair(&pair("keep", "fen")), pair("fen", "keep"));
    assert_eq!(faction_pair(&pair("fen", "keep")), pair("fen", "keep"));
    // Faction tracks start at `start` unless `starts` names the faction.
    let favour = world.standing_track("favour").unwrap();
    assert_eq!(favour.scope, StandingScope::Faction);
    assert_eq!(
        (favour.start(Some("keep")), favour.start(Some("fen"))),
        (0, -40)
    );
    // A label is the highest threshold reached; below the first, none.
    assert_eq!(favour.label(-100), Some("Hated"));
    assert_eq!(favour.label(19), Some("Neutral"));
    assert_eq!(favour.label(20), Some("Trusted"));
    let renown = world.standing_track("renown").unwrap();
    assert_eq!(renown.scope, StandingScope::Global);
    assert_eq!(
        (renown.label(9), renown.label(10)),
        (None, Some("Known on the fen roads"))
    );
}

#[test]
fn factions_alone_need_neither_diplomacy_nor_standing() {
    let mut world = demo();
    world.world.factions = vec![Faction {
        id: "watch".into(),
        name: "The Watch".into(),
    }];
    world.characters[1].faction = Some("watch".into());
    assert_eq!(codes(&world), Vec::<String>::new());
    // Their names are prose, so an overlay can translate them.
    assert_eq!(world.texts()["world.factions.watch.name"], "The Watch");
}

#[test]
fn faction_content_is_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| {
                let copy = w.world.factions[0].clone();
                w.world.factions.push(copy)
            },
            "duplicate_id",
        ),
        (|w| w.world.factions[0].name = " ".into(), "empty_name"),
        (
            |w| w.characters[2].faction = Some("crown".into()),
            "missing_reference",
        ),
        // Diplomacy pairs.
        (
            |w| w.world.diplomacy.as_mut().unwrap().at_war[0] = pair("keep", "keep"),
            "self_relation",
        ),
        (
            |w| w.world.diplomacy.as_mut().unwrap().at_war[0] = pair("keep", "crown"),
            "missing_reference",
        ),
        (
            |w| {
                let at_war = &mut w.world.diplomacy.as_mut().unwrap().at_war;
                at_war.push(pair("fen", "keep"))
            },
            "duplicate_pair",
        ),
        // Track bounds, starts and thresholds.
        (
            |w| track(w, "renown").max = STANDING_BOUND + 1,
            "invalid_bounds",
        ),
        (|w| track(w, "renown").start = -1, "invalid_start"),
        (
            |w| {
                track(w, "favour").starts.insert("keep".into(), 101);
            },
            "invalid_start",
        ),
        (
            |w| {
                track(w, "renown").starts.insert("keep".into(), 5);
            },
            "standing_scope",
        ),
        (
            |w| {
                track(w, "favour").starts.insert("crown".into(), 5);
            },
            "missing_reference",
        ),
        (
            |w| track(w, "favour").thresholds[1].at = -100,
            "invalid_threshold",
        ),
        (
            |w| track(w, "renown").thresholds[1].at = 1_001,
            "invalid_threshold",
        ),
        (
            |w| track(w, "renown").thresholds[0].name = "".into(),
            "empty_name",
        ),
        (
            |w| {
                let copy = w.world.standing[0].clone();
                w.world.standing.push(copy)
            },
            "duplicate_id",
        ),
        // Standing in conditions and effects.
        (
            |w| choice(w, standing("honour", None, 5), vec![]),
            "missing_reference",
        ),
        (
            |w| choice(w, standing("renown", Some("keep"), 5), vec![]),
            "standing_scope",
        ),
        (
            |w| choice(w, standing("favour", None, 5), vec![]),
            "standing_scope",
        ),
        (
            |w| choice(w, standing("favour", Some("crown"), 5), vec![]),
            "missing_reference",
        ),
        // At least the minimum always holds; past the maximum, never.
        (
            |w| choice(w, standing("favour", Some("keep"), -100), vec![]),
            "invalid_standing",
        ),
        (
            |w| choice(w, standing("renown", None, 1_001), vec![]),
            "invalid_standing",
        ),
        (
            |w| choice(w, None, change("renown", None, 0)),
            "invalid_change",
        ),
        (
            |w| choice(w, None, change("favour", Some("fen"), 201)),
            "invalid_change",
        ),
        (
            |w| choice(w, None, change("favour", None, 5)),
            "standing_scope",
        ),
        // War and peace in conditions and effects.
        (
            |w| {
                let factions = pair("fen", "fen");
                choice(w, None, vec![Effect::DeclareWar { factions }])
            },
            "self_relation",
        ),
        (
            |w| {
                let factions = pair("fen", "crown");
                choice(w, Some(Condition::AtWar { factions }), vec![])
            },
            "missing_reference",
        ),
        // Start answers shape the player, not the world's wars.
        (
            |w| {
                w.world.start_questions.push(StartQuestion {
                    id: "origin".into(),
                    name: "Origin".into(),
                    text: "Where from?".into(),
                    options: vec![StartOption {
                        id: "fen".into(),
                        text: "The fen.".into(),
                        effects: vec![Effect::MakePeace {
                            factions: pair("keep", "fen"),
                        }],
                    }],
                })
            },
            "invalid_effect",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = marches();
        change(&mut world);
        let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
        let expected: std::collections::BTreeSet<_> = [code.to_string()].into();
        assert_eq!(found, expected, "case {index}");
    }
}

#[test]
fn capabilities_stay_separate() {
    // War and peace need a diplomacy block, wherever they are named.
    let mut world = marches();
    world.world.diplomacy = None;
    let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
    assert_eq!(found, ["diplomacy_disabled".to_string()].into());
    // A faction track needs factions to be held with.
    let mut world = demo();
    world.world.standing = vec![StandingTrack {
        id: "favour".into(),
        name: "Favour".into(),
        scope: StandingScope::Faction,
        min: -10,
        max: 10,
        start: 0,
        starts: Default::default(),
        thresholds: vec![],
    }];
    assert_eq!(codes(&world), ["standing_scope"]);
    // A track runs from a minimum up to a higher maximum.
    world.world.standing[0].max = -10;
    assert_eq!(
        codes(&world),
        ["invalid_bounds", "invalid_start", "standing_scope"]
    );
    world.world.standing[0].max = 10;
    // A global track needs nothing else.
    world.world.standing[0].scope = StandingScope::Global;
    assert_eq!(codes(&world), Vec::<String>::new());
    // A start answer may change standing, like an event.
    let mut world = marches();
    world.world.start_questions.push(StartQuestion {
        id: "origin".into(),
        name: "Origin".into(),
        text: "Where from?".into(),
        options: vec![StartOption {
            id: "fen".into(),
            text: "The fen.".into(),
            effects: change("favour", Some("fen"), 30),
        }],
    });
    assert_eq!(codes(&world), Vec::<String>::new());
}

#[test]
fn an_outcome_may_wait_on_standing_or_war_no_start_gives() {
    let outcome = |when: Condition| RouteOutcome {
        id: "ending".into(),
        name: "An ending".into(),
        text: "It ends.".into(),
        when,
    };
    let favour = |at_least| Condition::Standing {
        track: "favour".into(),
        faction: Some("keep".into()),
        at_least,
    };
    let at_war = |a: &str, b: &str| Condition::AtWar {
        factions: pair(a, b),
    };
    let codes_with = |w: &mut WorldSpec, when| {
        w.world.outcomes = vec![outcome(when)];
        codes(w)
    };
    let mut world = marches();
    // Favour with the keep starts at 0, and the fen and the guild at peace.
    assert_eq!(codes_with(&mut world, favour(1)), Vec::<String>::new());
    assert_eq!(
        codes_with(&mut world, at_war("guild", "fen")),
        Vec::<String>::new()
    );
    // The keep and the fen start at war; favour 0 holds from the start.
    assert_eq!(
        codes_with(&mut world, at_war("fen", "keep")),
        ["outcome_at_start"]
    );
    assert_eq!(codes_with(&mut world, favour(0)), ["outcome_at_start"]);
    // A start answer worth 30 favour could give 30 at once, not 31.
    world.world.start_questions.push(StartQuestion {
        id: "origin".into(),
        name: "Origin".into(),
        text: "Where from?".into(),
        options: vec![
            StartOption {
                id: "keep".into(),
                text: "The keep.".into(),
                effects: change("favour", Some("keep"), 30),
            },
            StartOption {
                id: "road".into(),
                text: "The road.".into(),
                effects: vec![],
            },
        ],
    });
    assert_eq!(codes_with(&mut world, favour(30)), ["outcome_at_start"]);
    assert_eq!(codes_with(&mut world, favour(31)), Vec::<String>::new());
}

#[test]
fn an_outcome_may_wait_on_a_peace_or_a_fall_no_start_has() {
    let ending = |id: &str, when: Condition| RouteOutcome {
        id: id.into(),
        name: "An ending".into(),
        text: "It ends.".into(),
        when,
    };
    let not = |c: Condition| Condition::Not {
        condition: Box::new(c),
    };
    let at_war = |a: &str, b: &str| Condition::AtWar {
        factions: pair(a, b),
    };
    let fen_favour = |at_least| Condition::Standing {
        track: "favour".into(),
        faction: Some("fen".into()),
        at_least,
    };
    let codes_with = |outcomes: Vec<RouteOutcome>| {
        let mut world = marches();
        world.world.outcomes = outcomes;
        codes(&world)
    };
    let none = Vec::<String>::new();
    // The keep and the fen start at war, so peace between them is an ending;
    // the guild and the fen start at peace, so it is not.
    assert_eq!(
        codes_with(vec![ending("peace", not(at_war("fen", "keep")))]),
        none
    );
    assert_eq!(
        codes_with(vec![ending("peace", not(at_war("guild", "fen")))]),
        ["outcome_at_start"]
    );
    // The fen's favour starts at -40: falling below -40 is an ending, below -39 is not.
    assert_eq!(codes_with(vec![ending("fall", not(fen_favour(-40)))]), none);
    assert_eq!(
        codes_with(vec![ending("fall", not(fen_favour(-39)))]),
        ["outcome_at_start"]
    );
    // A war and its peace exclude each other, whichever order names the pair.
    let favour = Condition::Standing {
        track: "favour".into(),
        faction: Some("keep".into()),
        at_least: 1,
    };
    assert_eq!(
        codes_with(vec![
            ending("war", at_war("fen", "guild")),
            ending(
                "peace",
                Condition::All {
                    of: vec![not(at_war("guild", "fen")), favour],
                }
            ),
        ]),
        none
    );
}

#[test]
fn higher_standing_excludes_falling_below_a_lower_one() {
    let favour = |at_least| Condition::Standing {
        track: "favour".into(),
        faction: Some("keep".into()),
        at_least,
    };
    let ending = |id: &str, when| RouteOutcome {
        id: id.into(),
        name: "An ending".into(),
        text: "It ends.".into(),
        when,
    };
    let mut world = marches();
    world.world.outcomes = vec![
        ending("honoured", favour(10)),
        ending(
            "disgraced",
            Condition::Not {
                condition: Box::new(favour(-5)),
            },
        ),
    ];
    assert_eq!(codes(&world), Vec::<String>::new());
    // Below 10 and at least -5 can hold together.
    world.world.outcomes[0].when = Condition::Not {
        condition: Box::new(favour(10)),
    };
    world.world.outcomes[1].when = favour(-5);
    assert!(codes(&world).contains(&"ambiguous_outcomes".to_string()));
}

#[test]
fn inverted_bounds_are_reported_wherever_the_track_is_read() {
    let mut world = marches();
    let favour = world.world.standing.iter_mut().find(|t| t.id == "favour");
    let favour = favour.unwrap();
    (favour.min, favour.max) = (100, -100);
    world.world.outcomes = vec![RouteOutcome {
        id: "honoured".into(),
        name: "Honoured".into(),
        text: "It ends.".into(),
        when: Condition::Standing {
            track: "favour".into(),
            faction: Some("keep".into()),
            at_least: 50,
        },
    }];
    assert!(codes(&world).contains(&"invalid_bounds".to_string()));
}
