//! Real-time duels: the fixed-step clock, attack phases, guards, retreats,
//! host-reported contact, shared vitals, settlement and mid-duel saves, in
//! the practice yard.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn yard() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/practice-yard"
    ))
    .unwrap()
}

fn step(n: u32) -> Command {
    Intent(realmkit_engine::Intent::Advance(n))
}

fn strike() -> Command {
    Intent(realmkit_engine::Intent::Attack)
}

fn skill(id: &str) -> Command {
    Intent(realmkit_engine::Intent::Skill(id.into()))
}

fn guard() -> Command {
    Intent(realmkit_engine::Intent::Guard)
}

fn contact(by: &str) -> Command {
    Intent(realmkit_engine::Intent::Contact(by.into()))
}

fn retreat() -> Command {
    Intent(realmkit_engine::Intent::Retreat)
}

fn run(engine: &mut Engine<'_>, commands: &[Command]) -> Vec<Event> {
    commands
        .iter()
        .flat_map(|c| engine.execute(c.clone()).unwrap())
        .collect()
}

fn duel(engine: &Engine<'_>) -> realmkit_engine::Duel {
    engine.duel().unwrap().clone()
}

fn opponent(engine: &Engine<'_>) -> Vitals {
    duel(engine).fighters[1].vitals.unwrap()
}

/// One basic attack by the player that the host reports as landing, from
/// ready to ready again: wind-up 3, active 2, recovery 4.
fn landed_attack() -> Vec<Command> {
    vec![strike(), step(3), contact("trainee"), step(6)]
}

/// A basic hit's damage from the player on `id`, from the shared formula.
fn basic_hit(world: &WorldSpec, engine: &Engine<'_>, id: &str) -> u32 {
    let stats = world.character(id).unwrap().combat.as_ref().unwrap().stats;
    damage_modified(
        &engine.player_stats().unwrap(),
        &stats,
        Channel::Physical,
        BASIC_POWER,
        25,
        100,
        (1, 1),
    )
    .unwrap()
}

/// Everything but the command counter, which counts how a host batched its input.
fn played(engine: &Engine<'_>) -> GameState {
    let mut state = engine.state().clone();
    state.turn = 0;
    state
}

#[test]
fn a_duel_replays_identically_and_settles_a_victory_once() {
    let world = yard();
    let mut commands = vec![Duel("brigand".into()), guard(), step(2)];
    commands.push(Intent(realmkit_engine::Intent::LowerGuard));
    commands.extend(landed_attack());
    commands.push(skill("lunge"));
    // Its phases are 4, 3 and 5 steps long at 120% of a basic attack's time.
    commands.extend([step(4), contact("trainee"), step(8)]);
    let mut first = Engine::new_with_seed(&world, 9).unwrap();
    let mut events = run(&mut first, &commands);
    // Basic hits until the brigand falls, each landing as the host reports.
    while first.duel().is_some() {
        for command in landed_attack() {
            if first.duel().is_some() {
                events.extend(first.execute(command.clone()).unwrap());
                commands.push(command);
            }
        }
    }
    let mut second = Engine::new_with_seed(&world, 9).unwrap();
    assert_eq!(run(&mut second, &commands), events);
    assert_eq!(first.snapshot().to_json(), second.snapshot().to_json());
    // Hits are the shared damage formula's, at the level the duel began.
    let basic = basic_hit(&world, &Engine::new(&world).unwrap(), "brigand");
    let dealt: Vec<u32> = events
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(dealt[0], basic);
    assert!(dealt[1] > basic);

    // The duel settled at the hit that felled the brigand.
    assert!(first.duel().is_none());
    let ended: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::EncounterEnded { .. }))
        .collect();
    assert_eq!(
        ended,
        [&Event::EncounterEnded {
            outcome: Outcome::Victory
        }]
    );
    let combat = combat(&first);
    assert!(combat.defeated.contains("brigand"));
    assert_eq!(combat.xp, 10);
    assert_eq!(combat.level, 2);
    assert_eq!(first.state().player.inventory["coin_purse"], 1);
    // The brigand never struck the player: the host reported no contact.
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::DamageReceived { .. })));
    // Settled once: there is nothing left to finish or fight again.
    assert!(matches!(
        first.execute(step(1)),
        Err(EngineError::NotFighting)
    ));
    assert!(matches!(
        first.execute(Duel("brigand".into())),
        Err(EngineError::AlreadyDefeated(_))
    ));
}

