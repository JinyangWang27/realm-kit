use realmkit_spec::*;

fn demo() -> WorldSpec {
    // Deserialize directly so validation tests isolate validation from package loading.
    WorldSpec {
        world: serde_json::from_str(include_str!("../../../examples/demo-world/world.json"))
            .unwrap(),
        locations: serde_json::from_str(include_str!(
            "../../../examples/demo-world/locations.json"
        ))
        .unwrap(),
        npcs: serde_json::from_str(include_str!("../../../examples/demo-world/npcs.json")).unwrap(),
        monsters: serde_json::from_str(include_str!("../../../examples/demo-world/monsters.json"))
            .unwrap(),
        items: serde_json::from_str(include_str!("../../../examples/demo-world/items.json"))
            .unwrap(),
        quests: serde_json::from_str(include_str!("../../../examples/demo-world/quests.json"))
            .unwrap(),
        dialogues: serde_json::from_str(include_str!(
            "../../../examples/demo-world/dialogues.json"
        ))
        .unwrap(),
        narrative: serde_json::from_str(include_str!(
            "../../../examples/demo-world/narrative.json"
        ))
        .unwrap(),
    }
}

#[test]
fn loads_and_roundtrips_the_authored_package() {
    let world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/demo-world"
    ))
    .unwrap();
    assert_eq!(world, demo());
    assert_eq!(
        world.location("village").unwrap().exits[&Direction::North].destination,
        "forest"
    );
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
}

#[test]
fn refuses_unsupported_versions_duplicate_ids_and_dangling_references() {
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.world.format_version = 999,
        |w| w.locations.push(w.locations[0].clone()),
        |w| w.world.start = "missing".into(),
        |w| {
            w.locations[0]
                .exits
                .get_mut(&Direction::North)
                .unwrap()
                .destination = "missing".into()
        },
        |w| w.locations[0].npcs.push("missing".into()),
        |w| w.locations[1].monsters.push("missing".into()),
        |w| w.npcs[0].dialogue = "missing".into(),
        |w| w.dialogues[0].nodes[0].choices[0].next = Some("missing".into()),
        |w| {
            w.dialogues[0].nodes[0].choices[0].effect = Some(DialogueEffect::AcceptQuest {
                quest: "missing".into(),
            })
        },
        |w| w.monsters[0].loot[0].item = "missing".into(),
        |w| {
            w.quests[0].objective = QuestObjective::Defeat {
                monster: "missing".into(),
            }
        },
        |w| w.quests[0].completion_flags.push("missing".into()),
        |w| {
            w.locations[0]
                .exits
                .get_mut(&Direction::East)
                .unwrap()
                .requires = vec![Condition::Quest {
                quest: "missing".into(),
                status: QuestStatus::Completed,
            }]
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = demo();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid reference case {index}");
    }
}

#[test]
fn refuses_unusable_rules_and_malformed_templates() {
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.world.levels.clear(),
        |w| w.world.levels[0].xp = 1,
        |w| w.world.levels[1].xp = 0,
        |w| w.world.levels[0].hp = 0,
        |w| w.world.levels[0].attack = 0,
        |w| w.monsters[0].hp = 0,
        |w| w.monsters[0].loot[0].quantity = 0,
        |w| w.locations[0].monsters.push("wolf".into()),
        |w| w.narrative.attack.clear(),
        |w| w.narrative.attack[0].0 = "{unknown}".into(),
        |w| w.narrative.attack[0].0 = "{damage".into(),
        |w| w.narrative.attack[0].0 = "damage}".into(),
        |w| w.items[0].id = "has spaces".into(),
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = demo();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid rule case {index}");
    }
}

#[test]
fn diagnostics_identify_entities_and_stable_codes_for_repair() {
    let mut world = demo();
    world.world.start = "missing".into();
    world.monsters[0].hp = 0;
    let diagnostics = world.diagnostics();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics
        .iter()
        .any(|d| d.entity_id.as_deref() == Some("wolf")
            && d.code == "invalid_stats"
            && d.severity == Severity::Error));
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "missing_reference" && d.message.contains("missing")));
    let json = serde_json::to_value(&diagnostics).unwrap();
    assert_eq!(json[0]["severity"], "error");
}

#[test]
fn revision_is_stable_for_equal_content_and_changes_with_any_edit() {
    let world = demo();
    assert_eq!(world.revision(), demo().revision());
    let mut edited = demo();
    edited.items[0].description.push('.');
    assert_ne!(world.revision(), edited.revision());
}
