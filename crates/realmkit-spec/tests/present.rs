//! Stable diagnostics and deliberate classification of every effect variant.
use realmkit_spec::*;

fn ford() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/turning-ford"
    ))
    .unwrap()
}

#[test]
fn every_effect_is_deliberately_classified_for_scheduled_rules() {
    let samples = [
        r#"{"kind":"relocate","character":"envoy","to":"ford"}"#,
        r#"{"kind":"depart","character":"clerk","reason":"dead"}"#,
        r#"{"kind":"record","entry":"transfer"}"#,
        r#"{"kind":"set_control","location":"ford","faction":"reed"}"#,
        r#"{"kind":"accept_quest","quest":"testimony"}"#,
        r#"{"kind":"complete_quest","quest":"testimony"}"#,
        r#"{"kind":"set_flag","flag":"heard"}"#,
        r#"{"kind":"grant_technique","technique":"unknown"}"#,
        r#"{"kind":"grant_items","items":[]}"#,
        r#"{"kind":"take_items","items":[]}"#,
        r#"{"kind":"grant_currency","amount":1}"#,
        r#"{"kind":"pay_currency","amount":1}"#,
        r#"{"kind":"buy_workshop","workshop":"unknown"}"#,
        r#"{"kind":"sell_workshop","workshop":"unknown"}"#,
        r#"{"kind":"raise_proficiency","proficiency":"trading","ranks":1}"#,
        r#"{"kind":"discover_evidence","evidence":"unknown"}"#,
        r#"{"kind":"enter_phase","phase":"unknown"}"#,
        r#"{"kind":"change_standing","track":"trust","by":1}"#,
        r#"{"kind":"declare_war","factions":["reed","stone"]}"#,
        r#"{"kind":"make_peace","factions":["reed","stone"]}"#,
        r#"{"kind":"join_party","character":"clerk"}"#,
        r#"{"kind":"leave_party","character":"clerk"}"#,
    ];
    for json in samples {
        let effect: Effect = serde_json::from_str(json).unwrap();
        // Intentionally exhaustive: adding a variant must classify it here.
        let allowed = match &effect {
            Effect::Relocate { .. }
            | Effect::Depart { .. }
            | Effect::Record { .. }
            | Effect::SetControl { .. }
            | Effect::SetFlag { .. }
            | Effect::GrantTechnique(_)
            | Effect::GrantItems { .. }
            | Effect::GrantCurrency { .. }
            | Effect::ChangeStanding { .. }
            | Effect::DeclareWar { .. }
            | Effect::MakePeace { .. } => true,
            Effect::AcceptQuest { .. }
            | Effect::CompleteQuest { .. }
            | Effect::TakeItems { .. }
            | Effect::PayCurrency { .. }
            | Effect::BuyWorkshop { .. }
            | Effect::SellWorkshop { .. }
            | Effect::RaiseProficiency { .. }
            | Effect::DiscoverEvidence { .. }
            | Effect::EnterPhase { .. }
            | Effect::JoinParty { .. }
            | Effect::LeaveParty { .. } => false,
        };
        let mut world = ford();
        world.world.events[0].effects = vec![effect];
        assert_eq!(
            world
                .diagnostics()
                .iter()
                .any(|d| d.code == "invalid_effect"),
            !allowed,
            "{json}"
        );
    }
}

#[test]
fn present_content_has_stable_validation_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| w.world.chronicle.push(w.world.chronicle[0].clone()),
            "duplicate_id",
        ),
        (
            |w| w.world.chronicle[0].text.clear(),
            "empty_chronicle_text",
        ),
        (
            |w| {
                w.world.chronicle[0].known_when = Some(Condition::Departed {
                    character: "unknown".into(),
                })
            },
            "missing_reference",
        ),
        (|w| w.world.time = None, "time_disabled"),
        (
            |w| w.locations[0].controller = Some("unknown".into()),
            "missing_reference",
        ),
        (|w| w.quests[0].void_reason = None, "invalid_void_reason"),
        (
            |w| w.quests[0].void_reason = Some(" ".into()),
            "invalid_void_reason",
        ),
        (
            |w| {
                w.world.events[0].effects = vec![Effect::Relocate {
                    character: "you".into(),
                    to: "ford".into(),
                }]
            },
            "invalid_present_target",
        ),
        (
            |w| {
                w.world.events[0].effects = vec![Effect::Depart {
                    character: "you".into(),
                    reason: DepartureReason::Left,
                }]
            },
            "invalid_present_target",
        ),
        (
            |w| {
                w.world.events[0].effects = vec![Effect::Record {
                    entry: "unknown".into(),
                }]
            },
            "missing_reference",
        ),
        (
            |w| {
                w.world.events[0].requires = Some(Condition::Elapsed {
                    entry: "transfer".into(),
                    minutes: WORLD_TIME_BOUND + 1,
                })
            },
            "invalid_elapsed",
        ),
        (
            |w| {
                w.world.events[0].requires = Some(Condition::Controls {
                    location: "unknown".into(),
                    faction: "reed".into(),
                })
            },
            "missing_reference",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = ford();
        change(&mut world);
        assert!(
            world.diagnostics().iter().any(|d| d.code == code),
            "case {index}, expected {code}: {:?}",
            world.diagnostics()
        );
    }
    for target in ["moves", "companion"] {
        let mut value = serde_json::to_value(ford()).unwrap();
        value["characters"][1][target] = if target == "companion" {
            serde_json::json!({})
        } else {
            serde_json::json!({"among":["ford","refuge"],"schedule":{"at":60,"every":60}})
        };
        let world: WorldSpec = serde_json::from_value(value).unwrap();
        assert!(world
            .diagnostics()
            .iter()
            .any(|d| d.code == "invalid_present_target"));
    }
}

