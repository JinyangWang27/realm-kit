//! Story phases, main quests and quest prerequisites.

mod common;

use common::*;
use realmkit_spec::*;

fn phase(id: &str) -> Phase {
    Phase {
        id: id.into(),
        name: id.into(),
    }
}

fn done(quest: &str) -> Condition {
    Condition::Quest {
        quest: quest.into(),
        status: QuestStatus::Completed,
    }
}

/// The archive in two phases, the second entered when Pell hears the map
/// is found, and a second quest that follows the first.
fn chained() -> WorldSpec {
    let mut w = archive();
    w.world.phases = vec![phase("visit"), phase("vault")];
    let pell = w.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[1].effects.push(Effect::EnterPhase {
        phase: "vault".into(),
    });
    let mut second = w.quests[0].clone();
    second.id = "catalogue".into();
    second.requires = Some(done("lost_map"));
    w.quests.push(second);
    w
}

#[test]
fn a_chained_world_is_valid_and_roundtrips() {
    let world = chained();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let json = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&json).unwrap(), world);
}

#[test]
fn phases_and_prerequisites_are_checked() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| w.world.phases.push(phase("visit")), "duplicate_id"),
        // Nothing enters it, so the story could never get there.
        (
            |w| w.world.phases.push(phase("epilogue")),
            "unreachable_phase",
        ),
        (
            |w| {
                w.quests[1].requires = Some(Condition::Phase {
                    phase: "missing".into(),
                })
            },
            "missing_reference",
        ),
        (
            |w| {
                w.dialogues[0].nodes[0].choices[0]
                    .effects
                    .push(Effect::EnterPhase {
                        phase: "missing".into(),
                    })
            },
            "missing_reference",
        ),
        (
            |w| {
                w.world.start_questions[0].options[0]
                    .effects
                    .push(Effect::EnterPhase {
                        phase: "vault".into(),
                    })
            },
            "invalid_effect",
        ),
        (
            |w| w.quests[1].requires = Some(done("missing")),
            "missing_reference",
        ),
        // Each waits for the other.
        (
            |w| w.quests[0].requires = Some(done("catalogue")),
            "quest_cycle",
        ),
        (
            |w| w.quests[1].requires = Some(done("catalogue")),
            "quest_cycle",
        ),
        // The main story may not wait on a side story.
        (|w| w.quests[1].main = true, "main_requires_side"),
    ];
    for (change, code) in cases {
        let mut world = chained();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            codes(&world)
        );
    }
    // Both main, or a side alternative beside a main one, is fine.
    let mut world = chained();
    world.quests[0].main = true;
    world.quests[1].main = true;
    assert!(world.diagnostics().is_empty());
    let mut world = chained();
    world.quests[1].main = true;
    world.quests[1].requires = Some(Condition::Any {
        of: vec![
            done("lost_map"),
            Condition::Phase {
                phase: "vault".into(),
            },
        ],
    });
    assert!(world.diagnostics().is_empty());
}

#[test]
fn outcomes_are_checked() {
    let ending = RouteOutcome {
        id: "map_home".into(),
        name: "The map is home".into(),
        text: "The end.".into(),
        when: done("lost_map"),
    };
    let mut world = chained();
    world.world.outcomes = vec![ending.clone()];
    assert!(world.diagnostics().is_empty());
    world.world.outcomes.push(ending.clone());
    assert!(codes(&world).contains(&"duplicate_id".to_string()));
    world.world.outcomes = vec![RouteOutcome {
        when: done("missing"),
        ..ending.clone()
    }];
    assert!(codes(&world).contains(&"missing_reference".to_string()));
    // Each of these could hold before the first turn: a flag a start answer
    // sets, the first phase, an available quest, or nothing in particular.
    let flag = |f: &str| Condition::Flag { flag: f.into() };
    for when in [
        flag("river_scholar"),
        Condition::Phase {
            phase: "visit".into(),
        },
        Condition::Quest {
            quest: "lost_map".into(),
            status: QuestStatus::Available,
        },
        Condition::Not {
            condition: Box::new(flag("map_found")),
        },
        Condition::Any {
            of: vec![done("lost_map"), flag("apprentice")],
        },
    ] {
        let mut world = chained();
        world.world.outcomes = vec![RouteOutcome {
            when: when.clone(),
            ..ending.clone()
        }];
        assert!(
            codes(&world).contains(&"outcome_at_start".to_string()),
            "{when:?}"
        );
    }
    // A flag no answer sets, a later phase or evidence cannot hold yet.
    for when in [
        flag("map_found"),
        Condition::Phase {
            phase: "vault".into(),
        },
        Condition::All {
            of: vec![flag("river_scholar"), done("lost_map")],
        },
    ] {
        let mut world = chained();
        world.world.outcomes = vec![RouteOutcome {
            when,
            ..ending.clone()
        }];
        assert!(world.diagnostics().is_empty(), "{:?}", codes(&world));
    }
}

