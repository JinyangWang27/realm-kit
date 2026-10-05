//! The authored quest loop: moving, talking, fighting, rewards, actions and saves.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn accept(engine: &mut Engine<'_>) {
    engine.execute(Talk("elder".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
}

fn kill(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("wolf".into())).unwrap();
    engine.execute(Attack("wolf".into())).unwrap();
    engine.execute(Attack("wolf".into())).unwrap();
    engine.execute(Attack("wolf".into())).unwrap()
}

#[test]
fn full_quest_loop_and_replay_produce_identical_state_and_events() {
    let world = demo();
    let commands = vec![
        Look,
        Talk("elder".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Move(North),
        Engage("wolf".into()),
        Attack("wolf".into()),
        Attack("wolf".into()),
        Attack("wolf".into()),
        Move(South),
        Talk("elder".into()),
        ChooseDialogue(1),
        Move(East),
        Move(Down),
    ];
    let play = || {
        let mut engine = Engine::new(&world).unwrap();
        let mut events = Vec::new();
        for command in commands.clone() {
            events.extend(engine.execute(command).unwrap());
        }
        (engine.state().clone(), events)
    };
    let (state, events) = play();
    assert_eq!((state.clone(), events.clone()), play());
    assert_eq!(state.player.location, "crypt");
    assert_eq!(state.combat.as_ref().unwrap().level, 2);
    assert_eq!(state.combat.as_ref().unwrap().xp, 15);
    // Levelling up restores HP to level 2's maximum, and the fight is over.
    assert_eq!(
        state.combat.as_ref().unwrap().stance,
        Stance::Exploring(Vitals { hp: 48, mp: 0 })
    );
    assert_eq!(state.player.inventory["ash_pelt"], 1);
    assert_eq!(state.player.inventory["candle"], 1);
    assert_eq!(state.quests["quiet_the_track"], QuestStatus::Completed);
    assert!(state.flags.contains("ruins_open"));
    assert!(events.contains(&Event::QuestProgressed {
        quest: "quiet_the_track".into()
    }));
    assert!(events.contains(&Event::LevelUp {
        level: 2,
        mp_restored: false
    }));
}

#[test]
fn rejected_commands_are_atomic_and_remote_targets_cannot_be_used() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    for command in [
        Move(East),
        Move(West),
        Attack("wolf".into()),
        Talk("missing".into()),
        ChooseDialogue(1),
        CompleteQuest("quiet_the_track".into()),
    ] {
        let before = engine.state().clone();
        assert!(engine.execute(command).is_err());
        assert_eq!(engine.state(), &before);
    }
    engine.execute(Talk("elder".into())).unwrap();
    let before = engine.state().clone();
    for choice in [0, 3, usize::MAX] {
        assert!(engine.execute(ChooseDialogue(choice)).is_err());
        assert_eq!(engine.state(), &before);
    }
    engine.execute(Move(North)).unwrap();
    let before = engine.state().clone();
    for command in [
        Talk("elder".into()),
        AcceptQuest("quiet_the_track".into()),
        ChooseDialogue(1),
    ] {
        assert!(engine.execute(command).is_err());
        assert_eq!(engine.state(), &before);
    }
}

#[test]
fn combat_tracks_damage_and_never_rewards_a_defeat_twice() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    accept(&mut engine);
    engine.execute(Move(North)).unwrap();
    // The faster wolf strikes before the player's first command.
    let events = engine.execute(Engage("wolf".into())).unwrap();
    assert!(events.contains(&Event::EncounterStarted {
        opponents: vec!["wolf".into()]
    }));
    assert_eq!(vitals(&engine).hp, 36);
    let events = engine.execute(Attack("wolf".into())).unwrap();
    assert_eq!(foe(&engine).hp, 13);
    assert_eq!(vitals(&engine).hp, 32);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::DamageDealt { amount: 7, .. })));
    engine.execute(Attack("wolf".into())).unwrap();
    let events = engine.execute(Attack("wolf".into())).unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::DamageDealt { amount: 6, .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::DamageReceived { .. })));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    assert!(combat(&engine).defeated.contains("wolf"));
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Attack("wolf".into())),
        Err(EngineError::NotFighting)
    ));
    assert!(matches!(
        engine.execute(Engage("wolf".into())),
        Err(EngineError::AlreadyDefeated(_))
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(Move(South)).unwrap();
    engine
        .execute(CompleteQuest("quiet_the_track".into()))
        .unwrap();
    let before = engine.state().clone();
    assert!(engine
        .execute(CompleteQuest("quiet_the_track".into()))
        .is_err());
    assert!(engine
        .execute(AcceptQuest("quiet_the_track".into()))
        .is_err());
    assert_eq!(engine.state(), &before);
}

#[test]
fn defeating_the_target_before_accepting_does_not_softlock_the_quest() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    kill(&mut engine);
    engine.execute(Move(South)).unwrap();
    accept(&mut engine);
    assert_eq!(engine.state().quests["quiet_the_track"], QuestStatus::Ready);
    engine
        .execute(CompleteQuest("quiet_the_track".into()))
        .unwrap();
}