#[test]
fn revision_covers_every_new_rule_and_leaves_prose_translatable() {
    let world = ford();
    let revision = world.revision();
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.locations[0].controller = Some("reed".into()),
        |w| w.world.chronicle[0].id = "renamed".into(),
        |w| {
            w.world.chronicle[0].known_when = Some(Condition::Flag {
                flag: "heard".into(),
            })
        },
        |w| {
            w.quests[0].void_when = Some(Condition::Elapsed {
                entry: "transfer".into(),
                minutes: 12,
            })
        },
        |w| {
            w.world.events[0].effects[0] = Effect::Relocate {
                character: "envoy".into(),
                to: "ford".into(),
            }
        },
        |w| {
            w.world.events[0].effects[1] = Effect::Depart {
                character: "clerk".into(),
                reason: DepartureReason::Dead,
            }
        },
        |w| {
            w.world.events[0].effects[3] = Effect::Record {
                entry: "transfer".into(),
            }
        },
        |w| {
            w.dialogues[0].nodes[0].choices[0].effects[0] = Effect::SetControl {
                location: "refuge".into(),
                faction: "reed".into(),
            }
        },
    ];
    for change in changes {
        let mut edited = world.clone();
        change(&mut edited);
        assert_ne!(edited.revision(), revision);
    }
    let mut translated = world.clone();
    translated.world.chronicle[0].text = "Other words".into();
    translated.quests[0].void_reason = Some("Other reason".into());
    assert_eq!(translated.revision(), revision);
    assert!(world.texts().contains_key("world.chronicle.transfer.text"));
    assert!(world.texts().contains_key("quests.testimony.void_reason"));
    assert_eq!(
        serde_json::from_str::<Condition>(r#"{"kind":"elapsed","entry":"transfer"}"#).unwrap(),
        Condition::Elapsed {
            entry: "transfer".into(),
            minutes: 0
        }
    );
}

#[test]
fn a_shared_authored_character_cannot_be_a_present_target() {
    let mut world = ford();
    world.locations[1].characters.push("clerk".into());
    assert!(world
        .diagnostics()
        .iter()
        .any(|d| d.code == "invalid_present_target"));
}

#[test]
fn present_leaves_can_gate_an_outcome_and_monotone_place_knowledge() {
    let mut world = ford();
    world.world.outcomes.push(RouteOutcome {
        id: "exile".into(),
        name: "Exile".into(),
        text: "The clerk left.".into(),
        when: Condition::Departed {
            character: "clerk".into(),
        },
    });
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    world.world.outcomes[0].when = Condition::Elapsed {
        entry: "transfer".into(),
        minutes: 1,
    };
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    world.world.outcomes[0].when = Condition::Controls {
        location: "ford".into(),
        faction: "reed".into(),
    };
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
}

#[test]
fn closed_quest_outcomes_must_exclude_initial_closure() {
    let mut world = ford();
    world.world.outcomes.push(RouteOutcome {
        id: "closure".into(),
        name: "Closure".into(),
        text: "It ended.".into(),
        when: Condition::Quest {
            quest: "testimony".into(),
            status: QuestStatus::Closed,
        },
    });
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    world.quests[0].void_when = Some(Condition::Controls {
        location: "ford".into(),
        faction: "stone".into(),
    });
    assert!(world
        .diagnostics()
        .iter()
        .any(|d| d.code == "outcome_at_start"));
}
