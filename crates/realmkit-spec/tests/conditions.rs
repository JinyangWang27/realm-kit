//! Validation of condition trees and effect lists.

mod common;

use common::*;
use realmkit_spec::*;

fn flag(flag: &str) -> Condition {
    Condition::Flag { flag: flag.into() }
}

fn stack(item: &str, quantity: u64) -> ItemStack {
    ItemStack {
        item: item.into(),
        quantity,
    }
}

/// The copyist's first choice, to hang conditions and effects on.
fn choice(w: &mut WorldSpec) -> &mut DialogueChoice {
    let copyist = w.dialogues.iter_mut().find(|d| d.id == "copyist").unwrap();
    &mut copyist.nodes[0].choices[0]
}

#[test]
fn the_archive_uses_trees_and_effect_lists_and_roundtrips() {
    let world = archive();
    assert!(world.diagnostics().is_empty());
    let json = serde_json::to_string(&world.dialogues).unwrap();
    let back: Vec<Dialogue> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, world.dialogues);
}

#[test]
fn every_leaf_of_a_tree_is_checked_and_compositions_are_not_empty() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| choice(w).requires = Some(Condition::All { of: vec![] }),
            "invalid_condition",
        ),
        (
            |w| choice(w).requires = Some(Condition::Any { of: vec![] }),
            "invalid_condition",
        ),
        // A dangling flag deep inside `any` and `not` is still found.
        (
            |w| {
                choice(w).requires = Some(Condition::Any {
                    of: vec![
                        flag("map_found"),
                        Condition::Not {
                            condition: Box::new(flag("missing")),
                        },
                    ],
                })
            },
            "missing_reference",
        ),
        (
            |w| {
                choice(w).requires = Some(Condition::Item {
                    item: "missing".into(),
                    quantity: 1,
                })
            },
            "missing_reference",
        ),
        (
            |w| {
                choice(w).requires = Some(Condition::Item {
                    item: "pen".into(),
                    quantity: 0,
                })
            },
            "invalid_quantity",
        ),
        (
            |w| {
                choice(w).effects.push(Effect::TakeItems {
                    items: vec![stack("pen", 0)],
                })
            },
            "invalid_quantity",
        ),
        (
            |w| {
                choice(w).effects.push(Effect::GrantItems {
                    items: vec![stack("missing", 1)],
                })
            },
            "missing_reference",
        ),
        (
            |w| {
                choice(w).effects.push(Effect::SetFlag {
                    flag: "missing".into(),
                })
            },
            "missing_reference",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = archive();
        change(&mut world);
        assert_eq!(codes(&world), [code], "case {index}");
    }
}

fn smithy() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/smithy"
    ))
    .unwrap()
}

#[test]
fn equipment_cannot_be_counted_or_taken() {
    let mut world = smithy();
    let master = world.dialogues[0].nodes[0].choices.first_mut().unwrap();
    master.requires = Some(Condition::Item {
        item: "iron_sword".into(),
        quantity: 1,
    });
    assert_eq!(codes(&world), ["invalid_item"]);
    let mut world = smithy();
    let master = world.dialogues[0].nodes[0].choices.first_mut().unwrap();
    master.effects = vec![Effect::TakeItems {
        items: vec![stack("iron_sword", 1)],
    }];
    assert_eq!(codes(&world), ["invalid_item"]);
    // Granting a piece is fine: it arrives as an individual piece.
    let mut world = smithy();
    let master = world.dialogues[0].nodes[0].choices.first_mut().unwrap();
    master.effects = vec![Effect::GrantItems {
        items: vec![stack("iron_sword", 1)],
    }];
    assert!(world.diagnostics().is_empty());
}

#[test]
fn a_list_where_one_condition_is_expected_is_refused() {
    let json =
        r#"{ "text": "Hello", "next": null, "requires": [{ "kind": "flag", "flag": "a" }] }"#;
    assert!(serde_json::from_str::<DialogueChoice>(json).is_err());
    let json =
        r#"{ "text": "Hello", "next": null, "effect": { "kind": "set_flag", "flag": "a" } }"#;
    assert!(serde_json::from_str::<DialogueChoice>(json).is_err());
}

#[test]
fn start_questions_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| w.world.start_questions[0].options.clear(),
            "empty_start_question",
        ),
        (
            |w| {
                let copy = w.world.start_questions[0].options[0].clone();
                w.world.start_questions[0].options.push(copy);
            },
            "duplicate_id",
        ),
        (
            |w| {
                let copy = w.world.start_questions[0].clone();
                w.world.start_questions.push(copy);
            },
            "duplicate_id",
        ),
        (
            |w| {
                w.world.start_questions[0].options[0].effects = vec![Effect::SetFlag {
                    flag: "unknown".into(),
                }]
            },
            "missing_reference",
        ),
        (
            |w| {
                w.world.start_questions[0].options[0].effects = vec![Effect::TakeItems {
                    items: vec![ItemStack {
                        item: "pen".into(),
                        quantity: 1,
                    }],
                }]
            },
            "invalid_effect",
        ),
        (
            |w| {
                w.world.start_questions[0].options[0].effects = vec![Effect::AcceptQuest {
                    quest: "lost_map".into(),
                }]
            },
            "invalid_effect",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = archive();
        change(&mut world);
        assert_eq!(codes(&world), [code], "case {index}");
    }
}

#[test]
fn the_richest_start_answers_must_fit_the_currency_bound() {
    let grant = |amount| StartOption {
        id: format!("grant_{amount}"),
        text: "Coin.".into(),
        effects: vec![Effect::GrantCurrency { amount }],
    };
    let question = |id: &str, options| StartQuestion {
        id: id.into(),
        name: "Purse".into(),
        text: "How much do you carry?".into(),
        options,
    };
    let mut world = marches();
    let start = world.economy().unwrap().currency.start;
    // Alternatives in one question do not add up.
    let most = CURRENCY_BOUND - start;
    world.world.start_questions = vec![question("purse", vec![grant(most), grant(most - 1)])];
    assert!(codes(&world).is_empty(), "{:?}", codes(&world));
    // Answers to separate questions do.
    world
        .world
        .start_questions
        .push(question("legacy", vec![grant(1)]));
    assert_eq!(codes(&world), ["start_overflow"]);
}