#[test]
fn how_a_host_batches_steps_changes_nothing() {
    let world = yard();
    let mut whole = Engine::new_with_seed(&world, 3).unwrap();
    let mut events = run(&mut whole, &[Duel("master".into()), skill("lunge")]);
    let opening = events.clone();
    events.extend(run(&mut whole, &[step(4), contact("trainee"), step(23)]));
    for batches in [
        vec![1; 23],
        vec![3, 7, 13],
        vec![22, 1],
        vec![5, 5, 5, 5, 3],
    ] {
        let mut split = Engine::new_with_seed(&world, 3).unwrap();
        let mut seen = run(&mut split, &[Duel("master".into()), skill("lunge")]);
        assert_eq!(seen, opening);
        for _ in 0..4 {
            seen.extend(split.execute(step(1)).unwrap());
        }
        seen.extend(split.execute(contact("trainee")).unwrap());
        for n in batches {
            seen.extend(split.execute(step(n)).unwrap());
        }
        assert_eq!(seen, events);
        assert_eq!(played(&split), played(&whole));
    }
    assert_eq!(duel(&whole).step, 27);
}

#[test]
fn an_attack_moves_through_its_phases_and_commits_the_attacker() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), strike()]);
    let phase_at = |engine: &Engine<'_>| match duel(engine).fighters[0].action {
        DuelAction::Attack { phase, until, .. } => Some((phase, until)),
        _ => None,
    };
    assert_eq!(phase_at(&engine), Some((AttackPhase::Windup, 3)));
    // Committed: nothing else starts, and nothing lands, during the wind-up.
    for refused in [strike(), skill("lunge"), guard(), retreat()] {
        assert!(matches!(
            engine.execute(refused),
            Err(EngineError::Committed)
        ));
    }
    assert!(matches!(
        engine.execute(contact("trainee")),
        Err(EngineError::NoContact(_))
    ));
    run(&mut engine, &[step(2)]);
    assert_eq!(phase_at(&engine), Some((AttackPhase::Windup, 3)));
    let events = run(&mut engine, &[step(1)]);
    assert!(events.contains(&Event::Swing {
        character: "trainee".into(),
        skill: None,
        phase: AttackPhase::Active,
    }));
    assert_eq!(phase_at(&engine), Some((AttackPhase::Active, 5)));
    run(&mut engine, &[contact("trainee")]);
    // An attack lands once.
    assert!(matches!(
        engine.execute(contact("trainee")),
        Err(EngineError::NoContact(_))
    ));
    run(&mut engine, &[step(2)]);
    assert_eq!(phase_at(&engine), Some((AttackPhase::Recovery, 9)));
    // Recovery is committed too, and lands nothing.
    assert!(matches!(
        engine.execute(strike()),
        Err(EngineError::Committed)
    ));
    let events = run(&mut engine, &[step(4)]);
    assert!(events.contains(&Event::Ready {
        character: "trainee".into()
    }));
    assert_eq!(duel(&engine).fighters[0].action, DuelAction::Ready);
    // An attack that closes its window untouched is a miss, and earns no rage.
    let rage = vitals(&engine).rage;
    let events = run(&mut engine, &[strike(), step(5)]);
    assert!(events.contains(&Event::Missed {
        character: "trainee".into()
    }));
    assert_eq!(vitals(&engine).rage, rage);
}

#[test]
fn speed_and_skill_time_scale_the_phases() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    // The lunge takes 120% of a basic attack's time: ceil(3 × 1.2) = 4 steps.
    run(&mut engine, &[Duel("master".into()), skill("lunge")]);
    let DuelAction::Attack { until, .. } = duel(&engine).fighters[0].action else {
        panic!("attacking");
    };
    assert_eq!(until, 4);
    // The brigand, at speed 80, winds up for ceil(3 × 100 / 80) = 4 steps,
    // beginning after the first step.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into()), step(1)]);
    let DuelAction::Attack { phase, until, .. } = duel(&engine).fighters[1].action else {
        panic!("attacking");
    };
    assert_eq!((phase, until), (AttackPhase::Windup, 5));
}

