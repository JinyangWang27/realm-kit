//! MP and Rage in the character core: which a character uses, skills that
//! spend either or both, what world time does to them, and how they survive
//! saving. The duel gains a clock and both resources for these tests.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

const MP_ONLY: ResourceUse = ResourceUse {
    mp: true,
    rage: false,
};
const RAGE_ONLY: ResourceUse = ResourceUse {
    mp: false,
    rage: true,
};
const BOTH: ResourceUse = ResourceUse {
    mp: true,
    rage: true,
};

fn skill(id: &str, power: u32, costs: Costs) -> Skill {
    Skill {
        id: id.into(),
        name: id.into(),
        power,
        channel: Channel::Special,
        cost: 0,
        resource: Resource::Mp,
        costs: Some(costs),
        time: 100,
        level: 1,
        cross_share: None,
        text: TextTemplate("{attacker} {target} {damage}".into()),
        crit: None,
    }
}

/// The duel with a clock, Rage that carries up to a cap of 10 and decays a
/// point every 30 minutes, MP that recovers a quarter every hour, and two
/// more skills: `slash` for 4 Rage and `storm` for 6 MP and 3 Rage.
fn fighters(uses: Option<ResourceUse>, player_skills: &[&str]) -> WorldSpec {
    let mut world = duel();
    world.world.time = Some(WorldTime {
        start: 0,
        clock: TextTemplate("Day {day}, {hour}:{minute}".into()),
        wait: Some(30),
        rest: Some(480),
        calendar: None,
    });
    let combat = world.world.combat.as_mut().unwrap();
    combat.resources = Resources {
        mp_regen_percent: 3,
        rage_per_action: 2,
        rage_per_max_hp: 20,
        rage_cap: Some(10),
        rage_carries: true,
        mp_recovery: Some(Recovery {
            percent: 25,
            every: 60,
        }),
        rage_decay: Some(Decay {
            points: 1,
            every: 30,
        }),
    };
    combat
        .skills
        .push(skill("slash", 150, Costs { mp: 0, rage: 4 }));
    combat
        .skills
        .push(skill("storm", 200, Costs { mp: 6, rage: 3 }));
    combat.player_skills = player_skills.iter().map(|s| (*s).into()).collect();
    combat.player_resources = uses;
    world
}

fn engaged(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Engage("witch".into())).unwrap();
    engine
}

/// The player's vitals set directly, as a save the rules could produce.
fn with_vitals<'w>(world: &'w WorldSpec, engine: &Engine<'w>, vitals: Vitals) -> Engine<'w> {
    let mut snapshot = engine.snapshot();
    snapshot.state.combat.as_mut().unwrap().vitals = vitals;
    Engine::restore(world, snapshot).unwrap()
}

fn rejected(world: &WorldSpec, snapshot: SaveSnapshot) -> bool {
    matches!(
        Engine::restore(world, snapshot),
        Err(EngineError::InvalidSave(_))
    )
}

#[test]
fn an_mp_only_character_never_builds_rage() {
    let world = fighters(Some(MP_ONLY), &["spark", "bolt"]);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut engine = engaged(&world);
    let pools = engine.player_pools().unwrap();
    assert_eq!((pools.mp, pools.rage), (Some(24), None));
    // The witch, without a choice of her own, uses both: MP of her own, and
    // Rage because the world builds it.
    let witch = engine.pools_of("witch").unwrap();
    assert_eq!((witch.mp, witch.rage), (Some(10), Some(10)));
    engine.execute(cast("bolt")).unwrap();
    // Hit twice and having acted once, the player has no Rage; the witch,
    // hit once and having acted twice, does.
    assert_eq!(vitals(&engine).rage, 0);
    assert_eq!(vitals(&engine).rage_remainder, 0);
    assert!(foe_vitals(&engine).rage > 0);
    assert_eq!(vitals(&engine).mp, 12);
}

#[test]
fn a_rage_only_character_has_no_mp_to_spend_or_regain() {
    // The level table still gives 24 MP, but the player does not use it.
    let world = fighters(Some(RAGE_ONLY), &["spark", "slash"]);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut engine = engaged(&world);
    let pools = engine.player_pools().unwrap();
    assert_eq!((pools.mp, pools.rage), (None, Some(10)));
    // Inactive is not empty: the panel shows no MP, and none regenerates on
    // the timeline even though the world regenerates 3% per turn.
    assert_eq!(vitals(&engine).mp, 0);
    // The witch's opening hex: 6 of 34 HP at 20 Rage per full bar is 3 Rage.
    assert_eq!(vitals(&engine).rage, 3);
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(cast("slash")),
        Err(EngineError::NotEnoughRage(_))
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(cast("spark")).unwrap();
    assert!(vitals(&engine).rage >= 4);
    let events = engine.execute(cast("slash")).unwrap();
    assert!(events.contains(&Event::ResourceSpent {
        character: "apprentice".into(),
        resource: Resource::Rage,
        amount: 4,
    }));
    assert_eq!(vitals(&engine).mp, 0);
}

