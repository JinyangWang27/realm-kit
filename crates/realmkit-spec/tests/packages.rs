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
        basic_crit: None,
        level: 1,
        group: None,
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
        |w| w.world.combat.as_mut().unwrap().skills[1].time = TIME_BOUNDS.1 + 1,
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
    // A rage skill in a world with no rage source can never be paid for.
    let mut sourceless = duel();
    let bolt = &mut sourceless.world.combat.as_mut().unwrap().skills[1];
    (bolt.resource, bolt.cost) = (Resource::Rage, 5);
    assert!(sourceless.validate().is_ok());
    let warnings = sourceless.diagnostics();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        (warnings[0].code.as_str(), warnings[0].entity_id.as_deref()),
        ("unusable_skill", Some("bolt"))
    );
}

fn arena() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap()
}

fn fighter<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut CombatProfile {
    w.characters
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
}

#[test]
fn groups_and_profile_levels_are_validated() {
    let world = arena();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    assert!(world.group("warren").unwrap().repeatable);
    assert_eq!(fighter(&mut arena(), "ogre").level, 4);
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| fighter(w, "rat").group = Some("missing".into()),
        |w| fighter(w, "rat").level = 0,
        |w| {
            let groups = &mut w.world.combat.as_mut().unwrap().groups;
            groups.push(groups[0].clone())
        },
        |w| w.world.combat.as_mut().unwrap().groups[2].yield_share = Some(0),
        |w| w.world.combat.as_mut().unwrap().groups[2].yield_share = Some(101),
        |w| {
            w.world.combat.as_mut().unwrap().groups[2]
                .victory_flags
                .push("missing".into())
        },
        |w| {
            w.world.combat.as_mut().unwrap().groups[2]
                .defeat_flags
                .push("missing".into())
        },
        // A yielder never dies, so defeating it can never complete a quest.
        |w| {
            w.characters.push(Character {
                id: "sarge".into(),
                name: "Sarge".into(),
                description: "Gives orders.".into(),
                requires: vec![],
                dialogue: Some("orders".into()),
                combat: None,
            });
            w.dialogues.push(Dialogue {
                id: "orders".into(),
                start: "hello".into(),
                nodes: vec![DialogueNode {
                    id: "hello".into(),
                    text: "Beat Holt.".into(),
                    choices: vec![],
                }],
            });
            w.locations[0].characters.push("sarge".into());
            w.quests.push(Quest {
                id: "beat_holt".into(),
                name: "Beat Holt".into(),
                giver: "sarge".into(),
                objective: QuestObjective::Defeat {
                    character: "holt".into(),
                },
                introduction: "Go.".into(),
                progress: "Done.".into(),
                completion: "Well done.".into(),
                reward_xp: 0,
                reward_items: vec![],
                completion_flags: vec![],
                reward_techniques: vec![],
            })
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = arena();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid group case {index}");
    }
}

#[test]
fn profile_skills_unlock_at_the_profile_level() {
    // The ogre is level 4: crush (level 5) is locked, so its affordability is not checked.
    let mut world = arena();
    let combat = world.world.combat.as_mut().unwrap();
    let crush = combat.skills.iter_mut().find(|s| s.id == "crush").unwrap();
    (crush.resource, crush.cost) = (Resource::Mp, 50);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    fighter(&mut world, "ogre").level = 5;
    assert!(world.validate().is_err());
}

#[test]
fn crits_are_bounded_and_make_a_world_stochastic() {
    let crit = |chance_percent, multiplier_percent| {
        Some(Crit {
            chance_percent,
            multiplier_percent,
        })
    };
    assert!(!duel().stochastic() && !archive().stochastic() && arena().stochastic());
    let mut skill = duel();
    skill.world.combat.as_mut().unwrap().skills[0].crit = crit(25, 150);
    let mut basic = duel();
    basic.world.combat.as_mut().unwrap().player_basic_crit = crit(100, 101);
    let mut foe = duel();
    fighter(&mut foe, "witch").basic_crit = crit(1, 1_000);
    for world in [skill, basic, foe] {
        assert!(world.stochastic());
        assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    }
    for (chance, multiplier) in [(0, 150), (101, 150), (25, 100), (25, 1_001)] {
        let mut world = duel();
        world.world.combat.as_mut().unwrap().skills[0].crit = crit(chance, multiplier);
        assert!(world.validate().is_err(), "crit {chance}/{multiplier}");
    }
}