#[test]
fn a_raised_guard_lets_through_its_share_of_a_hit() {
    let world = yard();
    // The master's first attack, a sweep, is active from step 4 to 6.
    let hit = |guarding: bool| {
        let mut engine = Engine::new(&world).unwrap();
        run(&mut engine, &[Duel("master".into())]);
        if guarding {
            run(&mut engine, &[guard()]);
        }
        let events = run(&mut engine, &[step(4), contact("master")]);
        (vitals(&engine).hp, events)
    };
    let (open, _) = hit(false);
    let (guarded, events) = hit(true);
    let full = 40 - open;
    let through = full * 25 / 100;
    assert_eq!(40 - guarded, through);
    assert!(events.contains(&Event::Blocked {
        character: "trainee".into(),
        prevented: full - through,
    }));
    // A guard still rising stops nothing.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), step(4), guard()]);
    run(&mut engine, &[contact("master")]);
    assert_eq!(vitals(&engine).hp, open);
}

#[test]
fn refused_intents_change_nothing_and_draw_nothing() {
    let world = yard();
    let mut engine = Engine::new_with_seed(&world, 5).unwrap();
    assert!(engine.state().rng.unwrap().combat.is_some());
    run(&mut engine, &[Duel("brigand".into()), step(2)]);
    type Expected = fn(&EngineError) -> bool;
    let refusals: Vec<(Command, Expected)> = vec![
        (step(0), |e| matches!(e, EngineError::InvalidSteps)),
        (step(ADVANCE_BOUND + 1), |e| {
            matches!(e, EngineError::InvalidSteps)
        }),
        (skill("sweep"), |e| {
            matches!(e, EngineError::UnknownSkill(_))
        }),
        // Rage starts at 0, so the rush is unaffordable and costs nothing.
        (skill("rush"), |e| {
            matches!(e, EngineError::NotEnoughRage(_))
        }),
        (contact("brigand"), |e| {
            matches!(e, EngineError::NoContact(_))
        }),
        (contact("trainee"), |e| {
            matches!(e, EngineError::NoContact(_))
        }),
        (contact("master"), |e| matches!(e, EngineError::NotHere(_))),
        (Intent(realmkit_engine::Intent::LowerGuard), |e| {
            matches!(e, EngineError::NotGuarding)
        }),
        (retreat(), |e| matches!(e, EngineError::NoFlee)),
        (Wait(30), |e| matches!(e, EngineError::InEncounter)),
        (Rest, |e| matches!(e, EngineError::InEncounter)),
        (Engage("master".into()), |e| {
            matches!(e, EngineError::InEncounter)
        }),
        (Duel("master".into()), |e| {
            matches!(e, EngineError::InEncounter)
        }),
        (Attack("brigand".into()), |e| {
            matches!(e, EngineError::InEncounter)
        }),
        (Flee, |e| matches!(e, EngineError::InEncounter)),
    ];
    let before = engine.snapshot().to_json();
    for (command, expected) in refusals {
        let error = engine.execute(command.clone()).unwrap_err();
        assert!(expected(&error), "{command:?} gave {error:?}");
        assert_eq!(engine.snapshot().to_json(), before, "{command:?}");
    }
    // Outside a duel there is nothing to give intents to, and a pair does
    // not fight one-on-one.
    let mut engine = Engine::new_with_seed(&world, 5).unwrap();
    let before = engine.snapshot().to_json();
    assert!(matches!(
        engine.execute(strike()),
        Err(EngineError::NotFighting)
    ));
    assert!(matches!(
        engine.execute(Duel("left_twin".into())),
        Err(EngineError::NotOneOnOne(_))
    ));
    assert_eq!(engine.snapshot().to_json(), before);
}

