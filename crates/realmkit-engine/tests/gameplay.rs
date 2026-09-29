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
        crit: None,
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
        points: 0,
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
fn rage_saturates_instead_of_failing_the_encounter() {
    let mut world = demo();
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .resources
        .rage_per_action = u32::MAX;
    combatant(&mut world, "wolf").stats.speed = 200;
    // The wolf acts twice before the player's first turn, overflowing rage.
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("wolf".into())).unwrap();
    assert_eq!(foe(&engine).rage, u32::MAX);
}

#[test]
fn the_largest_action_cost_saves_and_restores_without_overflow() {
    let mut world = duel();
    world.world.combat.as_mut().unwrap().timeline.action_cost = ACTION_COST_BOUND;
    let engine = engaged(&world);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

fn arena() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap()
}

fn at(world: &WorldSpec, direction: Direction) -> Engine<'_> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(Move(direction)).unwrap();
    engine
}

/// Attacks the first fighting opponent until the encounter ends.
fn fight_out(engine: &mut Engine<'_>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(encounter) = engine.encounter() {
        if engine.is_dead() {
            break;
        }
        let foe = encounter.participants[1..]
            .iter()
            .find(|p| p.fighting())
            .unwrap()
            .character
            .clone();
        events.extend(engine.execute(Attack(foe)).unwrap());
    }
    events
}

#[test]
fn a_pack_fights_together_and_every_member_must_fall() {
    let world = arena();
    let mut engine = at(&world, North);
    let events = engine.execute(Engage("black_wolf".into())).unwrap();
    // Engaging either wolf brings both, in the den's authored order.
    assert!(events.contains(&Event::EncounterStarted {
        opponents: vec!["grey_wolf".into(), "black_wolf".into()]
    }));
    // The faster wolves opened; the player is next, then both wolves in order.
    assert_eq!(engine.turn_order(3), ["fighter", "grey_wolf", "black_wolf"]);
    let bites = events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .count();
    assert_eq!(bites, 2);
    // Felling the first wolf leaves the fight running against the second.
    let mut first = Vec::new();
    while engine.encounter().unwrap().participants[1].hp > 0 {
        first.extend(engine.execute(Attack("grey_wolf".into())).unwrap());
    }
    assert!(engine.encounter().is_some());
    assert!(!engine.turn_order(4).contains(&"grey_wolf".to_string()));
    assert!(matches!(
        engine.execute(Attack("grey_wolf".into())),
        Err(EngineError::NotHere(_))
    ));
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    let defeated = &combat(&engine).defeated;
    assert!(defeated.contains("grey_wolf") && defeated.contains("black_wolf"));
    assert_eq!(combat(&engine).xp, 16);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn xp_falls_off_with_level_difference_as_in_the_simulator() {
    // scripts/combat_sim XpRules.for_kill(100, player, monster) for monster − player.
    for (diff, expected) in [
        (-6, 0),
        (-5, 0),
        (-4, 60),
        (-1, 90),
        (0, 100),
        (2, 120),
        (4, 140),
        (9, 140),
    ] {
        let (player, monster) = if diff < 0 {
            (10, (10 + diff) as usize)
        } else {
            (10, 10 + diff as usize)
        };
        assert_eq!(
            xp_for_defeat(100, player, monster),
            expected,
            "difference {diff}"
        );
    }
    assert_eq!(xp_for_defeat(7, 1, 2), 7);
    assert_eq!(xp_for_defeat(u64::MAX, 1, 9), u64::MAX);
}

#[test]
fn a_repeatable_group_can_be_fought_again_and_rewards_every_victory() {
    let world = arena();
    let mut engine = at(&world, East);
    for round in 1..=3 {
        engine.execute(Engage("rat".into())).unwrap();
        let events = fight_out(&mut engine);
        assert!(events.contains(&Event::ItemReceived {
            item: "rat_tail".into(),
            quantity: 1
        }));
        assert_eq!(engine.state().player.inventory["rat_tail"], round);
    }
    // Never recorded, and 4 XP for each same-level rat.
    assert!(combat(&engine).defeated.is_empty());
    assert_eq!((combat(&engine).xp, combat(&engine).level), (12, 2));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn nothing_is_learned_from_opponents_five_levels_below() {
    // One 5-XP rat lifts the player from level 1 to 6; the next rat is five below.
    let mut world = arena();
    combatant(&mut world, "rat").xp = 5;
    for (level, entry) in world
        .world
        .combat
        .as_mut()
        .unwrap()
        .levels
        .iter_mut()
        .enumerate()
    {
        entry.xp = level as u64;
    }
    let mut engine = at(&world, East);
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!(combat(&engine).level, 6);
    engine.execute(Engage("rat".into())).unwrap();
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::ExperienceGranted { amount: 0 }));
    assert_eq!(combat(&engine).xp, 5);
}