#[test]
fn stat_points_are_validated_against_their_worst_case() {
    let world = arena();
    let points = world.combat().unwrap().stat_points.clone().unwrap();
    assert_eq!(points.values[&Stat::Hp], 5);
    assert_eq!(points.respec, Respec::Safe);
    assert_eq!(world.combat().unwrap().levels[0].points, 3);
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        // Points with nothing to spend them on.
        |w| w.world.combat.as_mut().unwrap().stat_points = None,
        |w| {
            let p = w
                .world
                .combat
                .as_mut()
                .unwrap()
                .stat_points
                .as_mut()
                .unwrap();
            p.values.clear()
        },
        |w| {
            let p = w
                .world
                .combat
                .as_mut()
                .unwrap()
                .stat_points
                .as_mut()
                .unwrap();
            p.values.insert(Stat::Mp, 0);
        },
        |w| {
            let p = w
                .world
                .combat
                .as_mut()
                .unwrap()
                .stat_points
                .as_mut()
                .unwrap();
            p.caps.insert(Stat::Sdef, 3);
        },
        // Every point in HP at 5,000 each would pass 9,999.
        |w| {
            let p = w
                .world
                .combat
                .as_mut()
                .unwrap()
                .stat_points
                .as_mut()
                .unwrap();
            p.values.insert(Stat::Hp, 5_000);
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = arena();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid points case {index}");
    }
    // A cap keeps the worst case in bounds.
    let mut capped = arena();
    let p = capped
        .world
        .combat
        .as_mut()
        .unwrap()
        .stat_points
        .as_mut()
        .unwrap();
    p.values.insert(Stat::Hp, 5_000);
    p.caps.insert(Stat::Hp, 1);
    assert!(capped.validate().is_ok(), "{:?}", capped.diagnostics());
    // Stat points nobody grants only warn.
    let mut unused = arena();
    for level in &mut unused.world.combat.as_mut().unwrap().levels {
        level.points = 0;
    }
    assert!(unused.validate().is_ok());
    assert_eq!(unused.diagnostics()[0].code, "unused_points");
}

#[test]
fn caps_that_cannot_take_every_point_warn() {
    let mut world = arena();
    let points = world
        .world
        .combat
        .as_mut()
        .unwrap()
        .stat_points
        .as_mut()
        .unwrap();
    points.values.retain(|stat, _| *stat == Stat::Patk);
    points.caps = [(Stat::Patk, 1)].into();
    assert!(world.validate().is_ok());
    let codes: Vec<_> = world.diagnostics().into_iter().map(|d| d.code).collect();
    assert_eq!(codes, ["stranded_points"]);
}

fn sect() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap()
}

