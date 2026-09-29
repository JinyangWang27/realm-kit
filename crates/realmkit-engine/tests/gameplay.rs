use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn demo() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/demo-world"
    ))
    .unwrap()
}

fn archive() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/quiet-archive"
    ))
    .unwrap()
}

fn combat<'e>(engine: &'e Engine<'_>) -> &'e CombatState {
    engine.state().combat.as_ref().unwrap()
}

fn vitals(engine: &Engine<'_>) -> Vitals {
    engine.player_vitals().unwrap()
}

fn foe(engine: &Engine<'_>) -> Participant {
    engine.encounter().unwrap().participants[1].clone()
}

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
    assert!(events.contains(&Event::LevelUp { level: 2 }));
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
    assert!(events.contains(&Event::EncounterEnded));
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

fn offered(engine: &Engine<'_>) -> Vec<(Command, bool)> {
    engine
        .actions()
        .into_iter()
        .map(|a| (a.command, a.available))
        .collect()
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
    let mut edited = demo();
    edited.items[0].description.push('.');
    assert!(Engine::restore(&edited, good.clone()).is_err());
    let mut json: serde_json::Value = serde_json::to_value(&good).unwrap();
    json["state"]["surprise"] = true.into();
    assert!(serde_json::from_value::<SaveSnapshot>(json).is_err());
    assert!(Engine::restore(&world, good).is_ok());
}

#[test]
fn a_choice_that_makes_the_speaker_unavailable_ends_the_conversation() {
    let mut world = demo();
    world.characters[1].requires = vec![Condition::Quest {
        quest: "quiet_the_track".into(),
        status: QuestStatus::Available,
    }];
    let accepted = &mut world.dialogues[0].nodes[2];
    accepted.choices.push(DialogueChoice {
        text: "Farewell.".into(),
        next: None,
        requires: Vec::new(),
        effect: None,
    });
    let mut engine = Engine::new(&world).unwrap();
    accept(&mut engine);
    assert_eq!(engine.state().dialogue, None);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

/// Learns where the map is and returns to the reading room.
fn find_map(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(South)).unwrap();
    events
}

fn accept_map(engine: &mut Engine<'_>) {
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
}

#[test]
fn a_world_without_combat_plays_its_main_quest_by_talking() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(engine.state().combat, None);
    assert!(!engine.is_dead());
    accept_map(&mut engine);
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Active);
    let events = find_map(&mut engine);
    assert!(events.contains(&Event::QuestProgressed {
        quest: "lost_map".into()
    }));
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Ready);
    engine.execute(Talk("archivist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::QuestCompleted {
        quest: "lost_map".into()
    }));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::ExperienceGranted { .. })));
    engine.execute(Move(East)).unwrap();
    assert_eq!(engine.state().player.location, "vault");
    assert_eq!(engine.state().player.inventory["vault_key"], 1);
    assert_eq!(engine.state().combat, None);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn a_world_without_combat_offers_no_attack_and_refuses_one() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    assert_eq!(
        offered(&engine),
        vec![
            (Talk("copyist".into()), true),
            (Move(South), true),
            (Inventory, true),
            (Status, true),
            (Quests, true),
        ]
    );
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Attack("copyist".into())),
        Err(EngineError::NotFighting)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn a_flag_set_before_accepting_readies_the_quest_on_acceptance() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    find_map(&mut engine);
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Available);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::QuestProgressed {
        quest: "lost_map".into()
    }));
    assert_eq!(engine.state().quests["lost_map"], QuestStatus::Ready);
}

#[test]
fn a_world_without_combat_rejects_inconsistent_saves() {
    let world = archive();
    let mut engine = Engine::new(&world).unwrap();
    accept_map(&mut engine);
    let good = engine.snapshot();
    let json = serde_json::to_string(&good).unwrap();
    let resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(resumed.state(), engine.state());
    let mut fighting = good.clone();
    fighting.state.combat = Engine::new(&demo()).unwrap().state().combat.clone();
    let mut ready = good.clone();
    ready
        .state
        .quests
        .insert("lost_map".into(), QuestStatus::Ready);
    let mut found = good.clone();
    found.state.flags.insert("map_found".into());
    for (i, snapshot) in [fighting, ready, found].into_iter().enumerate() {
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn a_defeated_character_can_no_longer_be_talked_to() {
    let mut world = demo();
    world.characters[2].dialogue = Some("mara".into());
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    assert_eq!(offered(&engine)[0], (Talk("wolf".into()), true));
    engine.execute(Engage("wolf".into())).unwrap();
    for _ in 0..3 {
        engine.execute(Attack("wolf".into())).unwrap();
    }
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Talk(_))));
    assert!(matches!(
        engine.execute(Talk("wolf".into())),
        Err(EngineError::NotHere(_))
    ));
}

