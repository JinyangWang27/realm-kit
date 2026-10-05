//! Technique ranks: teaching, training, gates and passives.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn learned(engine: &Engine<'_>, technique: &str) -> Option<TechniqueState> {
    combat(engine).techniques.get(technique).copied()
}

fn palm(skill: &str) -> Command {
    UseSkill {
        skill: skill.into(),
        target: "dummy".into(),
    }
}

/// Talks to Elder Qing and takes the Cloud Palm lesson.
fn learn_palm(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Talk("qing".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap()
}

#[test]
fn a_starting_internal_art_adds_its_rank_bonus() {
    let world = sect();
    let engine = Engine::new(&world).unwrap();
    assert_eq!(
        learned(&engine, "azure_breath"),
        Some(TechniqueState { rank: 1, xp: 0 })
    );
    // First Layer: +10 MP, +2 Qi attack, and play starts at the full pool.
    let stats = engine.player_stats().unwrap();
    assert_eq!((stats.mp, stats.satk), (30, 12));
    assert_eq!(vitals(&engine).mp, 30);
}

#[test]
fn teaching_a_technique_makes_its_rank_skill_usable() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    let events = learn_palm(&mut engine);
    assert!(events.contains(&Event::TechniqueLearned {
        technique: "cloud_palm".into()
    }));
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("dummy".into())).unwrap();
    assert!(offered(&engine).contains(&(palm("palm_drifting"), true)));
    assert!(matches!(
        engine.execute(palm("palm_storm")),
        Err(EngineError::UnknownSkill(_))
    ));
}

#[test]
fn use_trains_a_technique_through_its_named_ranks() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("dummy".into())).unwrap();
    let events = engine.execute(palm("palm_drifting")).unwrap();
    assert!(events.contains(&Event::TechniqueXpGained {
        technique: "cloud_palm".into(),
        amount: 10
    }));
    let mut all = events;
    while learned(&engine, "cloud_palm").unwrap().rank == 1 {
        if engine.encounter().is_none() {
            engine.execute(Engage("dummy".into())).unwrap();
        }
        all.extend(engine.execute(palm("palm_drifting")).unwrap());
    }
    // Three uses reach 30 XP: Gathering Storm, whose skill replaces the old one.
    assert!(all.contains(&Event::TechniqueRankUp {
        technique: "cloud_palm".into(),
        rank: 2
    }));
    assert_eq!(learned(&engine, "cloud_palm").unwrap().xp, 30);
    if engine.encounter().is_none() {
        engine.execute(Engage("dummy".into())).unwrap();
    }
    assert!(matches!(
        engine.execute(palm("palm_drifting")),
        Err(EngineError::UnknownSkill(_))
    ));
    engine.execute(palm("palm_storm")).unwrap();
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn technique_xp_from_use_falls_off_like_character_xp() {
    let mut world = sect();
    combatant(&mut world, "dummy").level = 3;
    let mut engine = Engine::new(&world).unwrap();
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("dummy".into())).unwrap();
    // Two levels above the player: 120% of 10.
    let events = engine.execute(palm("palm_drifting")).unwrap();
    assert!(events.contains(&Event::TechniqueXpGained {
        technique: "cloud_palm".into(),
        amount: 12
    }));
}

#[test]
fn an_internal_art_rises_by_a_small_share_of_victory_xp() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("dummy".into())).unwrap();
    let events = fight_out(&mut engine);
    // A 5-XP dummy at a 20% share: one point of Azure Breath.
    assert!(events.contains(&Event::ExperienceGranted { amount: 5 }));
    assert!(events.contains(&Event::TechniqueXpGained {
        technique: "azure_breath".into(),
        amount: 1
    }));
    assert_eq!(learned(&engine, "azure_breath").unwrap().xp, 1);
}