#[test]
fn an_explicit_choice_must_cover_every_skill_the_character_can_use() {
    let codes = |world: &WorldSpec| -> Vec<String> {
        world.diagnostics().into_iter().map(|d| d.code).collect()
    };
    // A rage-only player cannot be given an MP skill, nor an MP-only one a Rage skill.
    let world = fighters(Some(RAGE_ONLY), &["spark", "bolt"]);
    assert!(codes(&world).contains(&"inactive_resource".to_string()));
    let world = fighters(Some(MP_ONLY), &["spark", "storm"]);
    assert!(codes(&world).contains(&"inactive_resource".to_string()));
    let mut world = fighters(Some(BOTH), &["spark", "storm"]);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    // Both forms of cost at once are refused.
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .skills
        .last_mut()
        .unwrap()
        .cost = 1;
    assert!(codes(&world).contains(&"conflicting_costs".to_string()));
    // A rage cost over the cap could never be paid.
    let mut world = fighters(Some(BOTH), &["spark", "storm"]);
    world.world.combat.as_mut().unwrap().resources.rage_cap = Some(2);
    assert!(codes(&world).contains(&"unaffordable_skill".to_string()));
}

#[test]
fn a_skill_can_cost_both_resources_and_pays_mp_first() {
    let world = fighters(Some(BOTH), &["spark", "bolt", "storm"]);
    let mut engine = engaged(&world);
    // 24 MP but 3 Rage after one hex: storm wants 3 Rage, and has it.
    assert_eq!((vitals(&engine).mp, vitals(&engine).rage), (24, 3));
    let events = engine.execute(cast("storm")).unwrap();
    let spent: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::ResourceSpent {
                character,
                resource,
                amount,
            } if character == "apprentice" => Some((*resource, *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(spent, [(Resource::Mp, 6), (Resource::Rage, 3)]);
}

#[test]
fn a_short_resource_refuses_the_skill_and_changes_nothing() {
    let world = fighters(Some(BOTH), &["spark", "bolt", "storm"]);
    let engine = engaged(&world);
    // Enough Rage, too little MP: MP is named.
    let mut low_mp = with_vitals(
        &world,
        &engine,
        Vitals {
            mp: 5,
            ..vitals(&engine)
        },
    );
    let before = low_mp.state().clone();
    assert!(matches!(
        low_mp.execute(cast("storm")),
        Err(EngineError::NotEnoughMp(_))
    ));
    assert_eq!(low_mp.state(), &before);
    // Enough MP, too little Rage: Rage is named.
    let mut low_rage = with_vitals(
        &world,
        &engine,
        Vitals {
            rage: 2,
            ..vitals(&engine)
        },
    );
    let before = low_rage.state().clone();
    assert!(matches!(
        low_rage.execute(cast("storm")),
        Err(EngineError::NotEnoughRage(_))
    ));
    assert_eq!(low_rage.state(), &before);
}

#[test]
fn a_refused_skill_draws_nothing_from_the_combat_stream() {
    // The arena rolls crits; its heavy blow costs 5 Rage the player lacks at first.
    let world = open_pit();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Move(Down)).unwrap();
    engine.execute(Engage("ogre".into())).unwrap();
    assert!(vitals(&engine).rage < 5);
    let before = engine.state().clone();
    assert!(before.rng.as_ref().is_some_and(|r| r.combat.is_some()));
    let blow = UseSkill {
        skill: "heavy_blow".into(),
        target: "ogre".into(),
    };
    assert!(matches!(
        engine.execute(blow),
        Err(EngineError::NotEnoughRage(_))
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn rage_stops_at_the_cap_and_banks_nothing_there() {
    let world = fighters(Some(BOTH), &["spark"]);
    let mut engine = engaged(&world);
    while engine.encounter().is_some() && !engine.is_dead() {
        let v = vitals(&engine);
        assert!(v.rage <= 10);
        if v.rage == 10 {
            assert_eq!(v.rage_remainder, 0);
        }
        assert!(Engine::restore(&world, engine.snapshot()).is_ok());
        engine.execute(cast("spark")).unwrap();
    }
    assert_eq!(vitals(&engine).rage, 10);
}

#[test]
fn rage_belongs_to_one_fight_unless_the_world_carries_it() {
    let carried = fighters(None, &["spark"]);
    let mut legacy = carried.clone();
    legacy.world.combat.as_mut().unwrap().resources.rage_carries = false;
    legacy.world.combat.as_mut().unwrap().resources.rage_decay = None;
    let mut kept = engaged(&carried);
    let mut cleared = engaged(&legacy);
    for engine in [&mut kept, &mut cleared] {
        while engine.encounter().is_some() {
            engine.execute(cast("spark")).unwrap();
        }
        assert!(!engine.is_dead());
    }
    // The same fight, the same Rage at its end; only one world keeps it.
    assert_eq!(vitals(&kept).rage, 10);
    assert_eq!(
        (vitals(&cleared).rage, vitals(&cleared).rage_remainder),
        (0, 0)
    );
    assert_eq!(
        kept.state().combat.as_ref().unwrap().stance,
        Stance::Exploring
    );
}

#[test]
fn world_time_recovers_mp_and_drains_rage_at_fixed_minutes() {
    let world = fighters(Some(BOTH), &["spark", "bolt"]);
    let fresh = Engine::new(&world).unwrap();
    let start = Vitals {
        hp: 34,
        mp: 2,
        rage: 9,
        rage_remainder: 0,
    };
    let mut engine = with_vitals(&world, &fresh, start);
    // Minute 30: one decay boundary, no recovery boundary yet.
    engine.execute(Wait(30)).unwrap();
    assert_eq!((vitals(&engine).mp, vitals(&engine).rage), (2, 8));
    // Minute 60: a quarter of 24 MP, and another point of Rage.
    engine.execute(Wait(30)).unwrap();
    assert_eq!((vitals(&engine).mp, vitals(&engine).rage), (8, 7));
    // Splitting the hour differently crosses the same boundaries.
    let mut once = with_vitals(&world, &fresh, start);
    once.execute(Wait(60)).unwrap();
    assert_eq!(vitals(&once), vitals(&engine));
    let mut thirds = with_vitals(&world, &fresh, start);
    for _ in 0..3 {
        thirds.execute(Wait(20)).unwrap();
    }
    assert_eq!(vitals(&thirds), vitals(&engine));
    // A long wait fills MP to its maximum and drains Rage to 0, no further.
    engine.execute(Wait(600)).unwrap();
    assert_eq!((vitals(&engine).mp, vitals(&engine).rage), (24, 0));
    // The same commands always give the same vitals.
    let mut again = with_vitals(&world, &fresh, start);
    for minutes in [30, 30, 600] {
        again.execute(Wait(minutes)).unwrap();
    }
    assert_eq!(again.state(), engine.state());
}

#[test]
fn resting_restores_hp_and_mp_while_rage_decays_with_the_time_it_takes() {
    let world = fighters(Some(BOTH), &["spark", "bolt"]);
    let fresh = Engine::new(&world).unwrap();
    let mut engine = with_vitals(
        &world,
        &fresh,
        Vitals {
            hp: 3,
            mp: 0,
            rage: 10,
            rage_remainder: 0,
        },
    );
    engine.execute(Rest).unwrap();
    // 480 minutes: 16 decay boundaries drain all 10.
    assert_eq!(
        vitals(&engine),
        Vitals {
            hp: 34,
            mp: 24,
            rage: 0,
            rage_remainder: 0
        }
    );
}

#[test]
fn encounter_time_and_world_time_never_regenerate_the_same_mp() {
    // Only world time recovers: a fight regenerates nothing, and spends no world time.
    let mut world = fighters(Some(MP_ONLY), &["spark", "bolt"]);
    world
        .world
        .combat
        .as_mut()
        .unwrap()
        .resources
        .mp_regen_percent = 0;
    let mut engine = engaged(&world);
    engine.execute(cast("bolt")).unwrap();
    for _ in 0..2 {
        engine.execute(cast("spark")).unwrap();
    }
    assert_eq!(vitals(&engine).mp, 12);
    assert_eq!(engine.state().time, Some(0));
    // Only encounter time regenerates: waiting recovers nothing.
    let mut world = fighters(Some(MP_ONLY), &["spark", "bolt"]);
    world.world.combat.as_mut().unwrap().resources.mp_recovery = None;
    let fresh = Engine::new(&world).unwrap();
    let mut engine = with_vitals(
        &world,
        &fresh,
        Vitals {
            mp: 1,
            ..vitals(&fresh)
        },
    );
    engine.execute(Wait(600)).unwrap();
    assert_eq!(vitals(&engine).mp, 1);
}

#[test]
fn vitals_save_and_load_with_one_owner() {
    let world = fighters(Some(BOTH), &["spark", "bolt", "storm"]);
    let mut engine = engaged(&world);
    engine.execute(cast("storm")).unwrap();
    // Restored mid-fight, the playthrough goes on exactly as the original.
    let snapshot = engine.snapshot();
    let json = snapshot.to_json();
    let mut restored = Engine::restore(&world, SaveSnapshot::from_json(&json).unwrap()).unwrap();
    assert_eq!(restored.state(), engine.state());
    let encounter = engine.encounter().unwrap();
    assert!(encounter.participants[0].vitals.is_none());
    assert!(encounter.participants[1].vitals.is_some());
    for command in [cast("spark"), cast("bolt"), cast("spark")] {
        assert_eq!(
            restored.execute(command.clone()).ok(),
            engine.execute(command).ok()
        );
    }
    assert_eq!(restored.state(), engine.state());
    // Vitals the rules could not leave are refused.
    let broken: Vec<fn(&mut Vitals)> = vec![
        |v| v.rage = 11,
        |v| v.mp = 25,
        |v| v.hp = 35,
        |v| v.rage_remainder = 34,
        |v| (v.rage, v.rage_remainder) = (10, 1),
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut bad = snapshot.clone();
        corrupt(&mut bad.state.combat.as_mut().unwrap().vitals);
        assert!(rejected(&world, bad), "corruption {i} was accepted");
    }
}

#[test]
fn a_resource_the_character_does_not_use_saves_as_nothing() {
    let world = fighters(Some(RAGE_ONLY), &["spark", "slash"]);
    let engine = Engine::new(&world).unwrap();
    let mut bad = engine.snapshot();
    bad.state.combat.as_mut().unwrap().vitals.mp = 1;
    assert!(rejected(&world, bad));
    let world = fighters(Some(MP_ONLY), &["spark", "bolt"]);
    let engine = Engine::new(&world).unwrap();
    for corrupt in [
        |v: &mut Vitals| v.rage = 1,
        |v: &mut Vitals| v.rage_remainder = 1,
    ] {
        let mut bad = engine.snapshot();
        corrupt(&mut bad.state.combat.as_mut().unwrap().vitals);
        assert!(rejected(&world, bad));
    }
}

#[test]
fn rage_that_belongs_to_one_fight_never_saves_outside_it() {
    let world = duel();
    let mut arena = arena();
    let engine = Engine::new(&arena).unwrap();
    let mut bad = engine.snapshot();
    bad.state.combat.as_mut().unwrap().vitals.rage = 1;
    assert!(rejected(&arena, bad));
    // Carried, the same Rage is a save the rules could produce.
    arena.world.combat.as_mut().unwrap().resources.rage_carries = true;
    let engine = Engine::new(&arena).unwrap();
    let mut good = engine.snapshot();
    good.state.combat.as_mut().unwrap().vitals.rage = 1;
    assert!(Engine::restore(&arena, good).is_ok());
    // A world without Rage at all keeps none.
    let engine = Engine::new(&world).unwrap();
    let mut bad = engine.snapshot();
    bad.state.combat.as_mut().unwrap().vitals.rage = 1;
    assert!(rejected(&world, bad));
}

#[test]
fn a_save_in_the_earlier_vitals_layout_is_refused_by_name() {
    let world = duel();
    let engine = Engine::new(&world).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&engine.snapshot().to_json()).unwrap();
    let combat = value["state"]["combat"].as_object_mut().unwrap();
    combat.remove("vitals");
    combat.insert(
        "stance".into(),
        serde_json::json!({ "exploring": { "hp": 34, "mp": 24 } }),
    );
    let Err(EngineError::InvalidSave(why)) =
        SaveSnapshot::from_json(&serde_json::to_vec(&value).unwrap())
    else {
        panic!("the earlier layout loaded");
    };
    assert!(why.contains("earlier layout"), "{why}");
}

#[test]
fn a_world_without_a_timeline_keeps_its_character_but_runs_no_fights() {
    let mut world = fighters(Some(BOTH), &["spark", "bolt", "storm"]);
    world.world.combat.as_mut().unwrap().timeline = None;
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(North)).unwrap();
    assert!(!offered(&engine).contains(&(Engage("witch".into()), true)));
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Engage("witch".into())),
        Err(EngineError::NoEncounters)
    ));
    assert_eq!(engine.state(), &before);
    // Levels, stats and resources still hold, and time still recovers MP.
    let mut low = with_vitals(
        &world,
        &engine,
        Vitals {
            mp: 0,
            ..vitals(&engine)
        },
    );
    low.execute(Wait(60)).unwrap();
    assert_eq!(vitals(&low).mp, 6);
    assert!(Engine::restore(&world, low.snapshot()).is_ok());
}