fn duel() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel")).unwrap()
}

fn cast(skill: &str) -> Command {
    UseSkill {
        skill: skill.into(),
        target: "witch".into(),
    }
}

#[test]
fn damage_matches_the_roadmap_examples_and_the_simulator() {
    let stats = |patk, pdef, satk, sdef| Stats {
        hp: 1,
        mp: 0,
        patk,
        pdef,
        satk,
        sdef,
        speed: 100,
    };
    let attacker = stats(40, 0, 20, 0);
    // ROADMAP "Damage: two channels": physical and special hits by defender.
    for (defender, physical, special) in [
        (stats(0, 0, 0, 0), 45, 30),
        (stats(0, 45, 0, 0), 22, 21),
        (stats(0, 0, 0, 80), 31, 8),
        (stats(0, 20, 0, 0), 31, 25),
    ] {
        assert_eq!(
            damage(&attacker, &defender, Channel::Physical, 100, 25).unwrap(),
            physical
        );
        assert_eq!(
            damage(&attacker, &defender, Channel::Special, 100, 25).unwrap(),
            special
        );
    }
    // scripts/combat_sim `Rules.damage` on its sample characters.
    for (a, d, channel, power, expected) in [
        (
            stats(24, 15, 5, 10),
            stats(16, 10, 0, 10),
            Channel::Physical,
            100,
            16,
        ),
        (
            stats(19, 24, 47, 35),
            stats(0, 24, 38, 24),
            Channel::Special,
            170,
            55,
        ),
        (
            stats(49, 61, 122, 92),
            stats(118, 74, 0, 74),
            Channel::Special,
            250,
            198,
        ),
        (
            stats(0, 15, 23, 15),
            stats(35, 22, 7, 15),
            Channel::Special,
            100,
            12,
        ),
    ] {
        assert_eq!(damage(&a, &d, channel, power, 25).unwrap(), expected);
    }
    // A hit always lands for at least 1, even against overwhelming defence.
    let wall = stats(0, 9_999, 0, 9_999);
    assert_eq!(
        damage(&stats(1, 0, 0, 0), &wall, Channel::Physical, 1, 0).unwrap(),
        1
    );
    let top = stats(9_999, 0, 9_999, 0);
    assert!(damage(&top, &stats(0, 0, 0, 0), Channel::Special, 1_000, 100).is_ok());
}

fn combatant<'w>(world: &'w mut WorldSpec, id: &str) -> &'w mut CombatProfile {
    world
        .characters
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
}

fn witch_speed(speed: u32) -> WorldSpec {
    let mut world = duel();
    combatant(&mut world, "witch").stats.speed = speed;
    world
}

fn engaged(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("witch".into())).unwrap();
    engine
}

#[test]
fn speed_sets_who_acts_first_and_how_often_up_to_the_cap() {
    // Speed 120 against 100: the witch's opening delay is shorter, so she acts first.
    let world = duel();
    let engine = engaged(&world);
    assert!(vitals(&engine).hp < 34);
    assert_eq!(
        engine.turn_order(5),
        ["apprentice", "witch", "apprentice", "witch", "apprentice"]
    );
    // Twice the speed, twice the turns; above the cap, nothing more.
    let fast = witch_speed(200);
    let doubled = engaged(&fast).turn_order(6);
    assert_eq!(
        doubled,
        [
            "apprentice",
            "witch",
            "witch",
            "apprentice",
            "witch",
            "witch"
        ]
    );
    let capped = witch_speed(400);
    assert_eq!(engaged(&capped).turn_order(6), doubled);
}

#[test]
fn ties_go_to_the_players_side() {
    let world = witch_speed(100);
    let engine = engaged(&world);
    // Equal opening delays: the player's side acts first, so nobody has hit yet.
    assert_eq!(vitals(&engine).hp, 34);
    assert_eq!(engine.turn_order(2), ["apprentice", "witch"]);
}

