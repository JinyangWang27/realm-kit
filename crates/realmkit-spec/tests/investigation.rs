//! Evidence definitions and the conditions and effects that use them.

mod common;

use common::*;
use realmkit_spec::*;

fn discover(evidence: &str) -> Effect {
    Effect::DiscoverEvidence {
        evidence: evidence.into(),
    }
}

/// The copyist's first choice, which discovers the stitched map.
fn confession(w: &mut WorldSpec) -> &mut DialogueChoice {
    let copyist = w.dialogues.iter_mut().find(|d| d.id == "copyist").unwrap();
    &mut copyist.nodes[0].choices[0]
}

#[test]
fn the_archive_authors_evidence_and_roundtrips() {
    let world = archive();
    assert!(world.diagnostics().is_empty());
    let map = world.evidence("stitched_map").unwrap();
    assert_eq!(map.item, None);
    assert_eq!(map.facts.len(), 2);
    assert_eq!(map.interpretations[0].when, None);
    let json = serde_json::to_string(&world.world).unwrap();
    assert_eq!(serde_json::from_str::<World>(&json).unwrap(), world.world);
}

#[test]
fn evidence_references_are_checked() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| confession(w).effects.push(discover("missing")),
            "missing_reference",
        ),
        (
            |w| {
                confession(w).requires = Some(Condition::Evidence {
                    evidence: "missing".into(),
                })
            },
            "missing_reference",
        ),
        (
            |w| w.world.evidence[0].item = Some("missing".into()),
            "missing_reference",
        ),
        (
            |w| {
                let copy = w.world.evidence[0].clone();
                w.world.evidence.push(copy)
            },
            "duplicate_id",
        ),
        // Nothing discovers the map any more, so Pell's thanks never shows.
        (
            |w| {
                confession(w).effects.pop();
            },
            "undiscoverable_evidence",
        ),
        // A start answer or an event only shapes the world, not what the
        // player has found out.
        (
            |w| {
                w.world.start_questions[0].options[0]
                    .effects
                    .push(discover("stitched_map"))
            },
            "invalid_effect",
        ),
        // A reading waits on something that exists.
        (
            |w| {
                w.world.evidence[0].interpretations[1].when = Some(Condition::Flag {
                    flag: "missing".into(),
                })
            },
            "missing_reference",
        ),
        // Readings move only forward: a pen returned, or a `not`, could take
        // an understanding away again.
        (
            |w| {
                w.world.evidence[0].interpretations[1].when = Some(Condition::Item {
                    item: "pen".into(),
                    quantity: 1,
                })
            },
            "fleeting_interpretation",
        ),
        (
            |w| {
                w.world.evidence[0].interpretations[1].when = Some(Condition::Not {
                    condition: Box::new(Condition::Flag {
                        flag: "pen_borrowed".into(),
                    }),
                })
            },
            "fleeting_interpretation",
        ),
        // Facts, once known, stay known: the same rule as readings.
        (
            |w| {
                w.world.evidence[0].facts.push(Fact::Gated {
                    text: "Later.".into(),
                    when: Condition::Not {
                        condition: Box::new(Condition::Flag {
                            flag: "vault_open".into(),
                        }),
                    },
                })
            },
            "fleeting_fact",
        ),
        (
            |w| {
                w.world.evidence[0].facts.push(Fact::Gated {
                    text: "Later.".into(),
                    when: Condition::Quest {
                        quest: "lost_map".into(),
                        status: QuestStatus::Active,
                    },
                })
            },
            "fleeting_fact",
        ),
        (
            |w| {
                w.world.evidence[0].facts.push(Fact::Gated {
                    text: "Later.".into(),
                    when: Condition::Flag {
                        flag: "missing".into(),
                    },
                })
            },
            "missing_reference",
        ),
        // `at_least` lasts only when everything under it does.
        (
            |w| {
                w.world.evidence[0].interpretations[1].when = Some(Condition::AtLeast {
                    count: 1,
                    of: vec![
                        Condition::Flag {
                            flag: "vault_open".into(),
                        },
                        Condition::Item {
                            item: "pen".into(),
                            quantity: 1,
                        },
                    ],
                })
            },
            "fleeting_interpretation",
        ),
    ];
    for (change, code) in cases {
        let mut world = archive();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            codes(&world)
        );
    }
    // Over lasting leaves, `at_least` reads like `all` and `any`.
    let mut world = archive();
    world.world.evidence[0].interpretations[1].when = Some(Condition::AtLeast {
        count: 2,
        of: ["vault_open", "map_found", "pen_borrowed"]
            .map(|flag| Condition::Flag { flag: flag.into() })
            .into(),
    });
    assert_eq!(codes(&world), Vec::<String>::new());
    // A linked item that exists is fine: possession and evidence stay apart.
    let mut world = archive();
    world.world.evidence[0].item = Some("pen".into());
    assert!(world.diagnostics().is_empty());
}

#[test]
fn an_event_cannot_discover_evidence() {
    let mut world = marches();
    world.world.evidence.push(EvidenceDefinition {
        id: "tracks".into(),
        name: "Tracks".into(),
        description: "Hoofprints.".into(),
        source: String::new(),
        facts: Vec::new(),
        interpretations: Vec::new(),
        item: None,
    });
    world.world.events[0].effects.push(discover("tracks"));
    assert!(codes(&world).contains(&"invalid_effect".to_string()));
}

#[test]
fn a_fact_is_a_bare_string_or_text_with_a_condition() {
    let facts: Vec<Fact> = serde_json::from_str(
        r#"["Seen.", { "text": "Later.", "when": { "kind": "flag", "flag": "a" } }]"#,
    )
    .unwrap();
    assert_eq!(facts[0], Fact::Known("Seen.".into()));
    assert_eq!(facts[1].text(), "Later.");
    assert!(facts[1].when().is_some());
    // Bare strings stay bare when written back.
    assert_eq!(
        serde_json::to_string(&facts[0]).unwrap(),
        r#""Seen.""#.to_string()
    );
    let extra = r#"{ "text": "Later.", "when": { "kind": "flag", "flag": "a" }, "if": 1 }"#;
    assert!(serde_json::from_str::<Fact>(extra).is_err());
}