#[test]
fn fleeing_resolves_at_the_players_next_turn_and_grants_nothing() {
    let world = arena();
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    let hp = vitals(&engine).hp;
    let events = engine.execute(Flee).unwrap();
    assert_eq!(events[0], Event::FleeStarted);
    // Both wolves act during the wind-up, then the escape happens.
    let bites = events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .count();
    assert_eq!(bites, 2);
    assert_eq!(
        events.last(),
        Some(&Event::EncounterEnded {
            outcome: Outcome::Fled
        })
    );
    assert!(vitals(&engine).hp < hp);
    assert!(engine.encounter().is_none() && combat(&engine).defeated.is_empty());
    assert_eq!(combat(&engine).xp, 0);
    // The wolves are whole again next time.
    engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(engine.encounter().unwrap().participants[1..]
        .iter()
        .all(|p| p.hp == 24));
}

/// The arena with the pit open, for tests of the ogre.
fn open_pit() -> WorldSpec {
    let mut world = arena();
    world.locations[0]
        .exits
        .get_mut(&Down)
        .unwrap()
        .requires
        .clear();
    world
}

#[test]
fn a_no_flee_group_cannot_be_fled_and_opponent_skills_follow_their_level() {
    let world = open_pit();
    let mut engine = at(&world, Down);
    engine.execute(Engage("ogre".into())).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| *c == Flee));
    let before = engine.state().clone();
    assert!(matches!(engine.execute(Flee), Err(EngineError::NoFlee)));
    assert_eq!(engine.state(), &before);
    // The level-4 ogre has smash (level 3) but not crush (level 5).
    let mut used = Vec::new();
    while engine.encounter().is_some() && !engine.is_dead() {
        for event in engine.execute(Attack("ogre".into())).unwrap() {
            if let Event::DamageReceived { skill: Some(s), .. } = event {
                used.push(s);
            }
        }
    }
    assert!(used.contains(&"smash".to_string()));
    assert!(!used.contains(&"crush".to_string()));
}

#[test]
fn a_yielding_opponent_ends_the_fight_alive_and_sets_the_victory_flags() {
    let world = arena();
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    let events = fight_out(&mut engine);
    assert!(events.contains(&Event::Yielded {
        character: "holt".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::EnemyDefeated { .. })));
    assert!(engine.state().flags.contains("spar_won"));
    // Not recorded, no reward: the sergeant can spar again.
    assert!(combat(&engine).defeated.is_empty());
    assert_eq!(combat(&engine).xp, 0);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    engine.execute(Engage("holt".into())).unwrap();
}