#[test]
fn death_blocks_actions_but_allows_inspection() {
    let mut world = demo();
    world.characters[2].combat.as_mut().unwrap().stats.patk = STAT_BOUND;
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    let events = engine.execute(Engage("wolf".into())).unwrap();
    assert_eq!(vitals(&engine).hp, 0);
    assert!(events.contains(&Event::PlayerDied));
    assert!(engine.execute(Move(South)).is_err());
    assert!(engine.execute(Attack("wolf".into())).is_err());
    assert!(engine.execute(Status).is_ok());
    assert_eq!(
        offered(&engine),
        vec![(Inventory, true), (Status, true), (Quests, true)]
    );
    // Whether a dead save is useful is client policy; the engine round-trips it.
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn overflowing_rewards_roll_back_the_entire_command() {
    let mut world = demo();
    world.characters[2].combat.as_mut().unwrap().xp = u64::MAX;
    let mut engine = Engine::new(&world).unwrap();
    accept(&mut engine);
    kill(&mut engine);
    assert_eq!(combat(&engine).level, 3);
    engine.execute(Move(South)).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(CompleteQuest("quiet_the_track".into())),
        Err(EngineError::NumericLimit)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn actions_list_context_sensitive_commands_with_availability() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(
        offered(&engine),
        vec![
            (Talk("elder".into()), true),
            (Move(North), true),
            (Move(East), false),
            (Rest, true),
            (Inventory, true),
            (Status, true),
            (Quests, true),
        ]
    );
    engine.execute(Move(North)).unwrap();
    assert_eq!(offered(&engine)[0], (Engage("wolf".into()), true));
    engine.execute(Engage("wolf".into())).unwrap();
    // In a fight: attacks and skills on each living opponent, then panels only.
    assert_eq!(
        offered(&engine),
        vec![
            (Attack("wolf".into()), true),
            (
                UseSkill {
                    skill: "heavy_swing".into(),
                    target: "wolf".into()
                },
                false
            ),
            (Flee, true),
            (Inventory, true),
            (Status, true),
            (Quests, true),
        ]
    );
    for _ in 0..3 {
        engine.execute(Attack("wolf".into())).unwrap();
    }
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| matches!(c, Attack(_) | Engage(_))));
}

#[test]
fn offered_actions_match_engine_legality_throughout_the_demo() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    let path = [
        Talk("elder".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Move(North),
        Engage("wolf".into()),
        Attack("wolf".into()),
        Attack("wolf".into()),
        Attack("wolf".into()),
        Move(South),
        Talk("elder".into()),
        ChooseDialogue(1),
        Move(East),
        Move(Down),
    ];
    for step in path {
        for action in engine.actions() {
            // Probe on a copy so exploring actions never alters the real run.
            let mut probe = engine.clone();
            assert_eq!(
                probe.execute(action.command.clone()).is_ok(),
                action.available,
                "{:?} at {}",
                action.command,
                engine.state().player.location
            );
        }
        let events = engine.execute(step).unwrap();
        if let Some(Event::Dialogue { choices, .. }) = events
            .iter()
            .rev()
            .find(|e| matches!(e, Event::Dialogue { .. }))
        {
            if engine.state().dialogue.is_some() {
                assert_eq!(&engine.dialogue_choices(), choices);
            }
        }
    }
    assert!(engine.dialogue_choices().is_empty());
}

#[test]
fn viewing_panels_does_not_spend_time() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    let before = engine.state().clone();
    for command in [Look, Inventory, Status, Quests] {
        engine.execute(command).unwrap();
    }
    assert_eq!(engine.state(), &before);
}

