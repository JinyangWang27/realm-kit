//! Stat points and techniques, checked against their worst case.

mod common;

use common::*;
use realmkit_spec::*;

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