/// The caravan slice, broken in the ways an author could break it.
#[test]
fn the_caravan_slice_rejects_impossible_progression() {
    assert!(caravan_trail().diagnostics().is_empty());
    fn choices<'w>(w: &'w mut WorldSpec, id: &str) -> impl Iterator<Item = &'w mut DialogueChoice> {
        let dialogue = w.dialogues.iter_mut().find(|d| d.id == id).unwrap();
        dialogue.nodes.iter_mut().flat_map(|n| &mut n.choices)
    }
    fn quest<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Quest {
        w.quests.iter_mut().find(|q| q.id == id).unwrap()
    }
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        // Nothing enters The Buried Road's phase any more.
        (
            |w| {
                for choice in choices(w, "tenko") {
                    choice
                        .effects
                        .retain(|e| !matches!(e, Effect::EnterPhase { .. }));
                }
            },
            "unreachable_phase",
        ),
        // Without the ring, no reading can be asked for.
        (
            |w| {
                for choice in choices(w, "stone_ring") {
                    choice
                        .effects
                        .retain(|e| !matches!(e, Effect::DiscoverEvidence { .. }));
                }
            },
            "undiscoverable_evidence",
        ),
        // The mystery cannot wait on its own sequel.
        (
            |w| quest(w, "lost_wagons").requires = Some(done("buried_road")),
            "quest_cycle",
        ),
        // Nor on the ostler's errand.
        (
            |w| quest(w, "buried_road").requires = Some(done("bales")),
            "main_requires_side",
        ),
        (
            |w| {
                w.locations[2].known_when = Some(Condition::Evidence {
                    evidence: "rumour".into(),
                })
            },
            "missing_reference",
        ),
        (
            |w| w.world.evidence[1].item = Some("crate".into()),
            "missing_reference",
        ),
        (
            |w| {
                w.characters
                    .iter_mut()
                    .find(|c| c.id == "rask")
                    .unwrap()
                    .kind = CharacterKind::Feature
            },
            "invalid_feature",
        ),
    ];
    for (change, code) in cases {
        let mut world = caravan_trail();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            codes(&world)
        );
    }
}

#[test]
fn a_choice_cannot_establish_what_it_needs() {
    // Pell's thanks enter the vault phase, but only once it is reached.
    let mut world = chained();
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[1].requires = Some(Condition::Phase {
        phase: "vault".into(),
    });
    assert!(codes(&world).contains(&"unreachable_phase".to_string()));
    // The copyist shows the map only to someone who has already seen it.
    let mut world = archive();
    let copyist = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "copyist")
        .unwrap();
    copyist.nodes[0].choices[0].requires = Some(Condition::All {
        of: vec![
            Condition::Evidence {
                evidence: "stitched_map".into(),
            },
            Condition::Flag {
                flag: "pen_borrowed".into(),
            },
        ],
    });
    assert!(codes(&world).contains(&"undiscoverable_evidence".to_string()));
    // Needing it on one branch only leaves the other way open.
    let copyist = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "copyist")
        .unwrap();
    copyist.nodes[0].choices[0].requires = Some(Condition::Any {
        of: vec![
            Condition::Evidence {
                evidence: "stitched_map".into(),
            },
            Condition::Flag {
                flag: "pen_borrowed".into(),
            },
        ],
    });
    assert!(world.diagnostics().is_empty(), "{:?}", codes(&world));
}