#[test]
fn the_duel_reads_and_writes_the_players_one_copy_of_their_vitals() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into())]);
    assert_eq!(duel(&engine).fighters[0].vitals, None);
    // A skill is paid for at its wind-up, from the player's own MP.
    let events = run(&mut engine, &[skill("lunge")]);
    assert!(events.contains(&Event::ResourceSpent {
        character: "trainee".into(),
        resource: Resource::Mp,
        amount: 4,
    }));
    assert_eq!(vitals(&engine).mp, 8);
    assert_eq!(engine.player_vitals(), Some(combat(&engine).vitals));
    // Landing earns rage_per_action; taking damage earns rage from the hit.
    run(&mut engine, &[step(4), contact("trainee")]);
    assert_eq!(vitals(&engine).rage, 1);
    let hp = opponent(&engine).hp;
    assert!(hp < 40);
    let dealt = 40 - hp;
    assert_eq!(opponent(&engine).rage, 10 * dealt / 40);
    assert_eq!(opponent(&engine).rage_remainder, u64::from(10 * dealt % 40));
    // Duel time regenerates MP: 10% of 12 a second, at 10 steps a second.
    let before = vitals(&engine).mp;
    run(&mut engine, &[step(10)]);
    assert_eq!(vitals(&engine).mp, before + 1);
    // The master lands a sweep; the wound stays with the player after the bout.
    while !matches!(
        duel(&engine).fighters[1].action,
        DuelAction::Attack {
            phase: AttackPhase::Active,
            ..
        }
    ) {
        run(&mut engine, &[step(1)]);
    }
    run(&mut engine, &[contact("master")]);
    let wounded = vitals(&engine);
    assert!(wounded.hp < 40);
    run(&mut engine, &[retreat(), step(6)]);
    assert!(engine.duel().is_none());
    let after = vitals(&engine);
    assert_eq!(after.hp, wounded.hp);
    // Rage belongs to one fight in this world.
    assert_eq!((after.rage, after.rage_remainder), (0, 0));
}

#[test]
fn yielding_and_retreating_settle_their_own_way() {
    let world = yard();
    // The master yields at half HP: the player wins the bout, nobody dies.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into())]);
    let per_hit = basic_hit(&world, &engine, "master");
    let mut events = Vec::new();
    while engine.duel().is_some() {
        events.extend(run(&mut engine, &landed_attack()[..2]));
        events.extend(run(&mut engine, &[contact("trainee")]));
        if engine.duel().is_some() {
            events.extend(run(&mut engine, &[step(6)]));
        }
    }
    assert!(per_hit > 0);
    assert!(events.contains(&Event::Yielded {
        character: "master".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    assert!(engine.state().flags.contains("spar_won"));
    // A yielded sparring partner is not defeated, and spars again.
    assert!(!combat(&engine).defeated.contains("master"));
    assert_eq!(combat(&engine).xp, 0);
    run(&mut engine, &[Duel("master".into())]);

    // The player yields when the master's blows bring them to half HP.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into())]);
    let mut events = Vec::new();
    while engine.duel().is_some() {
        let active = matches!(
            duel(&engine).fighters[1].action,
            DuelAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            }
        );
        let command = if active { contact("master") } else { step(1) };
        events.extend(engine.execute(command).unwrap());
    }
    assert!(vitals(&engine).hp <= 20 && vitals(&engine).hp > 0);
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Yielded
    }));
    assert!(engine.state().flags.contains("spar_lost"));

    // A retreat takes its steps, then grants and records nothing.
    let mut engine = Engine::new(&world).unwrap();
    let events = run(&mut engine, &[Duel("master".into()), retreat(), step(5)]);
    assert!(events.contains(&Event::RetreatStarted));
    assert!(engine.duel().is_some());
    let events = run(&mut engine, &[step(10)]);
    assert_eq!(
        events.last(),
        Some(&Event::EncounterEnded {
            outcome: Outcome::Fled
        })
    );
    assert!(engine.duel().is_none());
    assert!(engine.state().flags.is_empty());
}

#[test]
fn a_player_killed_in_a_duel_stays_dead() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into())]);
    let mut events = Vec::new();
    while !engine.is_dead() {
        let active = matches!(
            duel(&engine).fighters[1].action,
            DuelAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            }
        );
        let command = if active { contact("brigand") } else { step(1) };
        events.extend(engine.execute(command).unwrap());
    }
    assert_eq!(events.last(), Some(&Event::PlayerDied));
    assert!(engine.duel().is_some());
    assert!(matches!(
        engine.execute(step(1)),
        Err(EngineError::PlayerDead)
    ));
    // The death is saved as it stands, and loads.
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn a_duel_saved_at_any_step_resumes_exactly() {
    let world = yard();
    let script: Vec<Command> = [
        vec![Duel("master".into()), guard(), step(4), contact("master")],
        landed_attack(),
        vec![skill("lunge"), step(2)],
        vec![step(2), contact("trainee"), step(5), contact("master")],
        vec![step(5), retreat(), step(6)],
    ]
    .concat();
    let mut whole = Engine::new_with_seed(&world, 2).unwrap();
    let events = run(&mut whole, &script);
    for cut in 1..script.len() {
        let mut first = Engine::new_with_seed(&world, 2).unwrap();
        let mut seen = run(&mut first, &script[..cut]);
        let bytes = first.snapshot().to_json();
        let mut resumed =
            Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
        seen.extend(run(&mut resumed, &script[cut..]));
        assert_eq!(seen, events, "cut at {cut}");
        assert_eq!(resumed.snapshot().to_json(), whole.snapshot().to_json());
    }
}