#[test]
fn saving_mid_quest_and_resuming_matches_uninterrupted_play() {
    let world = demo();
    let before = [
        Talk("elder".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Move(North),
        Engage("wolf".into()),
        Attack("wolf".into()),
    ];
    let after = [
        Attack("wolf".into()),
        Attack("wolf".into()),
        Move(South),
        Talk("elder".into()),
        ChooseDialogue(1),
    ];
    let mut uninterrupted = Engine::new(&world).unwrap();
    for command in before.clone() {
        uninterrupted.execute(command).unwrap();
    }
    let json = serde_json::to_string(&uninterrupted.snapshot()).unwrap();
    assert_eq!(
        uninterrupted.snapshot(),
        uninterrupted.snapshot(),
        "saving must not change state"
    );
    let mut resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(resumed.state(), uninterrupted.state());
    for command in after {
        assert_eq!(
            resumed.execute(command.clone()).unwrap(),
            uninterrupted.execute(command).unwrap()
        );
    }
    assert_eq!(resumed.state(), uninterrupted.state());
    // A finished quest's rewards, flags and level-up all pass validation.
    assert!(Engine::restore(&world, resumed.snapshot()).is_ok());
}

#[test]
fn mismatched_or_corrupt_saves_are_rejected() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    accept(&mut engine);
    let good = engine.snapshot();
    assert_eq!(good.player_route_id, "default");
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        |s| s.save_format_version = 99,
        |s| s.package_id = "other".into(),
        |s| s.package_revision = "fnv1a64:0".into(),
        |s| s.player_route_id = "other".into(),
        |s| s.state.player.location = "nowhere".into(),
        |s| s.state.combat.as_mut().unwrap().stance = Stance::Exploring(Vitals { hp: 41, mp: 0 }),
        |s| s.state.combat.as_mut().unwrap().stance = Stance::Exploring(Vitals { hp: 1, mp: 1 }),
        |s| s.state.combat.as_mut().unwrap().level = 0,
        |s| s.state.combat.as_mut().unwrap().xp = 10,
        |s| s.state.combat = None,
        |s| {
            s.state.player.inventory.insert("ghost".into(), 1);
        },
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .defeated
                .insert("elder".into());
        },
        |s| {
            s.state.quests.insert("extra".into(), QuestStatus::Active);
        },
        |s| {
            s.state.flags.insert("undeclared".into());
        },
        |s| {
            s.state.player.inventory.insert("ash_pelt".into(), u64::MAX);
        },
        |s| {
            s.state
                .quests
                .insert("quiet_the_track".into(), QuestStatus::Ready);
        },
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .defeated
                .insert("wolf".into());
        },
        |s| s.state.combat.as_mut().unwrap().xp += 1,
        |s| s.state.turn = u64::MAX,
        |s| {
            s.state.player.inventory.insert("ash_pelt".into(), 1);
        },
        |s| {
            s.state.flags.insert("ruins_open".into());
        },
        |s| {
            s.state.dialogue = Some(DialogueState {
                npc: "elder".into(),
                node: "missing".into(),
            })
        },
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(&mut snapshot);
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
    // Saves bind to the rules: a prose fix keeps them, a rule change does not.
    let mut edited = demo();
    edited.items[0].description.push('.');
    assert!(Engine::restore(&edited, good.clone()).is_ok());
    edited.quests[0].reward_xp += 1;
    assert!(Engine::restore(&edited, good.clone()).is_err());
    let mut json: serde_json::Value = serde_json::to_value(&good).unwrap();
    json["state"]["surprise"] = true.into();
    assert!(serde_json::from_value::<SaveSnapshot>(json).is_err());
    assert!(Engine::restore(&world, good).is_ok());
}

#[test]
fn a_choice_that_makes_the_speaker_unavailable_ends_the_conversation() {
    let mut world = demo();
    world.characters[1].requires = Some(Condition::Quest {
        quest: "quiet_the_track".into(),
        status: QuestStatus::Available,
    });
    let accepted = &mut world.dialogues[0].nodes[2];
    accepted.choices.push(DialogueChoice {
        text: "Farewell.".into(),
        next: None,
        requires: None,
        effects: Vec::new(),
        ..Default::default()
    });
    let mut engine = Engine::new(&world).unwrap();
    accept(&mut engine);
    assert_eq!(engine.state().dialogue, None);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}