#[test]
fn a_player_who_yields_keeps_playing_with_the_defeat_flags_set() {
    let mut world = arena();
    combatant(&mut world, "holt").stats.patk = STAT_BOUND;
    let mut engine = at(&world, West);
    // A hit never takes a yielder below 1 HP, so an overwhelming blow yields.
    // Equal speeds: the player's side acts first, then Holt strikes.
    engine.execute(Engage("holt".into())).unwrap();
    let events = engine.execute(Attack("holt".into())).unwrap();
    assert!(events.contains(&Event::Yielded {
        character: "fighter".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Yielded
    }));
    assert!(!events.contains(&Event::PlayerDied));
    assert_eq!(vitals(&engine).hp, 1);
    assert!(engine.state().flags.contains("spar_lost"));
    engine.execute(Move(East)).unwrap();
    engine.execute(Rest).unwrap();
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn group_encounters_save_exactly_and_reject_a_wrong_cast() {
    let world = arena();
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    engine.execute(Attack("grey_wolf".into())).unwrap();
    let good = engine.snapshot();
    let mut resumed = Engine::restore(&world, good.clone()).unwrap();
    assert_eq!(fight_out(&mut resumed), fight_out(&mut engine));
    let broken: Vec<fn(&mut Encounter)> = vec![
        |e| e.participants.truncate(2),
        |e| e.participants.swap(1, 2),
        |e| e.participants[1].yielded = true,
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
}

#[test]
fn a_pack_is_rewarded_at_the_level_the_player_fought_at() {
    // The first wolf's 8 XP reaches level 2; the second still pays in full,
    // whatever order the den lists them in.
    let mut world = arena();
    world.world.combat.as_mut().unwrap().levels[1].xp = 8;
    let mut engine = at(&world, North);
    engine.execute(Engage("grey_wolf".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!((combat(&engine).xp, combat(&engine).level), (16, 2));
}

#[test]
fn a_save_after_one_sparring_partner_yields_still_loads() {
    // A second partner keeps the fight going after the first yields, while the
    // yielder's turn time falls behind the schedule.
    let mut world = arena();
    let mut second = world
        .characters
        .iter()
        .find(|c| c.id == "holt")
        .unwrap()
        .clone();
    second.id = "corporal".into();
    second.name = "Corporal Wren".into();
    // Weak enough that the player outlasts Holt.
    second.combat.as_mut().unwrap().stats.patk = 1;
    world.characters.push(second);
    world.locations[3].characters.push("corporal".into());
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    while !engine.encounter().unwrap().participants[1].yielded {
        engine.execute(Attack("holt".into())).unwrap();
    }
    engine.execute(Attack("corporal".into())).unwrap();
    let encounter = engine.encounter().unwrap();
    assert!(encounter.participants[1].next_time < encounter.now);
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    // Yielding is exactly "at or below the threshold", and nobody dies.
    let broken: Vec<fn(&mut Encounter)> = vec![
        |e| e.participants[2].yielded = true,
        |e| e.participants[1].yielded = false,
        |e| (e.participants[2].hp, e.participants[2].yielded) = (0, false),
        |e| e.participants[1].hp = 0,
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        let Stance::Fighting(encounter) = &mut snapshot.state.combat.as_mut().unwrap().stance
        else {
            unreachable!()
        };
        corrupt(encounter);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn untouched_participants_at_a_full_yield_share_save_and_load() {
    // At a 100% share everyone starts within the threshold, but nobody has
    // yielded before the first hit; the player acts first on the tie.
    let mut world = arena();
    world.world.combat.as_mut().unwrap().groups[2].yield_share = Some(100);
    let mut engine = at(&world, West);
    engine.execute(Engage("holt".into())).unwrap();
    assert!(!engine.encounter().unwrap().participants[1].yielded);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    // The first touch ends the bout.
    let events = engine.execute(Attack("holt".into())).unwrap();
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
}

#[test]
fn splitmix64_produces_its_published_sequence() {
    let mut state = 0;
    let outputs: Vec<u64> = (0..3).map(|_| splitmix64(&mut state)).collect();
    assert_eq!(
        outputs,
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f
        ]
    );
}

#[test]
fn a_world_without_random_content_keeps_no_generator() {
    for world in [demo(), duel(), archive()] {
        assert!(!world.stochastic());
        assert_eq!(Engine::new_with_seed(&world, 7).unwrap().state().rng, None);
    }
    let world = arena();
    assert!(world.stochastic());
    assert_eq!(
        Engine::new_with_seed(&world, 7).unwrap().state().rng,
        Some(RngState::new(7))
    );
}

/// Heavy blows at the pit ogre until the fight ends, starting from `seed`.
fn swings(world: &WorldSpec, seed: u64) -> (Vec<Event>, GameState) {
    let mut engine = Engine::new_with_seed(world, seed).unwrap();
    engine.execute(Move(Down)).unwrap();
    engine.execute(Engage("ogre".into())).unwrap();
    let mut events = Vec::new();
    while engine.encounter().is_some() && !engine.is_dead() {
        let player = &engine.encounter().unwrap().participants[0];
        let command = if player.rage >= 5 {
            UseSkill {
                skill: "heavy_blow".into(),
                target: "ogre".into(),
            }
        } else {
            Attack("ogre".into())
        };
        events.extend(engine.execute(command).unwrap());
    }
    (events, engine.state().clone())
}

fn criticals(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::DamageDealt { critical: true, .. }
                    | Event::DamageReceived { critical: true, .. }
            )
        })
        .count()
}

#[test]
fn the_same_seed_replays_exactly_and_seeds_differ() {
    let world = open_pit();
    assert_eq!(swings(&world, 42), swings(&world, 42));
    let runs: Vec<_> = (0..20)
        .map(|seed| criticals(&swings(&world, seed).0))
        .collect();
    assert!(runs.iter().any(|&n| n > 0), "{runs:?}");
    assert!(runs.windows(2).any(|w| w[0] != w[1]), "{runs:?}");
}

#[test]
fn a_mid_stream_save_continues_the_sequence_and_refusals_draw_nothing() {
    let world = open_pit();
    let mut engine = Engine::new_with_seed(&world, 9).unwrap();
    engine.execute(Move(Down)).unwrap();
    engine.execute(Engage("ogre".into())).unwrap();
    engine.execute(Attack("ogre".into())).unwrap();
    // A refused skill draws nothing and changes nothing.
    let before = engine.state().clone();
    assert!(engine.execute(cast("missing")).is_err());
    assert_eq!(engine.state(), &before);
    let mut resumed = Engine::restore(&world, engine.snapshot()).unwrap();
    for _ in 0..6 {
        if engine.encounter().is_none() || engine.is_dead() {
            break;
        }
        let command = Attack("ogre".into());
        assert_eq!(
            resumed.execute(command.clone()).unwrap(),
            engine.execute(command).unwrap()
        );
    }
    assert_eq!(resumed.state(), engine.state());
    // The stream belongs to the world: a save without it, or with another
    // algorithm, is refused.
    let mut missing = engine.snapshot();
    missing.state.rng = None;
    assert!(Engine::restore(&world, missing).is_err());
    let mut other = engine.snapshot();
    other.state.rng.as_mut().unwrap().version = RNG_VERSION + 1;
    assert!(Engine::restore(&world, other).is_err());
    let mut stray = Engine::new(&duel()).unwrap().snapshot();
    stray.state.rng = Some(RngState::new(1));
    assert!(Engine::restore(&duel(), stray).is_err());
}

#[test]
fn a_crit_multiplies_before_the_single_rounding() {
    let stats = |patk, pdef| Stats {
        hp: 1,
        mp: 0,
        patk,
        pdef,
        satk: 0,
        sdef: 0,
        speed: 100,
    };
    // 10 000 / 1 300 = 7.69: a normal hit deals 7; at 150% it is 11.5 → 11,
    // where rounding first would give 7 × 1.5 → 10.
    let (a, d) = (stats(10, 0), stats(0, 3));
    assert_eq!(damage(&a, &d, Channel::Physical, 100, 0).unwrap(), 7);
    assert_eq!(
        damage_scaled(&a, &d, Channel::Physical, 100, 0, 150).unwrap(),
        11
    );
    // A certain crit always fires, and shows on the event.
    let mut world = open_pit();
    world.world.combat.as_mut().unwrap().player_basic_crit = Some(Crit {
        chance_percent: 100,
        multiplier_percent: 200,
    });
    let mut engine = Engine::new_with_seed(&world, 3).unwrap();
    engine.execute(Move(Down)).unwrap();
    engine.execute(Engage("ogre".into())).unwrap();
    let events = engine.execute(Attack("ogre".into())).unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::DamageDealt { critical: true, .. })));
}