#[test]
fn techniques_have_named_rising_ranks_and_valid_grants() {
    let world = sect();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let breath = world.technique("azure_breath").unwrap();
    assert_eq!(breath.ranks[2].name, "Third Layer");
    assert_eq!(breath.ranks[1].passive[&Stat::Mp], 20);
    assert_eq!(world.combat().unwrap().technique_xp_per_use, 10);
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| w.world.combat.as_mut().unwrap().techniques[0].ranks[1].name = " ".into(),
        |w| w.world.combat.as_mut().unwrap().techniques[0].ranks[0].xp = 1,
        |w| w.world.combat.as_mut().unwrap().techniques[0].ranks[2].xp = 10,
        |w| w.world.combat.as_mut().unwrap().techniques[0].ranks.clear(),
        |w| w.world.combat.as_mut().unwrap().techniques[0].xp_share_percent = 101,
        |w| w.world.combat.as_mut().unwrap().techniques[1].ranks[0].skill = Some("missing".into()),
        |w| {
            w.world.combat.as_mut().unwrap().techniques[0].ranks[2].requires =
                vec![Condition::Flag {
                    flag: "missing".into(),
                }]
        },
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            combat.techniques.push(combat.techniques[0].clone())
        },
        |w| {
            w.world
                .combat
                .as_mut()
                .unwrap()
                .player_skills
                .push("palm_drifting".into())
        },
        |w| w.world.combat.as_mut().unwrap().core_art = Some("missing".into()),
        |w| w.world.combat.as_mut().unwrap().player_techniques[0].rank = Some(4),
        |w| w.world.combat.as_mut().unwrap().player_techniques[0].rank = Some(0),
        |w| w.quests[0].reward_techniques[0].technique = "missing".into(),
        // A rank skill the player could never unlock or pay for.
        |w| w.world.combat.as_mut().unwrap().skills[0].level = 9,
        |w| w.world.combat.as_mut().unwrap().skills[1].cost = 99,
        // One skill in two techniques could train only one of them.
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            combat.techniques[0].ranks[0].skill = Some("palm_drifting".into())
        },
        // A dialogue choice can be repeated, so it may not hand out XP.
        |w| {
            let choice = &mut w.dialogues[0].nodes[0].choices[0];
            choice.effect = Some(DialogueEffect::GrantTechnique(TechniqueGrant {
                technique: "cloud_palm".into(),
                rank: None,
                xp: 5,
            }))
        },
        // Technique conditions name a technique and one of its ranks.
        |w| {
            w.locations[0]
                .exits
                .get_mut(&Direction::North)
                .unwrap()
                .requires = vec![Condition::Technique {
                technique: "missing".into(),
                rank: 1,
            }]
        },
        |w| {
            w.locations[0]
                .exits
                .get_mut(&Direction::North)
                .unwrap()
                .requires = vec![Condition::Technique {
                technique: "cloud_palm".into(),
                rank: 3,
            }]
        },
        // A core art nothing can teach would never show a realm.
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            let mut hidden = combat.techniques[0].clone();
            hidden.id = "hidden_art".into();
            hidden.ranks.iter_mut().for_each(|r| r.passive.clear());
            combat.techniques.push(hidden);
            combat.core_art = Some("hidden_art".into())
        },
        // A passive this large could lift MP past the stat bound.
        |w| {
            w.world.combat.as_mut().unwrap().techniques[0].ranks[2]
                .passive
                .insert(Stat::Mp, STAT_BOUND);
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = sect();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid technique case {index}");
    }
    let mut renamed = sect();
    renamed.world.combat.as_mut().unwrap().techniques[1].name = "Palm of Clouds".into();
    assert!(renamed.validate().is_ok());
}

#[test]
fn a_rank_skill_may_rely_on_its_own_rank_bonus() {
    // Without special attack or MP of its own, the player can still use the
    // palm at Gathering Storm because that rank grants what it needs.
    let mut world = sect();
    let combat = world.world.combat.as_mut().unwrap();
    for level in &mut combat.levels {
        (level.stats.mp, level.stats.satk, level.stats.patk) = (0, 0, 0);
    }
    combat.player_basic_channel = Channel::Physical;
    combat.levels.iter_mut().for_each(|l| l.stats.patk = 1);
    let rank = &mut combat.techniques[1].ranks[1];
    rank.passive = [(Stat::Mp, 10), (Stat::Satk, 5)].into();
    combat.techniques[1].ranks[0].skill = None;
    combat.techniques[0]
        .ranks
        .iter_mut()
        .for_each(|r| r.passive.clear());
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
}