#[test]
fn a_world_without_combat_has_no_vitals_at_all() {
    let world = archive();
    let mut engine = reader(&world);
    assert_eq!(engine.player_vitals(), None);
    assert_eq!(engine.player_pools(), None);
    engine.execute(Command::Look).unwrap();
    let json = String::from_utf8(engine.snapshot().to_json()).unwrap();
    assert!(!json.contains("vitals") && !json.contains("rage"));
}

#[test]
fn without_a_choice_resources_follow_the_world() {
    // The duel's apprentice has MP and the world builds no Rage.
    let world = duel();
    let engine = Engine::new(&world).unwrap();
    let pools = engine.player_pools().unwrap();
    assert_eq!((pools.mp, pools.rage), (Some(24), None));
    // The demo's fighter has no MP, and the world builds uncapped Rage.
    let world = demo();
    let engine = Engine::new(&world).unwrap();
    let pools = engine.player_pools().unwrap();
    assert_eq!((pools.mp, pools.rage), (None, Some(u32::MAX)));
    let wolf = engine.pools_of("wolf").unwrap();
    assert_eq!(wolf.mp, None);
    assert_eq!(engine.pools_of("nobody"), None);
}

fn physical(id: &str, power: u32, cost: u32, resource: Resource, costs: Option<Costs>) -> Skill {
    Skill {
        channel: Channel::Physical,
        cost,
        resource,
        costs,
        ..skill(id, power, Costs::default())
    }
}