fn spend(engine: &mut Engine<'_>, stat: Stat, points: u32) -> Result<Vec<Event>, EngineError> {
    engine.execute(Allocate { stat, points })
}

#[test]
fn allocated_points_act_exactly_like_base_stats() {
    // Three points of attack against a world whose level table already has them.
    let world = arena();
    let mut spent = Engine::new(&world).unwrap();
    assert_eq!(spent.unspent_points(), Some(3));
    let events = spend(&mut spent, Stat::Patk, 3).unwrap();
    assert_eq!(
        events,
        vec![Event::PointsAllocated {
            stat: Stat::Patk,
            points: 3
        }]
    );
    assert_eq!(spent.unspent_points(), Some(0));
    // 12 from the level, 3 from points, 2 from the starting practice sword.
    assert_eq!(spent.player_stats().unwrap().patk, 17);
    let mut based = arena();
    for level in &mut based.world.combat.as_mut().unwrap().levels {
        level.stats.patk += 3;
    }
    let mut base = Engine::new(&based).unwrap();
    let first_hit = |engine: &mut Engine<'_>| {
        engine.execute(Move(West)).unwrap();
        engine.execute(Engage("holt".into())).unwrap();
        engine.execute(Attack("holt".into())).unwrap()
    };
    assert_eq!(first_hit(&mut spent), first_hit(&mut base));
}

