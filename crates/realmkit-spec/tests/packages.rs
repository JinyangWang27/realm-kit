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
        characters: serde_json::from_str(include_str!(
            "../../../examples/demo-world/characters.json"
        ))
        .unwrap(),
        items: serde_json::from_str(include_str!("../../../examples/demo-world/items.json"))
            .unwrap(),
        quests: serde_json::from_str(include_str!("../../../examples/demo-world/quests.json"))
            .unwrap(),
        dialogues: serde_json::from_str(include_str!(
            "../../../examples/demo-world/dialogues.json"
        ))
        .unwrap(),
    }
}

fn archive() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/quiet-archive"
    ))
    .unwrap()
}

fn wolf(w: &mut WorldSpec) -> &mut Character {
    w.characters.iter_mut().find(|c| c.id == "wolf").unwrap()
}

fn duel() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel")).unwrap()
}

fn profile() -> CombatProfile {
    CombatProfile {
        stats: wolf(&mut demo()).combat.clone().unwrap().stats,
        xp: 1,
        loot: vec![],
        skills: vec![],
        basic_channel: Channel::Physical,
    }
}

fn codes(w: &WorldSpec) -> Vec<String> {
    w.diagnostics().into_iter().map(|d| d.code).collect()
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
            w.dialogues[0].nodes[0].choices[0].effect = Some(DialogueEffect::AcceptQuest {
                quest: "missing".into(),
            })
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
fn skills_are_bounded_referenced_usable_and_affordable() {
    let world = duel();
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    assert_eq!(world.skill("bolt").unwrap().cost, 12);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.world.combat.as_mut().unwrap().skills[1].power = 0,
        |w| w.world.combat.as_mut().unwrap().skills[1].power = POWER_BOUNDS.1 + 1,
        |w| w.world.combat.as_mut().unwrap().skills[1].level = 0,
        |w| w.world.combat.as_mut().unwrap().skills[1].cross_share = Some(101),
        |w| w.world.combat.as_mut().unwrap().skills[1].text.0 = "{skill}".into(),
        |w| w.world.combat.as_mut().unwrap().skills[1].id = "spark".into(),
        |w| {
            w.world
                .combat
                .as_mut()
                .unwrap()
                .player_skills
                .push("missing".into())
        },
        |w| {
            w.world
                .combat
                .as_mut()
                .unwrap()
                .player_skills
                .push("bolt".into())
        },
        // Fireball unlocks at level 2; level 3 does not exist.
        |w| w.world.combat.as_mut().unwrap().skills[2].level = 3,
        // More MP than the player has at the level the skill unlocks.
        |w| w.world.combat.as_mut().unwrap().skills[1].cost = 25,
        |w| w.world.combat.as_mut().unwrap().skills[1].time = 0,
        |w| w.world.combat.as_mut().unwrap().timeline.action_cost = 0,
        |w| w.world.combat.as_mut().unwrap().timeline.action_cost = ACTION_COST_BOUND + 1,
        |w| w.world.combat.as_mut().unwrap().timeline.speed_cap = 0,
        |w| w.world.combat.as_mut().unwrap().timeline.speed_cap = STAT_BOUND + 1,
        // Hex costs 5; the witch has 10.
        |w| w.characters[1].combat.as_mut().unwrap().stats.mp = 4,
        // A special skill with no attack in either channel.
        |w| {
            let stats = &mut w.world.combat.as_mut().unwrap().levels[0].stats;
            (stats.satk, stats.patk) = (0, 0)
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = duel();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid skill case {index}");
    }
}

#[test]
fn the_cross_share_blends_channels_and_defaults_to_25() {
    let world = duel();
    assert_eq!(world.combat().unwrap().cross_share, 25);
    let stats = world.combat().unwrap().levels[0].stats;
    assert_eq!(
        stats.combined(Channel::Special, 25, false),
        100 * 10 + 25 * 3
    );
    assert_eq!(
        stats.combined(Channel::Physical, 25, true),
        100 * 4 + 25 * 6
    );
    assert_eq!(stats.combined(Channel::Physical, 0, false), 300);
}

#[test]
fn rage_skills_need_no_mp_and_unused_resources_only_warn() {
    // A rage cost above every MP pool is fine: rage accumulates in a fight.
    let mut world = duel();
    let combat = world.world.combat.as_mut().unwrap();
    let bolt = &mut combat.skills[1];
    (bolt.resource, bolt.cost) = (Resource::Rage, 99);
    combat.resources.rage_per_action = 1;
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    // Rage constants with no rage skill are harmless, so they only warn.
    let mut unused = duel();
    unused
        .world
        .combat
        .as_mut()
        .unwrap()
        .resources
        .rage_per_action = 1;
    assert!(unused.validate().is_ok());
    let warnings = unused.diagnostics();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        (warnings[0].code.as_str(), warnings[0].severity),
        ("unused_resource", Severity::Warning)
    );
}