// Numbers from `scripts/combat_sim`: `Encounter(Rules(rage_cap=5), you, [foe]).run()`
// with storm (190, 8 MP and 4 Rage), slash (140, 3 Rage) and a rage-only foe
// whose maul costs 5 Rage. Uncapped, the same fight leaves 18 HP and 1 MP.
#[test]
fn a_capped_dual_resource_fight_matches_the_simulator() {
    let stats = |hp, mp, patk, pdef, satk, sdef, speed| Stats {
        hp,
        mp,
        patk,
        pdef,
        satk,
        sdef,
        speed,
    };
    let storm = physical(
        "storm",
        190,
        0,
        Resource::Mp,
        Some(Costs { mp: 8, rage: 4 }),
    );
    let slash = physical("slash", 140, 3, Resource::Rage, None);
    let maul = physical("maul", 150, 5, Resource::Rage, None);
    let mut world = duel();
    let combat = world.world.combat.as_mut().unwrap();
    combat.levels = vec![Level {
        xp: 0,
        stats: stats(60, 30, 12, 8, 10, 8, 100),
        points: 0,
        proficiency_points: 0,
    }];
    combat.resources = Resources {
        mp_regen_percent: 3,
        rage_per_action: 1,
        rage_per_max_hp: 20,
        rage_cap: Some(5),
        // Kept after the fight, so the player's last Rage can be read.
        rage_carries: true,
        ..Resources::default()
    };
    combat.player_basic_channel = Channel::Physical;
    combat.skills = vec![storm.clone(), slash.clone(), maul];
    combat.player_skills = vec!["storm".into(), "slash".into()];
    let witch = combatant(&mut world, "witch");
    witch.stats = stats(70, 20, 10, 9, 2, 6, 110);
    witch.skills = vec!["maul".into()];
    witch.basic_channel = Channel::Physical;
    witch.resources = Some(RAGE_ONLY);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut engine = engaged(&world);
    let mut actions = 0;
    while engine.encounter().is_some() {
        let me = vitals(&engine);
        let affordable =
            |s: &Skill| me.mp >= s.cost_in(Resource::Mp) && me.rage >= s.cost_in(Resource::Rage);
        let command = match [&storm, &slash].into_iter().find(|s| affordable(s)) {
            Some(s) => cast(&s.id),
            None => Attack("witch".into()),
        };
        engine.execute(command).unwrap();
        actions += 1;
    }
    let left = vitals(&engine);
    assert_eq!((actions, left.hp, left.mp, left.rage), (6, 21, 9, 1));
}