#[test]
fn speed_points_change_turn_timing() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().levels[0].points = 10;
    let mut engine = Engine::new(&world).unwrap();
    spend(&mut engine, Stat::Speed, 10).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 120);
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    // 120 against Holt's 100: six player turns for every five of his.
    let order = engine.turn_order(11);
    assert_eq!(order.iter().filter(|id| *id == "fighter").count(), 6);
}

#[test]
fn points_raise_current_vitals_and_refusals_change_nothing() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().levels[0].points = 20;
    let mut engine = Engine::new(&world).unwrap();
    spend(&mut engine, Stat::Hp, 1).unwrap();
    assert_eq!(vitals(&engine).hp, 65);
    let before = engine.state().clone();
    for (stat, points, error) in [
        (Stat::Patk, 20, "not enough unspent stat points"),
        (Stat::Patk, 0, "not enough unspent stat points"),
        (Stat::Satk, 1, "you cannot spend points on that stat"),
        (Stat::Speed, 11, "that stat cannot take more points"),
    ] {
        assert_eq!(
            spend(&mut engine, stat, points).unwrap_err().to_string(),
            error
        );
        assert_eq!(engine.state(), &before);
    }
    // Not mid-fight, and not in a world without stat points.
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    assert!(matches!(
        spend(&mut engine, Stat::Hp, 1),
        Err(EngineError::InEncounter)
    ));
    let demo = demo();
    let mut plain = Engine::new(&demo).unwrap();
    assert!(matches!(
        spend(&mut plain, Stat::Hp, 1),
        Err(EngineError::NoSuchStat)
    ));
}

