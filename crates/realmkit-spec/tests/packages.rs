//! Loading, formats, IDs, references, revisions and worlds without combat.

mod common;

use common::*;
use realmkit_spec::*;

fn profile() -> CombatProfile {
    CombatProfile {
        stats: wolf(&mut demo()).combat.clone().unwrap().stats,
        xp: 1,
        loot: vec![],
        skills: vec![],
        basic_channel: Channel::Physical,
        basic_crit: None,
        level: 1,
        group: None,
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
        |w| w.locations[0].characters.push("missing".into()),
        |w| w.characters[1].dialogue = Some("missing".into()),
        |w| w.world.player = "missing".into(),
        |w| w.dialogues[0].nodes[0].choices[0].next = Some("missing".into()),
        |w| {
            w.dialogues[0].nodes[0].choices[0].effects = vec![Effect::AcceptQuest {
                quest: "missing".into(),
            }]
        },
        |w| wolf(w).combat.as_mut().unwrap().loot[0].item = "missing".into(),
        |w| {
            w.quests[0].objective = QuestObjective::Defeat {
                character: "missing".into(),
            }
        },
        |w| {
            w.quests[0].objective = QuestObjective::Flag {
                flag: "missing".into(),
            }
        },
        |w| w.quests[0].completion_flags.push("missing".into()),
        |w| {
            w.locations[0]
                .exits
                .get_mut(&Direction::East)
                .unwrap()
                .requires = Some(Condition::Quest {
                quest: "missing".into(),
                status: QuestStatus::Completed,
            })
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
        |w| w.world.combat.as_mut().unwrap().levels.clear(),
        |w| w.world.combat.as_mut().unwrap().levels[0].xp = 1,
        |w| w.world.combat.as_mut().unwrap().levels[1].xp = 0,
        |w| w.world.combat.as_mut().unwrap().levels[0].stats.hp = 0,
        |w| w.world.combat.as_mut().unwrap().levels[0].stats.speed = 0,
        |w| w.world.combat.as_mut().unwrap().levels[0].stats.mp = STAT_BOUND + 1,
        |w| w.world.combat.as_mut().unwrap().levels[1].stats.sdef = 0,
        |w| {
            let stats = &mut w.world.combat.as_mut().unwrap().levels[0].stats;
            (stats.patk, stats.satk) = (0, 0)
        },
        |w| w.world.combat.as_mut().unwrap().special_name = " ".into(),
        |w| w.world.combat.as_mut().unwrap().cross_share = 101,
        // Out-of-range stats and share together must report, not overflow.
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            combat.cross_share = u32::MAX;
            combat.levels[0].stats.satk = u32::MAX;
            wolf(w).combat.as_mut().unwrap().stats.satk = u32::MAX;
        },
        |w| wolf(w).combat.as_mut().unwrap().stats.hp = 0,
        |w| wolf(w).combat.as_mut().unwrap().stats.patk = 0,
        |w| {
            wolf(w)
                .combat
                .as_mut()
                .unwrap()
                .skills
                .push("missing".into())
        },
        |w| wolf(w).combat.as_mut().unwrap().loot[0].quantity = 0,
        |w| w.locations[0].characters.push("wolf".into()),
        |w| w.world.combat.as_mut().unwrap().narrative.attack.clear(),
        |w| w.world.combat.as_mut().unwrap().narrative.attack[0].0 = "{unknown}".into(),
        |w| w.world.combat.as_mut().unwrap().narrative.attack[0].0 = "{damage".into(),
        |w| w.world.combat.as_mut().unwrap().narrative.attack[0].0 = "damage}".into(),
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
    wolf(&mut world).combat.as_mut().unwrap().stats.hp = 0;
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

#[test]
fn a_world_without_combat_loads_and_roundtrips() {
    let world = archive();
    assert!(world.combat().is_none());
    assert!(world.characters.iter().all(|c| c.combat.is_none()));
    let encoded = serde_json::to_string(&world).unwrap();
    assert!(!encoded.contains("combat"));
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
}

#[test]
fn a_world_without_combat_refuses_fighting_content() {
    let mut fighter = archive();
    fighter.characters[2].combat = Some(profile());
    let mut defeat = archive();
    defeat.quests[0].objective = QuestObjective::Defeat {
        character: "copyist".into(),
    };
    let mut xp = archive();
    xp.quests[0].reward_xp = 5;
    let mut rest = archive();
    rest.locations[0].safe = true;
    for world in [fighter, defeat, xp, rest] {
        assert!(codes(&world).contains(&"combat_disabled".to_string()));
    }
}

#[test]
fn the_player_is_an_unplaced_character_without_components() {
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.characters[0].dialogue = Some("mara".into()),
        |w| w.characters[0].combat = Some(profile()),
        |w| w.locations[0].characters.push("you".into()),
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = demo();
        change(&mut world);
        assert_eq!(codes(&world), ["invalid_player"], "player case {index}");
    }
}

#[test]
fn quest_givers_talk_and_defeat_targets_fight() {
    let mut giver = demo();
    giver.quests[0].giver = "wolf".into();
    assert_eq!(codes(&giver), ["invalid_giver"]);
    let mut target = demo();
    target.quests[0].objective = QuestObjective::Defeat {
        character: "elder".into(),
    };
    assert!(codes(&target).contains(&"invalid_target".to_string()));
}

#[test]
fn older_packages_are_rejected_clearly() {
    let temp = std::env::temp_dir().join(format!("realmkit-older-{}", std::process::id()));
    for (version, world) in [
        (
            1,
            r#"{ "format_version": 1, "id": "old", "player_name": "You", "levels": [] }"#,
        ),
        // M3a's combat block, which this format no longer accepts.
        (
            2,
            r#"{ "format_version": 2, "combat": { "levels": [{ "xp": 0, "hp": 1, "attack": 1 }] } }"#,
        ),
        // M4b's format, readable by runtimes that know no equipment.
        (
            8,
            r#"{ "format_version": 8, "combat": { "special_name": "Magic" } }"#,
        ),
        // M4a's format, readable by runtimes that know no techniques.
        (
            7,
            r#"{ "format_version": 7, "combat": { "special_name": "Magic" } }"#,
        ),
        // M3d's format, readable by runtimes that know no stat points.
        (
            6,
            r#"{ "format_version": 6, "combat": { "special_name": "Magic" } }"#,
        ),
        // M3c-2's format, readable by runtimes that know no crits.
        (
            5,
            r#"{ "format_version": 5, "combat": { "special_name": "Magic" } }"#,
        ),
        // M3c-1's opponents used every listed skill regardless of level.
        (
            4,
            r#"{ "format_version": 4, "combat": { "special_name": "Magic" } }"#,
        ),
        // M3b's combat block has no timeline.
        (
            3,
            r#"{ "format_version": 3, "combat": { "special_name": "Magic" } }"#,
        ),
    ] {
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir(&temp).unwrap();
        std::fs::write(temp.join("world.json"), world).unwrap();
        let error = WorldSpec::load(&temp).unwrap_err();
        std::fs::remove_dir_all(&temp).unwrap();
        assert!(matches!(error, SpecError::UnsupportedFormat { found: Some(v) } if v == version));
        assert!(error
            .to_string()
            .contains("older packages are not migrated"));
    }
}

#[test]
fn packages_load_from_memory_as_from_a_directory() {
    let files = |name: &str| -> std::io::Result<Vec<u8>> {
        let bytes: &[u8] = match name {
            "world.json" => include_bytes!("../../../examples/demo-world/world.json"),
            "locations.json" => include_bytes!("../../../examples/demo-world/locations.json"),
            "characters.json" => include_bytes!("../../../examples/demo-world/characters.json"),
            "items.json" => include_bytes!("../../../examples/demo-world/items.json"),
            "quests.json" => include_bytes!("../../../examples/demo-world/quests.json"),
            "dialogues.json" => include_bytes!("../../../examples/demo-world/dialogues.json"),
            _ => return Err(std::io::ErrorKind::NotFound.into()),
        };
        Ok(bytes.to_vec())
    };
    assert_eq!(WorldSpec::from_files(files).unwrap(), demo());
    for name in PACKAGE_FILES {
        let missing = WorldSpec::from_files(|f| {
            if f == name {
                Err(std::io::ErrorKind::NotFound.into())
            } else {
                files(f)
            }
        });
        assert!(
            matches!(&missing, Err(SpecError::Io { path, .. }) if path == std::path::Path::new(name)),
            "{name}"
        );
    }
}

#[test]
fn place_knowledge_and_features_are_checked() {
    let flag = |f: &str| Condition::Flag { flag: f.into() };
    // Knowledge of a place belongs to the map.
    let mut world = demo();
    world.locations[1].known_when = Some(flag("ruins_open"));
    assert!(codes(&world).contains(&"map_disabled".to_string()));
    let mut world = marches();
    world.locations[1].known_when = Some(flag("missing"));
    assert!(codes(&world).contains(&"missing_reference".to_string()));
    // A feature is examined, so it has dialogue and neither fights nor moves.
    let mut world = demo();
    world
        .characters
        .iter_mut()
        .find(|c| c.id == "elder")
        .unwrap()
        .kind = CharacterKind::Feature;
    assert!(world.diagnostics().is_empty(), "{:?}", codes(&world));
    wolf(&mut world).kind = CharacterKind::Feature;
    assert!(codes(&world).contains(&"invalid_feature".to_string()));
}

#[test]
fn a_place_known_by_a_passing_condition_draws_a_warning() {
    let warnings = |w: &WorldSpec| -> Vec<String> {
        w.diagnostics()
            .into_iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| d.code)
            .collect()
    };
    let flag = Condition::Flag {
        flag: "letter_delivered".into(),
    };
    let mut world = marches();
    // Lasting knowledge, even composed, warns of nothing.
    world.locations[1].known_when = Some(Condition::Any {
        of: vec![
            flag.clone(),
            Condition::Quest {
                quest: world.quests[0].id.clone(),
                status: QuestStatus::Completed,
            },
        ],
    });
    assert!(warnings(&world).is_empty(), "{:?}", warnings(&world));
    // An active quest, an item or a `not` can stop holding.
    for passing in [
        Condition::Quest {
            quest: world.quests[0].id.clone(),
            status: QuestStatus::Active,
        },
        Condition::Not {
            condition: Box::new(flag.clone()),
        },
        Condition::All {
            of: vec![flag, Condition::Currency { amount: 1 }],
        },
    ] {
        world.locations[1].known_when = Some(passing);
        assert_eq!(warnings(&world), ["forgettable_place"]);
        assert!(world.validate().is_ok());
    }
}
