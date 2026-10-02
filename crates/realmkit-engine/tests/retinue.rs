//! Recruiting soldiers, their promotion and upgrades, and the upkeep that
//! pays, heals and loses them.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn squad(engine: &Engine<'_>, line: &str, level: usize) -> Option<Squad> {
    engine
        .state()
        .retinue
        .as_ref()?
        .roster
        .get(line)?
        .get(&level)
        .copied()
}

fn pool(engine: &Engine<'_>) -> u64 {
    engine.state().retinue.as_ref().unwrap().pools["ashmere"]["levy"]
}

fn currency(engine: &Engine<'_>) -> u64 {
    engine.state().economy.as_ref().unwrap().currency
}

fn recruit(line: &str, quantity: u64) -> Command {
    Recruit {
        line: line.into(),
        quantity,
    }
}

/// At Ashmere, where levies are raised, with `marks` to spend.
fn at_ashmere(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine
}

fn marches_with(silver: u64) -> WorldSpec {
    let mut world = marches();
    world.world.economy.as_mut().unwrap().currency.start = silver;
    world
}

/// A save whose roster holds exactly `squads`, loaded into `world`.
fn with_roster<'w>(world: &'w WorldSpec, squads: &[(&str, usize, Squad)]) -> Engine<'w> {
    let mut snapshot = Engine::new_with_seed(world, 7).unwrap().snapshot();
    let roster = &mut snapshot.state.retinue.as_mut().unwrap().roster;
    for (line, level, squad) in squads {
        roster
            .entry((*line).into())
            .or_default()
            .insert(*level, *squad);
    }
    Engine::restore(world, snapshot).unwrap()
}

fn healthy(healthy: u64, xp: u64) -> Squad {
    Squad {
        healthy,
        wounded: 0,
        xp,
    }
}

#[test]
fn levies_are_raised_from_the_pool_for_silver() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(pool(&engine), 8);
    assert!(matches!(
        engine.execute(recruit("levy", 1)),
        Err(EngineError::NotRecruitedHere(_))
    ));
    engine.execute(Travel("ashmere".into())).unwrap();
    let events = engine.execute(recruit("levy", 3)).unwrap();
    assert_eq!(
        events,
        [Event::Recruited {
            line: "levy".into(),
            quantity: 3,
            cost: 30
        }]
    );
    assert_eq!(squad(&engine, "levy", 1), Some(healthy(3, 0)));
    assert_eq!((pool(&engine), currency(&engine)), (5, 70));
    // Refusals change nothing.
    let before = engine.state().clone();
    for (command, refused) in [
        (recruit("levy", 6), "pool"),
        (recruit("levy", 0), "quantity"),
        (recruit("bowmen", 1), "not offered"),
    ] {
        assert!(engine.execute(command).is_err(), "{refused}");
    }
    assert_eq!(engine.state(), &before);
    let poor_world = marches_with(25);
    let mut poor = at_ashmere(&poor_world);
    assert!(matches!(
        poor.execute(recruit("levy", 3)),
        Err(EngineError::NotEnoughCurrency)
    ));
    // The roster holds at most its limit.
    let mut small = marches();
    small.world.troops.as_mut().unwrap().limit = 2;
    let mut engine = at_ashmere(&small);
    assert!(matches!(
        engine.execute(recruit("levy", 3)),
        Err(EngineError::RosterFull)
    ));
    engine.execute(recruit("levy", 2)).unwrap();
}

#[test]
fn an_upgrade_carries_its_share_and_may_promote_at_once() {
    // Two spearmen sharing 40 XP: one turns bowman with 20, which is past
    // the 15 a marksman needs.
    let world = marches_with(500);
    let mut engine = with_roster(&world, &[("levy", 3, healthy(2, 40))]);
    let events = engine
        .execute(Upgrade {
            line: "levy".into(),
            to: "bowmen".into(),
            quantity: 1,
        })
        .unwrap();
    assert_eq!(
        events,
        [
            Event::Upgraded {
                line: "levy".into(),
                to: "bowmen".into(),
                quantity: 1,
                cost: 20
            },
            Event::Promoted {
                line: "bowmen".into(),
                level: 2,
                count: 1
            }
        ]
    );
    assert_eq!(squad(&engine, "levy", 3), Some(healthy(1, 20)));
    assert_eq!(squad(&engine, "bowmen", 1), None);
    assert_eq!(squad(&engine, "bowmen", 2), Some(healthy(1, 5)));
    assert_eq!(currency(&engine), 480);
    // Only from the last level, only into a listed branch, only healthy soldiers.
    for (line, to, quantity) in [
        ("levy", "outlaws", 1),
        ("levy", "bowmen", 2),
        ("bowmen", "riders", 1),
    ] {
        assert!(engine
            .execute(Upgrade {
                line: line.into(),
                to: to.into(),
                quantity
            })
            .is_err());
    }
}