#[test]
fn skills_spend_their_resource_and_opponents_choose_their_strongest_affordable_skill() {
    let world = duel();
    let mut engine = engaged(&world);
    let events = engine.execute(cast("bolt")).unwrap();
    assert!(events.contains(&Event::ResourceSpent {
        character: "apprentice".into(),
        resource: Resource::Mp,
        amount: 12
    }));
    // The witch hexed on her opening turn and again after the bolt: 10 MP, 5 each.
    let hexes = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::DamageReceived { skill: Some(s), .. } if s == "hex"))
            .count()
    };
    assert_eq!(hexes(&events), 1);
    assert_eq!(foe(&engine).mp, 0);
    // Out of MP, she falls back to her basic attack.
    let events = engine.execute(cast("spark")).unwrap();
    assert_eq!(hexes(&events), 0);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::DamageReceived { skill: None, .. })));
}

#[test]
fn mp_regenerates_over_encounter_time_with_a_carried_remainder() {
    let world = witch_speed(100);
    let mut engine = engaged(&world);
    let mp = |engine: &Engine<'_>| {
        let player = &engine.encounter().unwrap().participants[0];
        (player.mp, player.mp_remainder)
    };
    // Time spent at full MP banks nothing; the pause at the player's turn
    // already includes the regeneration up to it.
    assert_eq!(mp(&engine), (24, 0));
    // Each baseline turn regains 3% of 24 MP: 0.72 MP, carried as a remainder.
    engine.execute(cast("bolt")).unwrap();
    assert_eq!(mp(&engine), (12, 72_000));
    engine.execute(cast("spark")).unwrap();
    assert_eq!(mp(&engine), (13, 44_000));
}

#[test]
fn rage_from_an_action_arrives_after_it_resolves() {
    let mut world = demo();
    let combat = world.world.combat.as_mut().unwrap();
    (
        combat.resources.rage_per_action,
        combat.resources.rage_per_max_hp,
    ) = (5, 0);
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("wolf".into())).unwrap();
    let swing = UseSkill {
        skill: "heavy_swing".into(),
        target: "wolf".into(),
    };
    // Its own action would earn the 5 rage it costs, but not in time to pay for it.
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(swing.clone()),
        Err(EngineError::NotEnoughRage(_))
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(Attack("wolf".into())).unwrap();
    let events = engine.execute(swing).unwrap();
    assert!(events.contains(&Event::ResourceSpent {
        character: "you".into(),
        resource: Resource::Rage,
        amount: 5
    }));
}

#[test]
fn rage_from_damage_taken_is_proportional_and_cumulative() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("wolf".into())).unwrap();
    // The wolf's 4 damage is a tenth of 40 HP: 20 × 4 / 40 = 2 rage, nothing carried.
    let player = &engine.encounter().unwrap().participants[0];
    assert_eq!((player.rage, player.rage_remainder), (2, 0));
    engine.execute(Attack("wolf".into())).unwrap();
    // +1 for acting, +2 for the next bite; the wolf keeps 20 × 7 = 140 of 20 HP: 7 rage.
    let encounter = engine.encounter().unwrap();
    assert_eq!(encounter.participants[0].rage, 5);
    assert_eq!(
        (
            encounter.participants[1].rage,
            encounter.participants[1].rage_remainder
        ),
        (2 + 7, 0)
    );
}

#[test]
fn an_encounter_allows_only_combat_actions_and_panels() {
    let world = duel();
    let mut engine = engaged(&world);
    let before = engine.state().clone();
    for command in [Move(South), Rest, Engage("witch".into())] {
        assert!(
            matches!(
                engine.execute(command.clone()),
                Err(EngineError::InEncounter)
            ),
            "{command:?}"
        );
        assert_eq!(engine.state(), &before);
    }
    for command in [cast("hex"), cast("missing")] {
        assert!(matches!(
            engine.execute(command),
            Err(EngineError::UnknownSkill(_))
        ));
    }
    assert!(matches!(
        engine.execute(cast("fireball")),
        Err(EngineError::SkillLocked(_))
    ));
    assert!(matches!(
        engine.execute(Attack("apprentice".into())),
        Err(EngineError::NotHere(_))
    ));
    for command in [Look, Status, Inventory, Quests] {
        engine.execute(command).unwrap();
    }
    assert_eq!(engine.state(), &before);
    let mut exploring = Engine::new(&world).unwrap();
    assert!(matches!(
        exploring.execute(Attack("witch".into())),
        Err(EngineError::NotFighting)
    ));
}