#[test]
fn a_respec_refunds_everything_only_where_allowed() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    spend(&mut engine, Stat::Hp, 3).unwrap();
    assert_eq!(vitals(&engine).hp, 75);
    engine.execute(Move(West)).unwrap();
    let before = engine.state().clone();
    assert!(matches!(engine.execute(Respec), Err(EngineError::NoRespec)));
    assert_eq!(engine.state(), &before);
    engine.execute(Move(East)).unwrap();
    assert_eq!(engine.execute(Respec).unwrap(), vec![Event::PointsRefunded]);
    assert_eq!(engine.unspent_points(), Some(3));
    assert_eq!(vitals(&engine).hp, 60);
    // A world that never allows it.
    let mut fixed = arena();
    let points = fixed
        .world
        .combat
        .as_mut()
        .unwrap()
        .stat_points
        .as_mut()
        .unwrap();
    points.respec = realmkit_spec::Respec::Never;
    let mut engine = Engine::new(&fixed).unwrap();
    assert!(matches!(engine.execute(Respec), Err(EngineError::NoRespec)));
}

#[test]
fn levelling_up_grants_points_and_restores_to_effective_maxima() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    spend(&mut engine, Stat::Hp, 3).unwrap();
    engine.execute(Move(East)).unwrap();
    for _ in 0..3 {
        engine.execute(Engage("rat".into())).unwrap();
        fight_out(&mut engine);
    }
    assert_eq!(combat(&engine).level, 2);
    assert_eq!(engine.unspent_points(), Some(2));
    assert_eq!(vitals(&engine).hp, 66 + 15);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn saves_reject_allocations_the_rules_could_not_make() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    spend(&mut engine, Stat::Speed, 2).unwrap();
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .allocation
                .insert(Stat::Patk, 2);
        },
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .allocation
                .insert(Stat::Satk, 1);
        },
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .allocation
                .insert(Stat::Speed, 11);
        },
        |s| s.state.combat.as_mut().unwrap().stance = Stance::Exploring(Vitals { hp: 61, mp: 0 }),
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

fn sect() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap()
}

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
    let storm = [Condition::Technique {
        technique: "cloud_palm".into(),
        rank: 2,
    }];
    let breath = [Condition::Technique {
        technique: "azure_breath".into(),
        rank: 1,
    }];
    assert!(engine.conditions_met(&breath));
    assert!(!engine.conditions_met(&storm));
    learn_palm(&mut engine);
    engine.execute(Move(North)).unwrap();
    while learned(&engine, "cloud_palm").unwrap().rank == 1 {
        if engine.encounter().is_none() {
            engine.execute(Engage("dummy".into())).unwrap();
        }
        engine.execute(palm("palm_drifting")).unwrap();
    }
    assert!(engine.conditions_met(&storm));
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

fn pieces(engine: &Engine<'_>) -> Vec<(u64, String, bool)> {
    combat(engine)
        .gear
        .iter()
        .map(|(id, g)| (*id, g.item.clone(), g.equipped))
        .collect()
}

fn item<'w>(world: &'w mut WorldSpec, id: &str) -> &'w mut Equipment {
    world
        .items
        .iter_mut()
        .find(|i| i.id == id)
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
}

/// Clears the den and returns the pieces the wolves dropped.
fn beat_the_pack(engine: &mut Engine<'_>) -> Vec<Event> {
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    let events = fight_out(engine);
    engine.execute(Move(South)).unwrap();
    events
}

#[test]
fn starting_gear_is_worn_and_counts_in_effective_stats() {
    let world = arena();
    let engine = Engine::new(&world).unwrap();
    assert_eq!(
        pieces(&engine),
        [
            (1, "practice_sword".into(), true),
            (2, "buckler".into(), true),
            (3, "leather_vest".into(), true),
        ]
    );
    let stats = engine.player_stats().unwrap();
    assert_eq!((stats.patk, stats.pdef), (12 + 2, 8 + 2 + 2));
}

