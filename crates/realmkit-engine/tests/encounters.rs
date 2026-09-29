//! Damage, the initiative timeline, skills, resources and parity with the simulator.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

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
