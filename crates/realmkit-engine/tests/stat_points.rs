//! Player-allocated stat points and refunds.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

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