#[test]
fn saves_with_duel_states_the_rules_cannot_reach_are_refused() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), strike(), step(1)]);
    Engine::restore(&world, engine.snapshot()).unwrap();
    let tamper = |edit: &dyn Fn(&mut realmkit_engine::Duel, &mut Vitals)| {
        let mut snapshot = engine.snapshot();
        let combat = snapshot.state.combat.as_mut().unwrap();
        let Stance::Dueling(duel) = &mut combat.stance else {
            panic!("dueling");
        };
        edit(duel, &mut combat.vitals);
        Engine::restore(&world, snapshot).is_err()
    };
    let attack = |f: &mut Fighter, to: u64, landed: bool| {
        let DuelAction::Attack {
            until, connected, ..
        } = &mut f.action
        else {
            panic!("attacking");
        };
        (*until, *connected) = (to, landed);
    };
    assert!(tamper(
        &|d, _| d.fighters[0].vitals = Some(Vitals::default())
    ));
    assert!(tamper(&|d, _| d.fighters[1].vitals = None));
    assert!(tamper(&|d, _| d.fighters[1].character = "brigand".into()));
    assert!(tamper(&|d, _| d.fighters[1].action = DuelAction::Ready));
    assert!(tamper(
        &|d, _| d.fighters[1].action = DuelAction::Guard { ready_at: 1 }
    ));
    assert!(tamper(&|d, _| d.fighters[0].mp_remainder = 1_000));
    // The wind-up began at step 0 and ends at 3: not in the past, and no
    // longer than the slowest player could hold it (a rank earned mid-duel
    // may change their speed), 3 × 100 steps at speed 1. The opponent's
    // stats never change, so its phases are exact: its wind-up from step 1
    // ends at 4.
    assert!(tamper(&|d, _| attack(&mut d.fighters[0], 1, false)));
    assert!(!tamper(&|d, _| attack(&mut d.fighters[0], 301, false)));
    assert!(tamper(&|d, _| attack(&mut d.fighters[0], 302, false)));
    assert!(tamper(&|d, _| attack(&mut d.fighters[1], 5, false)));
    assert!(tamper(&|d, _| attack(&mut d.fighters[0], 3, true)));
    assert!(tamper(
        &|d, _| d.fighters[0].action = DuelAction::Retreat { until: 99 }
    ));
    assert!(tamper(&|d, _| d.fighters[1].vitals.as_mut().unwrap().hp = 0));
    assert!(tamper(&|d, _| d.fighters[1].yielded = true));
    assert!(tamper(&|d, _| {
        d.fighters[0].action = DuelAction::Attack {
            skill: Some("sweep".into()),
            phase: AttackPhase::Windup,
            until: 3,
            connected: false,
        }
    }));
    // A world without real-time rules has no duels to resume.
    let mut snapshot = engine.snapshot();
    let mut plain = yard();
    plain.world.combat.as_mut().unwrap().realtime = None;
    snapshot.package_revision = plain.revision();
    assert!(Engine::restore(&plain, snapshot).is_err());
}

#[test]
fn duel_time_is_neither_world_time_nor_encounter_time() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), skill("lunge")]);
    let minute = engine.state().time;
    // A long duel passes no world time, so world recovery never runs in it.
    run(&mut engine, &[step(ADVANCE_BOUND)]);
    assert_eq!(engine.state().time, minute);
    // Duel regeneration is the only MP the duel restores, and its progress
    // ends with the duel.
    assert_eq!(vitals(&engine).mp, 12);
    run(&mut engine, &[retreat(), step(6)]);
    assert!(engine.duel().is_none());
    assert_eq!(engine.state().time, minute);
    // A turn-based encounter in the same world regenerates on its own
    // timeline only, as before.
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[
            Engage("master".into()),
            UseSkill {
                skill: "lunge".into(),
                target: "master".into(),
            },
        ],
    );
    assert!(engine.encounter().is_some());
    assert!(engine.duel().is_none());
}