#[test]
fn loot_arrives_as_individual_pieces_and_two_hands_displace_both() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    let events = beat_the_pack(&mut engine);
    assert!(events.contains(&Event::ItemReceived {
        item: "greatsword".into(),
        quantity: 1
    }));
    assert!(!engine.state().player.inventory.contains_key("greatsword"));
    assert_eq!(
        &pieces(&engine)[3..],
        [
            (4, "charm_of_warding".into(), false),
            (5, "greatsword".into(), false)
        ]
    );
    // The pack's pieces are offered to wear.
    assert!(offered(&engine).contains(&(Equip(5), true)));
    let events = engine.execute(Equip(5)).unwrap();
    assert_eq!(
        events,
        [
            Event::Unequipped { gear: 1 },
            Event::Unequipped { gear: 2 },
            Event::Equipped { gear: 5 }
        ]
    );
    let stats = engine.player_stats().unwrap();
    // Level 2 after the pack (13 attack, 9 defence), greatsword on, buckler off.
    assert_eq!((stats.patk, stats.pdef), (13 + 8, 9 + 2));
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn a_heavy_weapon_slows_the_basic_attack_that_uses_it() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment = vec!["greatsword".into()];
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    let before = engine.encounter().unwrap().participants[0].next_time;
    engine.execute(Attack("holt".into())).unwrap();
    // 130% of a basic action at speed 100: 1,300 ticks instead of 1,000.
    let after = engine.encounter().unwrap().participants[0].next_time;
    assert_eq!(after - before, 1_300);
}

#[test]
fn armour_speed_penalties_add_up_and_leave_at_least_1() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment =
        vec!["iron_mail".into(), "practice_sword".into()];
    let engine = Engine::new(&world).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 90);
    item(&mut world, "practice_sword").speed_penalty = 500;
    let engine = Engine::new(&world).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 1);
}

#[test]
fn worn_modifiers_multiply_and_immunity_wins() {
    let stats = |patk, satk| Stats {
        hp: 1,
        mp: 0,
        patk,
        pdef: 0,
        satk,
        sdef: 0,
        speed: 100,
    };
    let (a, d) = (stats(40, 20), stats(0, 0));
    // 45 normally; halved to 22; doubled to 90; immunity deals nothing, not 1.
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (1, 1)).unwrap(),
        45
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (1, 2)).unwrap(),
        22
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (2, 1)).unwrap(),
        90
    );
    assert_eq!(
        damage_modified(&a, &d, Channel::Physical, 100, 25, 100, (0, 1)).unwrap(),
        0
    );
    // Worn: a physical ward and immunity on another piece means no bite lands.
    let mut world = arena();
    let charm = item(&mut world, "charm_of_warding");
    charm
        .modifiers
        .insert(Channel::Physical, Modifier { num: 1, den: 2 });
    item(&mut world, "buckler")
        .modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .player_equipment
        .push("charm_of_warding".into());
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    let events = engine.execute(Engage("grey_wolf".into())).unwrap();
    assert!(events
        .iter()
        .filter(|e| matches!(e, Event::DamageReceived { .. }))
        .all(|e| matches!(e, Event::DamageReceived { amount: 0, .. })));
    assert_eq!(vitals(&engine).hp, 60);
}

#[test]
fn equipping_is_refused_atomically_where_it_cannot_happen() {
    let mut world = arena();
    item(&mut world, "leather_vest")
        .bonuses
        .insert(Stat::Hp, 10);
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(vitals(&engine).hp, 70);
    let before = engine.state().clone();
    for (command, error) in [
        (Equip(9), "you have no equipment #9"),
        (Equip(1), "#1 is already equipped"),
        (Unequip(9), "you have no equipment #9"),
    ] {
        assert_eq!(engine.execute(command).unwrap_err().to_string(), error);
        assert_eq!(engine.state(), &before);
    }
    // Taking the vest off takes its HP with it.
    engine.execute(Unequip(3)).unwrap();
    assert_eq!(vitals(&engine).hp, 60);
    assert_eq!(
        engine.execute(Unequip(3)).unwrap_err().to_string(),
        "#3 is not equipped"
    );
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    assert!(matches!(
        engine.execute(Equip(3)),
        Err(EngineError::InEncounter)
    ));
}