#[test]
fn a_breakthrough_gate_holds_xp_until_it_opens() {
    // Start with 40 XP of Azure Breath: Second Layer, waiting at the sealed third.
    let mut world = sect();
    world.world.combat.as_mut().unwrap().player_techniques[0].xp = 40;
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(
        learned(&engine, "azure_breath"),
        Some(TechniqueState { rank: 2, xp: 30 })
    );
    assert_eq!(engine.player_stats().unwrap().mp, 20 + 20);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    // Accept Wen's quest, then learn from Qing where the volume is: the flag
    // opens the gate at once.
    engine.execute(Move(East)).unwrap();
    engine.execute(Talk("wen".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Talk("qing".into())).unwrap();
    let events = engine.execute(ChooseDialogue(2)).unwrap();
    assert!(events.contains(&Event::TechniqueRankUp {
        technique: "azure_breath".into(),
        rank: 3
    }));
    assert_eq!(engine.player_stats().unwrap().mp, 20 + 35);
}

#[test]
fn a_quest_reward_can_teach_technique_xp() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(East)).unwrap();
    engine.execute(Talk("wen".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Talk("qing".into())).unwrap();
    engine.execute(ChooseDialogue(2)).unwrap();
    engine.execute(Move(East)).unwrap();
    engine.execute(Talk("wen".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::TechniqueXpGained {
        technique: "azure_breath".into(),
        amount: 15
    }));
    assert_eq!(learned(&engine, "azure_breath").unwrap().rank, 2);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn saves_reject_technique_states_the_rules_could_not_reach() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    learn_palm(&mut engine);
    let good = engine.snapshot();
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        |s| {
            let t = &mut s.state.combat.as_mut().unwrap().techniques;
            t.insert("missing".into(), TechniqueState { rank: 1, xp: 0 });
        },
        |s| {
            let t = &mut s.state.combat.as_mut().unwrap().techniques;
            t.insert("cloud_palm".into(), TechniqueState { rank: 0, xp: 0 });
        },
        |s| {
            let t = &mut s.state.combat.as_mut().unwrap().techniques;
            t.insert("cloud_palm".into(), TechniqueState { rank: 3, xp: 99 });
        },
        // Second Layer needs 10 XP.
        |s| {
            let t = &mut s.state.combat.as_mut().unwrap().techniques;
            t.insert("azure_breath".into(), TechniqueState { rank: 2, xp: 5 });
        },
        // At an open threshold the rank would already have risen.
        |s| {
            let t = &mut s.state.combat.as_mut().unwrap().techniques;
            t.insert("cloud_palm".into(), TechniqueState { rank: 1, xp: 30 });
        },
        // The starting art is never forgotten.
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .techniques
                .remove("azure_breath");
        },
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(&mut snapshot);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
}