#[test]
fn unpaid_wages_cost_deserters_and_the_wounded_mend() {
    // Five levies at a silver a day, and no silver left after raising them.
    let world = marches_with(50);
    let mut engine = at_ashmere(&world);
    engine.execute(recruit("levy", 5)).unwrap();
    assert_eq!(currency(&engine), 0);
    // The first upkeep is at minute 1,440; play starts at 480 and the road took 120.
    let events = engine.execute(Wait(1_440 - 600)).unwrap();
    assert!(events.contains(&Event::Deserted {
        line: "levy".into(),
        level: 1,
        count: 1
    }));
    assert_eq!(squad(&engine, "levy", 1), Some(healthy(4, 0)));
    // Paid wages leave everyone.
    let world = marches();
    let mut engine = at_ashmere(&world);
    engine.execute(recruit("levy", 5)).unwrap();
    let events = engine.execute(Wait(1_440 - 600)).unwrap();
    assert!(events.contains(&Event::WagesPaid { amount: 5 }));
    assert_eq!(currency(&engine), 45);
    // Half the wounded mend each day, rounded up; resting mends the rest.
    let world = marches();
    let wounded = Squad {
        healthy: 2,
        wounded: 3,
        xp: 0,
    };
    let mut engine = with_roster(&world, &[("levy", 1, wounded)]);
    let events = engine.execute(Wait(1_440 - 480)).unwrap();
    assert!(events.contains(&Event::Recovered {
        line: "levy".into(),
        level: 1,
        count: 2
    }));
    assert_eq!(
        squad(&engine, "levy", 1),
        Some(Squad {
            healthy: 4,
            wounded: 1,
            xp: 0
        })
    );
    engine.execute(Rest).unwrap();
    assert_eq!(squad(&engine, "levy", 1), Some(healthy(5, 0)));
}

#[test]
fn pools_refill_each_day_up_to_their_size() {
    let world = marches_with(500);
    let mut engine = at_ashmere(&world);
    engine.execute(recruit("levy", 8)).unwrap();
    assert!(matches!(
        engine.execute(recruit("levy", 1)),
        Err(EngineError::PoolEmpty(_))
    ));
    engine.execute(Wait(1_440 - 600)).unwrap();
    assert_eq!(pool(&engine), 2);
    engine.execute(Wait(10 * 1_440)).unwrap();
    assert_eq!(pool(&engine), 8);
}

#[test]
fn recruiting_and_upgrades_are_offered_where_they_can_happen() {
    let world = marches();
    let mut engine = at_ashmere(&world);
    let offers = offered(&engine);
    assert!(offers.contains(&(recruit("levy", 1), true)));
    assert!(offers.contains(&(Retinue, true)));
    engine.execute(recruit("levy", 1)).unwrap();
    let world = marches_with(30);
    let engine = with_roster(&world, &[("levy", 3, healthy(1, 0))]);
    let offers = offered(&engine);
    let upgrade = |to: &str| Upgrade {
        line: "levy".into(),
        to: to.into(),
        quantity: 1,
    };
    assert!(offers.contains(&(upgrade("bowmen"), true)));
    assert!(offers.contains(&(upgrade("riders"), false)));
}

#[test]
fn saves_reject_rosters_the_rules_could_not_produce() {
    let world = marches();
    let good = Engine::new_with_seed(&world, 7).unwrap().snapshot();
    let broken: Vec<fn(&mut RetinueState)> = vec![
        // A level the line does not have.
        |r| {
            r.roster
                .entry("levy".into())
                .or_default()
                .insert(4, healthy(1, 0));
        },
        // A line that does not exist.
        |r| {
            r.roster
                .entry("archers".into())
                .or_default()
                .insert(1, healthy(1, 0));
        },
        // An empty squad.
        |r| {
            r.roster
                .entry("levy".into())
                .or_default()
                .insert(1, healthy(0, 5));
        },
        // Past the roster limit.
        |r| {
            r.roster
                .entry("levy".into())
                .or_default()
                .insert(1, healthy(31, 0));
        },
        // Enough XP to have been promoted.
        |r| {
            r.roster
                .entry("levy".into())
                .or_default()
                .insert(1, healthy(2, 12));
        },
        // A pool past its size, or where nobody recruits.
        |r| {
            r.pools.get_mut("ashmere").unwrap().insert("levy".into(), 9);
        },
        |r| {
            r.pools.insert("greyford".into(), Default::default());
        },
    ];
    for (i, corrupt) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(snapshot.state.retinue.as_mut().unwrap());
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "corruption {i} was accepted"
        );
    }
    let mut snapshot = good;
    snapshot.state.retinue = None;
    assert!(Engine::restore(&world, snapshot).is_err());
}

#[test]
fn deserters_leave_behind_xp_that_may_promote_the_rest() {
    // Five unpaid levies with 29 XP (5 each): one deserts with 5, and the
    // remaining 24 among four is a share of 6, enough for level 2.
    let world = marches_with(0);
    let squad = Squad {
        healthy: 5,
        wounded: 0,
        xp: 29,
    };
    let mut engine = with_roster(&world, &[("levy", 1, squad)]);
    engine.execute(Wait(1_440 - 480)).unwrap();
    assert!(squad_at(&engine, 1).is_none());
    assert_eq!(squad_at(&engine, 2).map(|s| s.healthy), Some(4));
    Engine::restore(&world, engine.snapshot()).unwrap();
}

fn squad_at(engine: &Engine<'_>, level: usize) -> Option<Squad> {
    squad(engine, "levy", level)
}