#[test]
fn duels_are_offered_only_where_the_world_runs_them() {
    let world = yard();
    let engine = Engine::new(&world).unwrap();
    let offers = offered(&engine);
    // Fighters who fight alone, beside turn-based engagement.
    for id in ["master", "brigand"] {
        assert!(offers.contains(&(Duel(id.into()), true)));
    }
    assert!(!offers.contains(&(Duel("left_twin".into()), true)));
    // In a duel: the intents that are legal now; contact is never offered.
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("brigand".into()), strike()]);
    let offers = offered(&engine);
    assert!(offers.contains(&(step(1), true)));
    assert!(offers.contains(&(strike(), false)));
    assert!(!offers
        .iter()
        .any(|(c, _)| matches!(c, Intent(realmkit_engine::Intent::Contact(_)))));
    assert!(
        !offers.contains(&(retreat(), true)),
        "the brigand allows no flight"
    );

    // A world with combat but no real-time rules offers no duel and refuses one.
    let arena = arena();
    let engine = Engine::new(&arena).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Duel(_))));
    let mut plain = yard();
    plain.world.combat.as_mut().unwrap().realtime = None;
    let mut engine = Engine::new(&plain).unwrap();
    let before = engine.snapshot().to_json();
    assert!(matches!(
        engine.execute(Duel("master".into())),
        Err(EngineError::NoDuels)
    ));
    assert_eq!(engine.snapshot().to_json(), before);
}

#[test]
fn worlds_without_combat_have_no_duels_and_no_duel_state() {
    let world = archive();
    let mut engine = reader(&world);
    assert!(engine.state().combat.is_none());
    assert!(engine.duel().is_none());
    let before = engine.snapshot().to_json();
    assert!(engine.execute(Duel("archivist".into())).is_err());
    assert!(matches!(engine.execute(step(1)), Err(EngineError::NoDuels)));
    assert_eq!(engine.snapshot().to_json(), before);
    assert!(!String::from_utf8(before).unwrap().contains("duel"));
}

#[test]
fn a_clock_at_its_limit_refuses_to_run_on() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    run(&mut engine, &[Duel("master".into()), step(2)]);
    // A duel one step from the clock's end, its opponent mid-wind-up.
    let mut snapshot = engine.snapshot();
    let Stance::Dueling(duel) = &mut snapshot.state.combat.as_mut().unwrap().stance else {
        panic!("dueling");
    };
    duel.step = u64::MAX - 1;
    let DuelAction::Attack { until, .. } = &mut duel.fighters[1].action else {
        panic!("attacking");
    };
    *until = u64::MAX;
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    let before = engine.snapshot().to_json();
    for refused in [step(1), retreat(), strike()] {
        assert!(matches!(
            engine.execute(refused),
            Err(EngineError::NumericLimit)
        ));
        assert_eq!(engine.snapshot().to_json(), before);
    }
}

/// The yard with the lunge taught as a technique whose second rank, earned
/// by one landed lunge, is a faster thrust.
fn yard_with_lunge_form() -> WorldSpec {
    let mut world = yard();
    let combat = world.world.combat.as_mut().unwrap();
    let mut thrust = combat
        .skills
        .iter()
        .find(|s| s.id == "lunge")
        .unwrap()
        .clone();
    thrust.id = "thrust".into();
    combat.skills.push(thrust);
    combat.player_skills.retain(|s| s != "lunge");
    combat.techniques.push(
        serde_json::from_value(serde_json::json!({
            "id": "lunge_form",
            "name": "Lunge Form",
            "ranks": [
                { "name": "First Form", "xp": 0, "skill": "lunge" },
                { "name": "Second Form", "xp": 10, "skill": "thrust", "passive": { "speed": 50 } }
            ]
        }))
        .unwrap(),
    );
    combat.player_techniques.push(TechniqueGrant {
        technique: "lunge_form".into(),
        rank: None,
        xp: 0,
    });
    world.validate().unwrap();
    world
}