#[test]
fn a_mid_encounter_save_resumes_exactly() {
    let world = duel();
    let mut uninterrupted = engaged(&world);
    uninterrupted.execute(cast("bolt")).unwrap();
    let json = serde_json::to_string(&uninterrupted.snapshot()).unwrap();
    let mut resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    for command in [cast("bolt"), cast("spark"), cast("spark")] {
        assert_eq!(
            resumed.execute(command.clone()).unwrap(),
            uninterrupted.execute(command).unwrap()
        );
    }
    assert_eq!(resumed.state(), uninterrupted.state());
}

#[test]
fn inconsistent_encounter_saves_are_rejected() {
    let world = duel();
    let mut engine = engaged(&world);
    engine.execute(cast("bolt")).unwrap();
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    let broken: Vec<fn(&mut Encounter)> = vec![
        |e| e.participants.swap(0, 1),
        |e| e.participants.truncate(1),
        |e| e.participants.push(e.participants[1].clone()),
        |e| e.participants[1].hp = 31,
        |e| e.participants[1].mp = 11,
        |e| e.participants[0].mp_remainder = 100_000,
        |e| e.participants[1].rage_remainder = 30,
        |e| e.participants[1].next_time = e.now - 1,
        |e| e.participants[1].control = Control::Player,
        |e| e.participants[1].side = 0,
        |e| e.participants[1].character = "apprentice".into(),
        |e| e.participants[1].hp = 0,
        // The opponent would be overdue: the player's turn must be now.
        |e| e.participants[0].next_time += 1,
        |e| e.participants[1].side = 2,
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        let Stance::Fighting(encounter) = &mut snapshot.state.combat.as_mut().unwrap().stance
        else {
            unreachable!()
        };
        corrupt(encounter);
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
    // Opponents must be present and undefeated, and nobody talks mid-fight.
    let mut elsewhere = good.clone();
    elsewhere.state.player.location = "yard".into();
    assert!(Engine::restore(&world, elsewhere).is_err());
    let mut defeated = good;
    defeated
        .state
        .combat
        .as_mut()
        .unwrap()
        .defeated
        .insert("witch".into());
    assert!(Engine::restore(&world, defeated).is_err());
}

/// One fight from the simulator's report: both sides' stats at a level, their
/// usable skills, and the result `scripts/combat_sim` gives for it.
struct SimFight {
    player: Stats,
    player_skills: Vec<Skill>,
    player_basic: Channel,
    foe: Stats,
    foe_skills: Vec<Skill>,
    foe_basic: Channel,
    actions: usize,
    hp: u32,
    mp: u32,
}

fn stats(hp: u32, mp: u32, patk: u32, pdef: u32, satk: u32, sdef: u32, speed: u32) -> Stats {
    Stats {
        hp,
        mp,
        patk,
        pdef,
        satk,
        sdef,
        speed,
    }
}

fn skill(id: &str, power: u32, channel: Channel, cost: u32, resource: Resource) -> Skill {
    Skill {
        id: id.into(),
        name: id.into(),
        power,
        channel,
        cost,
        resource,
        time: 100,
        level: 1,
        cross_share: None,
        text: TextTemplate("{attacker} {target} {damage}".into()),
    }
}

/// Plays one simulator fight in the engine, choosing the player's action as the
/// simulator does: the strongest affordable skill, else the basic attack.
fn replay(fight: SimFight) {
    let mut world = duel();
    let combat = world.world.combat.as_mut().unwrap();
    combat.levels = vec![Level {
        xp: 0,
        stats: fight.player,
    }];
    combat.resources = Resources {
        mp_regen_percent: 3,
        rage_per_action: 1,
        rage_per_max_hp: 20,
    };
    combat.player_basic_channel = fight.player_basic;
    combat.player_skills = fight.player_skills.iter().map(|s| s.id.clone()).collect();
    combat.skills = fight.player_skills.clone();
    combat.skills.extend(fight.foe_skills.iter().cloned());
    let witch = combatant(&mut world, "witch");
    witch.stats = fight.foe;
    witch.skills = fight.foe_skills.iter().map(|s| s.id.clone()).collect();
    witch.basic_channel = fight.foe_basic;
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut engine = engaged(&world);
    let mut actions = 0;
    while let Some(encounter) = engine.encounter() {
        let player = &encounter.participants[0];
        let pool = |s: &Skill| match s.resource {
            Resource::Mp => player.mp,
            Resource::Rage => player.rage,
        };
        let choice = fight
            .player_skills
            .iter()
            .filter(|s| pool(s) >= s.cost)
            .rev()
            .max_by_key(|s| (s.power, std::cmp::Reverse(s.cost), s.level));
        let command = match choice {
            Some(s) => UseSkill {
                skill: s.id.clone(),
                target: "witch".into(),
            },
            None => Attack("witch".into()),
        };
        engine.execute(command).unwrap();
        actions += 1;
    }
    let left = vitals(&engine);
    assert_eq!(
        (actions, left.hp, left.mp),
        (fight.actions, fight.hp, fight.mp)
    );
}

// Numbers from `scripts/combat_sim` (Rules() defaults, content.DEFAULT):
// `Encounter(rules, player, [foe]).run()` for each pairing below.
#[test]
fn encounters_match_the_simulator() {
    use Channel::*;
    use Resource::*;
    let rage = |id, power, channel| skill(id, power, channel, 5, Rage);
    // Warrior level 10 against the beast at level 10: rage on both sides.
    replay(SimFight {
        player: stats(472, 0, 57, 35, 12, 24, 100),
        player_skills: vec![
            rage("rage_strike", 150, Physical),
            rage("cleave", 185, Physical),
        ],
        player_basic: Physical,
        foe: stats(189, 0, 38, 24, 0, 24, 110),
        foe_skills: vec![rage("rend", 130, Physical), rage("maul", 155, Physical)],
        foe_basic: Physical,
        actions: 4,
        hp: 370,
        mp: 0,
    });
    let mage = |fireball| {
        let mut skills = vec![
            skill("spark", 80, Special, 0, Mp),
            skill("bolt", 170, Special, 12, Mp),
        ];
        if fireball {
            skills.push(skill("fireball", 210, Special, 12, Mp));
        }
        skills
    };
    // Mage level 10 against the spirit at level 10: MP regeneration and special channels.
    replay(SimFight {
        player: stats(401, 75, 19, 24, 47, 35, 100),
        player_skills: mage(true),
        player_basic: Physical,
        foe: stats(189, 0, 0, 24, 38, 24, 110),
        foe_skills: vec![rage("hex", 130, Special), rage("curse", 155, Special)],
        foe_basic: Special,
        actions: 3,
        hp: 327,
        mp: 43,
    });
    // Mage level 5 against the beast at level 5.
    replay(SimFight {
        player: stats(249, 75, 12, 15, 29, 22, 100),
        player_skills: mage(false),
        player_basic: Physical,
        foe: stats(117, 0, 23, 15, 0, 15, 110),
        foe_skills: vec![rage("rend", 130, Physical)],
        foe_basic: Physical,
        actions: 4,
        hp: 192,
        mp: 33,
    });
}

#[test]
fn a_skill_paid_for_by_regeneration_up_to_the_players_turn_is_usable() {
    // 50% of 24 MP per baseline turn: two bolts empty the pool, and the 12 MP
    // regained on the way to the next turn pay for a third.
    let mut world = witch_speed(100);
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .resources
        .mp_regen_percent = 50;
    let mut engine = engaged(&world);
    engine.execute(cast("bolt")).unwrap();
    engine.execute(cast("bolt")).unwrap();
    assert!(vitals(&engine).mp >= 12);
    assert!(offered(&engine).contains(&(cast("bolt"), true)));
    engine.execute(cast("bolt")).unwrap();
}

#[test]
fn the_largest_action_cost_saves_and_restores_without_overflow() {
    let mut world = duel();
    world.world.combat.as_mut().unwrap().timeline.action_cost = ACTION_COST_BOUND;
    let engine = engaged(&world);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}
