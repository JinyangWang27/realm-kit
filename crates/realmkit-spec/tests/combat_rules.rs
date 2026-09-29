//! Combat rules: skills, channels, resources, groups, profiles and crits.

mod common;

use common::*;
use realmkit_spec::*;

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
