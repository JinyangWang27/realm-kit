//! The seeded generator and critical hits.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

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
    // Only the combat stream: nothing in the arena moves.
    assert_eq!(
        Engine::new_with_seed(&world, 7).unwrap().state().rng,
        Some(RngState {
            version: RNG_VERSION,
            combat: Some(7 ^ 0x636f_6d62_6174),
            world: None,
            market: None,
            battle: None,
        })
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
    stray.state.rng = RngState::for_world(&world, 1);
    assert!(Engine::restore(&duel(), stray).is_err());
    // A stream for a domain the world never draws from is refused too.
    let mut extra = engine.snapshot();
    extra.state.rng.as_mut().unwrap().world = Some(1);
    assert!(Engine::restore(&world, extra).is_err());
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