#[test]
fn a_rank_earned_mid_attack_leaves_a_save_that_loads() {
    let world = yard_with_lunge_form();
    let mut engine = Engine::new(&world).unwrap();
    run(
        &mut engine,
        &[Duel("master".into()), skill("lunge"), step(4)],
    );
    let events = run(&mut engine, &[contact("trainee")]);
    assert!(events.contains(&Event::TechniqueRankUp {
        technique: "lunge_form".into(),
        rank: 2,
    }));
    // The lunge is still active, though the player now knows the thrust,
    // and faster: its remaining steps were set at the old speed.
    let DuelAction::Attack {
        skill: committed,
        phase,
        ..
    } = duel(&engine).fighters[0].action.clone()
    else {
        panic!("attacking");
    };
    assert_eq!(
        (committed.as_deref(), phase),
        (Some("lunge"), AttackPhase::Active)
    );
    assert_eq!(engine.player_stats().unwrap().speed, 150);
    let mut whole = engine.clone();
    let mut resumed = Engine::restore(&world, engine.snapshot()).unwrap();
    let rest = [step(1), step(6), skill("thrust"), step(4)];
    assert_eq!(run(&mut resumed, &rest), run(&mut whole, &rest));
    assert_eq!(resumed.snapshot().to_json(), whole.snapshot().to_json());
    // A skill the player never knew is still refused.
    let mut snapshot = whole.snapshot();
    let Stance::Dueling(duel) = &mut snapshot.state.combat.as_mut().unwrap().stance else {
        panic!("dueling");
    };
    let DuelAction::Attack {
        skill: committed, ..
    } = &mut duel.fighters[0].action
    else {
        panic!("attacking");
    };
    *committed = Some("sweep".into());
    assert!(Engine::restore(&world, snapshot).is_err());
}

#[test]
fn a_player_who_enters_a_yielding_duel_already_low_can_save_and_yields_when_hit() {
    let world = yard();
    let mut engine = Engine::new(&world).unwrap();
    let hit_by_master = |engine: &mut Engine<'_>| loop {
        let active = matches!(
            duel(engine).fighters[1].action,
            DuelAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            }
        );
        let command = if active { contact("master") } else { step(1) };
        let events = engine.execute(command).unwrap();
        if active {
            return events;
        }
    };
    run(&mut engine, &[Duel("master".into())]);
    while engine.duel().is_some() {
        hit_by_master(&mut engine);
    }
    assert!(engine.state().flags.contains("spar_lost"));
    // Straight back into the circle at or below half HP.
    run(&mut engine, &[Duel("master".into())]);
    assert!(vitals(&engine).hp <= 20);
    let restored = Engine::restore(&world, engine.snapshot());
    assert!(restored.is_ok(), "{:?}", restored.err());
    let events = hit_by_master(&mut engine);
    assert!(events.contains(&Event::Yielded {
        character: "trainee".into()
    }));
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Yielded
    }));
}

#[test]
fn narrative_variants_follow_the_duel_clock_not_the_command_count() {
    let mut world = yard();
    let narrative = &mut world.world.combat.as_mut().unwrap().narrative;
    narrative.attack.push(TextTemplate(
        "{attacker} catch {target}: {damage} damage.".into(),
    ));
    narrative.hurt.push(TextTemplate(
        "{attacker} clips {target}: {damage} damage.".into(),
    ));
    let variants = |batches: &[u32]| {
        let mut engine = Engine::new(&world).unwrap();
        run(&mut engine, &[Duel("master".into()), strike()]);
        for &n in batches {
            run(&mut engine, &[step(n)]);
        }
        let mut seen: Vec<usize> = run(&mut engine, &[contact("trainee")])
            .into_iter()
            .filter_map(|e| match e {
                Event::DamageDealt { variant, .. } => Some(variant),
                _ => None,
            })
            .collect();
        // The master's sweep is active at step 4.
        run(&mut engine, &[step(1), contact("master")])
            .into_iter()
            .for_each(|e| {
                if let Event::DamageReceived { variant, .. } = e {
                    seen.push(variant);
                }
            });
        seen
    };
    let whole = variants(&[3]);
    assert_eq!(whole, [1, 0], "step 3 then step 4, of two variants each");
    assert_eq!(variants(&[1, 2]), whole);
    assert_eq!(variants(&[1, 1, 1]), whole);
}