#[test]
fn a_starting_art_can_supply_the_attack_the_player_fights_with() {
    // No attack in the level table: Azure Breath's First Layer gives the Qi
    // attack that the physical basic attack blends in.
    let mut world = sect();
    let combat = world.world.combat.as_mut().unwrap();
    for level in &mut combat.levels {
        (level.stats.patk, level.stats.satk) = (0, 0);
    }
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
}

fn gear<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Equipment {
    w.items
        .iter_mut()
        .find(|i| i.id == id)
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
}

#[test]
fn equipment_occupies_declared_slots_within_bounds() {
    let world = arena();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let greatsword = world.item("greatsword").unwrap().equipment.clone().unwrap();
    assert_eq!(greatsword.slots, ["main_hand", "off_hand"]);
    assert_eq!(greatsword.basic_time, Some(130));
    let charm = world
        .item("charm_of_warding")
        .unwrap()
        .equipment
        .clone()
        .unwrap();
    assert_eq!(
        charm.modifiers[&Channel::Special],
        Modifier { num: 1, den: 2 }
    );
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| gear(w, "buckler").slots.clear(),
        |w| gear(w, "buckler").slots.push("off_hand".into()),
        |w| gear(w, "buckler").slots.push("tail".into()),
        |w| {
            gear(w, "buckler")
                .bonuses
                .insert(Stat::Pdef, STAT_BOUND + 1);
        },
        |w| gear(w, "iron_mail").speed_penalty = STAT_BOUND + 1,
        |w| gear(w, "greatsword").basic_time = Some(0),
        |w| {
            gear(w, "charm_of_warding")
                .modifiers
                .insert(Channel::Physical, Modifier { num: 11, den: 1 });
        },
        |w| {
            gear(w, "charm_of_warding")
                .modifiers
                .insert(Channel::Physical, Modifier { num: 1, den: 0 });
        },
        |w| {
            w.world
                .combat
                .as_mut()
                .unwrap()
                .player_equipment
                .push("rat_tail".into())
        },
        |w| {
            let slots = &mut w.world.combat.as_mut().unwrap().slots;
            slots.push(slots[0].clone())
        },
        // Each wearable one is its own piece, so grants stay small.
        |w| {
            let wolf = w
                .characters
                .iter_mut()
                .find(|c| c.id == "black_wolf")
                .unwrap();
            wolf.combat.as_mut().unwrap().loot[0].quantity = GEAR_STACK_BOUND + 1;
        },
        |w| {
            let slots = &mut w.world.combat.as_mut().unwrap().slots;
            slots.extend((0..SLOT_BOUND).map(|i| format!("extra_{i}")));
        },
        // A special-channel weapon for a player with no special attack at all.
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            combat.cross_share = 0;
            combat.levels.iter_mut().for_each(|l| l.stats.satk = 0);
            gear(w, "greatsword").basic_channel = Some(Channel::Special);
        },
        // Two weapons that can be worn together would compete for the attack.
        |w| {
            gear(w, "charm_of_warding").basic_time = Some(90);
        },
        // Every best piece at once could lift defence past the stat bound.
        |w| {
            gear(w, "iron_mail").bonuses.insert(Stat::Pdef, 9_990);
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = arena();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid equipment case {index}");
    }
    // Immunity is a modifier of 0 over anything.
    let mut immune = arena();
    gear(&mut immune, "charm_of_warding")
        .modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    assert!(immune.validate().is_ok());
    // Nothing can be worn in a world without combat.
    let mut quiet = archive();
    quiet.items[0].equipment = Some(arena().item("buckler").unwrap().equipment.clone().unwrap());
    assert!(codes(&quiet).contains(&"combat_disabled".to_string()));
}

#[test]
fn a_two_handed_bonus_counts_once_in_the_worst_case() {
    // 9,000 attack on a piece filling two slots is 9,000 worn, not 18,000.
    let mut world = arena();
    gear(&mut world, "greatsword")
        .bonuses
        .insert(Stat::Patk, 9_000);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
}