/// Accepts Wen's quest, fetches the volume from Qing, and hands it in.
fn finish_lost_volume(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(East)).unwrap();
    engine.execute(Talk("wen".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Talk("qing".into())).unwrap();
    engine.execute(ChooseDialogue(2)).unwrap();
    engine.execute(Move(East)).unwrap();
    engine.execute(Talk("wen".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap()
}

#[test]
fn a_smaller_rank_bonus_keeps_vitals_within_the_new_maxima() {
    // Second Layer here gives less MP than First Layer did.
    let mut world = sect();
    let ranks = &mut world.world.combat.as_mut().unwrap().techniques[0].ranks;
    ranks[1].passive.insert(Stat::Mp, 5);
    ranks[2].passive.insert(Stat::Mp, 5);
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(vitals(&engine).mp, 30);
    finish_lost_volume(&mut engine);
    assert_eq!(learned(&engine, "azure_breath").unwrap().rank, 2);
    assert_eq!(vitals(&engine).mp, 25);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn xp_held_back_by_a_gate_is_not_reported_as_gained() {
    // 25 XP at Second Layer; the sealed Third Layer waits at 30.
    let mut world = sect();
    world.world.combat.as_mut().unwrap().player_techniques[0].xp = 25;
    combatant(&mut world, "dummy").xp = 50;
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("dummy".into())).unwrap();
    let events = fight_out(&mut engine);
    // A 20% share of 50 is 10, but only 5 fit below the gate.
    assert!(events.contains(&Event::TechniqueXpGained {
        technique: "azure_breath".into(),
        amount: 5
    }));
    engine.execute(Engage("dummy".into())).unwrap();
    let events = fight_out(&mut engine);
    assert!(!events.iter().any(
        |e| matches!(e, Event::TechniqueXpGained { technique, .. } if technique == "azure_breath")
    ));
}

#[test]
fn a_save_cannot_hold_a_technique_nothing_teaches() {
    let mut world = sect();
    let mut hidden = world.world.combat.as_ref().unwrap().techniques[0].clone();
    hidden.id = "hidden_art".into();
    world.world.combat.as_mut().unwrap().techniques.push(hidden);
    let engine = Engine::new(&world).unwrap();
    let mut snapshot = engine.snapshot();
    snapshot
        .state
        .combat
        .as_mut()
        .unwrap()
        .techniques
        .insert("hidden_art".into(), TechniqueState { rank: 1, xp: 0 });
    assert!(Engine::restore(&world, snapshot).is_err());
}

#[test]
fn a_technique_condition_tests_the_rank_reached() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    let storm = Condition::Technique {
        technique: "cloud_palm".into(),
        rank: 2,
    };
    let breath = Condition::Technique {
        technique: "azure_breath".into(),
        rank: 1,
    };
    assert!(engine.holds(&breath));
    assert!(!engine.holds(&storm));
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    while learned(&engine, "cloud_palm").unwrap().rank == 1 {
        if engine.encounter().is_none() {
            engine.execute(Engage("dummy".into())).unwrap();
        }
        engine.execute(palm("palm_drifting")).unwrap();
    }
    assert!(engine.holds(&storm));
}

#[test]
fn a_mastered_technique_gains_no_more_xp() {
    let world = sect();
    let mut engine = Engine::new(&world).unwrap();
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    while learned(&engine, "cloud_palm").unwrap().rank == 1 {
        if engine.encounter().is_none() {
            engine.execute(Engage("dummy".into())).unwrap();
        }
        engine.execute(palm("palm_drifting")).unwrap();
    }
    if engine.encounter().is_none() {
        engine.execute(Engage("dummy".into())).unwrap();
    }
    let events = engine.execute(palm("palm_storm")).unwrap();
    assert!(!events.iter().any(
        |e| matches!(e, Event::TechniqueXpGained { technique, .. } if technique == "cloud_palm")
    ));
    assert_eq!(learned(&engine, "cloud_palm").unwrap().xp, 30);
}

#[test]
fn a_rank_that_lowers_max_hp_carries_rage_progress_over() {
    // Drifting Cloud adds 30 HP; Gathering Storm adds none, so the rank-up
    // mid-fight shrinks the maximum that rage progress is counted in.
    let mut world = sect();
    let combat = world.world.combat.as_mut().unwrap();
    // Each 1-damage bite adds 30/80 of a rage point; three palms fit in one
    // fight, so 60 progress meets the rank-up's new maximum of 50.
    combat.resources.rage_per_max_hp = 30;
    combat.techniques[1].ranks[0].passive.insert(Stat::Hp, 30);
    // At speed 70 the dummy bites at ticks 1429 and 2858; the third palm lands
    // at 3000, and the player's next turn (4000) comes before its next bite.
    let dummy = combatant(&mut world, "dummy");
    (dummy.stats.hp, dummy.stats.speed) = (60, 70);
    let mut engine = Engine::new(&world).unwrap();
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    while learned(&engine, "cloud_palm").unwrap().rank == 1 {
        if engine.encounter().is_none() {
            engine.execute(Engage("dummy".into())).unwrap();
        }
        engine.execute(palm("palm_drifting")).unwrap();
        if let Some(encounter) = engine.encounter() {
            let max = engine.player_stats().unwrap().hp;
            assert!(encounter.participants[0].rage_remainder < u64::from(max));
            assert!(Engine::restore(&world, engine.snapshot()).is_ok());
        }
    }
}