#[test]
fn saves_reject_equipment_the_rules_could_not_produce() {
    let world = arena();
    let mut engine = Engine::new(&world).unwrap();
    beat_the_pack(&mut engine);
    let good = engine.snapshot();
    assert!(Engine::restore(&world, good.clone()).is_ok());
    let broken: Vec<fn(&mut SaveSnapshot)> = vec![
        // An ID at or past the counter.
        |s| {
            let c = s.state.combat.as_mut().unwrap();
            let piece = c.gear.remove(&5).unwrap();
            c.gear.insert(6, piece);
        },
        // Not equipment at all.
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&4)
                .unwrap()
                .item = "rat_tail".into()
        },
        // Two pieces in the same slot.
        |s| {
            s.state
                .combat
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&5)
                .unwrap()
                .equipped = true
        },
        // A piece nothing granted.
        |s| {
            let c = s.state.combat.as_mut().unwrap();
            c.gear.insert(
                6,
                Gear {
                    item: "iron_mail".into(),
                    equipped: false,
                },
            );
            c.next_gear = 7;
        },
        // A starting piece gone.
        |s| {
            s.state.combat.as_mut().unwrap().gear.remove(&2);
        },
        // Equipment held as a count.
        |s| {
            s.state.combat.as_mut().unwrap().gear.remove(&5);
            s.state.player.inventory.insert("greatsword".into(), 1);
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

#[test]
fn the_projection_uses_a_heavy_weapons_basic_time() {
    let mut world = arena();
    world.world.combat.as_mut().unwrap().player_equipment = vec!["greatsword".into()];
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Engage("holt".into())).unwrap();
    // Both open at 1,000; the greatsword's 130% puts the player's next turn
    // at 2,300, after Holt's at 2,000.
    assert_eq!(engine.turn_order(4), ["fighter", "holt", "holt", "fighter"]);
}

#[test]
fn a_save_wearing_an_unknown_item_is_rejected_not_a_crash() {
    let world = arena();
    let engine = Engine::new(&world).unwrap();
    for item in ["missing", "rat_tail"] {
        let mut snapshot = engine.snapshot();
        let piece = snapshot
            .state
            .combat
            .as_mut()
            .unwrap()
            .gear
            .get_mut(&1)
            .unwrap();
        piece.item = item.into();
        assert!(matches!(
            Engine::restore(&world, snapshot),
            Err(EngineError::InvalidSave(_))
        ));
    }
}

#[test]
fn many_worn_modifiers_combine_exactly() {
    // 28 more slots (32 in all) of alternating 10/9 and 9/10 wards multiply
    // to exactly 1, though each product alone overflows 64 bits.
    let warded = |modify: bool| {
        let mut world = arena();
        let slots: Vec<String> = (0..28).map(|i| format!("s{i}")).collect();
        let combat = world.world.combat.as_mut().unwrap();
        combat.slots.extend(slots.iter().cloned());
        combat.player_equipment = slots.iter().map(|s| format!("ward_{s}")).collect();
        for (i, slot) in slots.iter().enumerate() {
            let (num, den) = if i % 2 == 0 { (10, 9) } else { (9, 10) };
            let mut modifiers = std::collections::BTreeMap::new();
            if modify {
                modifiers.insert(Channel::Physical, Modifier { num, den });
            }
            world.items.push(Item {
                id: format!("ward_{slot}"),
                name: "Ward".into(),
                description: "A ward.".into(),
                equipment: Some(Equipment {
                    slots: vec![slot.clone()],
                    bonuses: Default::default(),
                    speed_penalty: 0,
                    basic_channel: None,
                    basic_time: None,
                    modifiers,
                }),
            });
        }
        world
    };
    let bites = |world: &WorldSpec| {
        let mut engine = Engine::new(world).unwrap();
        engine.execute(Move(North)).unwrap();
        engine.execute(Engage("grey_wolf".into())).unwrap()
    };
    assert_eq!(bites(&warded(true)), bites(&warded(false)));
}
