//! Host-reported world time: `PassTime` lets minutes pass as a continuous
//! host sees them go by. However the host batches them, the world comes out
//! the same; only the command counter tells the batches apart.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn now(engine: &Engine<'_>) -> u64 {
    engine.state().time.unwrap()
}

/// Everything but the command counter, which counts how a host batched time.
fn played(engine: &Engine<'_>) -> GameState {
    let mut state = engine.state().clone();
    state.turn = 0;
    state
}

/// What a span of time brought, without the `TimePassed` reports that say
/// how it was batched.
fn happenings(events: Vec<Event>) -> Vec<Event> {
    events
        .into_iter()
        .filter(|e| !matches!(e, Event::TimePassed { .. }))
        .collect()
}

/// Passes `minutes` in commits of `each` minutes (the last one shorter),
/// returning everything that happened.
fn pass_in_steps(engine: &mut Engine<'_>, minutes: u64, each: u64) -> Vec<Event> {
    let mut events = Vec::new();
    let mut left = minutes;
    while left > 0 {
        let step = left.min(each);
        events.extend(engine.execute(PassTime(step)).unwrap());
        left -= step;
    }
    happenings(events)
}

/// The marches crossing two days: the thaw at day 2, 14:00, Wenna moving
/// every few hours, the price tick, prosperity, restocking and the retinue's
/// upkeep, all drawn from the world stream.
const TWO_DAYS: u64 = 2 * MINUTES_PER_DAY;

#[test]
fn one_long_pass_and_many_short_ones_make_the_same_world() {
    let world = marches();
    let mut once = Engine::new_with_seed(&world, 7).unwrap();
    let mut minutes = Engine::new_with_seed(&world, 7).unwrap();
    let mut uneven = Engine::new_with_seed(&world, 7).unwrap();
    let long = happenings(once.execute(PassTime(TWO_DAYS)).unwrap());
    assert_eq!(pass_in_steps(&mut minutes, TWO_DAYS, 1), long);
    assert_eq!(pass_in_steps(&mut uneven, TWO_DAYS, 7), long);
    // The thaw is a significant boundary, crossed one minute at a time.
    assert!(long.contains(&Event::StoryFlagSet {
        flag: "thaw".into()
    }));
    assert!(long
        .iter()
        .any(|e| matches!(e, Event::CharacterLeft { .. })));
    assert_eq!(now(&once), 480 + TWO_DAYS);
    // State, prices, stock, whereabouts and every random stream agree.
    assert_eq!(played(&minutes), played(&once));
    assert_eq!(played(&uneven), played(&once));
    assert_eq!(minutes.state().rng, once.state().rng);
    // Only the counter differs: one command per commit.
    assert_eq!(minutes.state().turn, once.state().turn + TWO_DAYS - 1);
}

#[test]
fn passing_time_is_waiting_without_an_authored_wait_or_an_ended_conversation() {
    let world = marches();
    let mut waited = Engine::new_with_seed(&world, 7).unwrap();
    let mut passed = Engine::new_with_seed(&world, 7).unwrap();
    for minutes in [59, 1, 600, 1_440] {
        assert_eq!(
            passed.execute(PassTime(minutes)).unwrap(),
            waited.execute(Wait(minutes)).unwrap()
        );
        assert_eq!(passed.state(), waited.state());
    }
    // Waiting ends a conversation; time passing in front of it does not.
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("wenna".into())).unwrap();
    let open = engine.state().dialogue.clone();
    assert!(open.is_some());
    engine.execute(PassTime(1)).unwrap();
    assert_eq!(engine.state().dialogue, open);
    engine.execute(Wait(1)).unwrap();
    assert_eq!(engine.state().dialogue, None);
    // A world that authors no waiting still lets a host pass time.
    let mut fixed = marches();
    fixed.world.time.as_mut().unwrap().wait = None;
    let mut engine = Engine::new_with_seed(&fixed, 7).unwrap();
    assert!(matches!(
        engine.execute(Wait(1)),
        Err(EngineError::NoWaiting)
    ));
    engine.execute(PassTime(1)).unwrap();
    assert_eq!(now(&engine), 481);
}

#[test]
fn a_conversation_ends_only_when_the_time_takes_its_speaker_away() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("wenna".into())).unwrap();
    let mut ended = None;
    for minute in 1..=TWO_DAYS {
        let events = engine.execute(PassTime(1)).unwrap();
        if events.contains(&Event::DialogueEnded) {
            ended = Some(minute);
            break;
        }
        assert!(engine.state().dialogue.is_some());
    }
    // Wenna moves on within two days, and the conversation goes with her.
    assert!(ended.is_some());
    assert_ne!(wenna(&engine), "greyford");
    assert_eq!(engine.state().dialogue, None);
}

#[test]
fn a_conversation_whose_speaker_leaves_and_returns_in_one_batch_ends_as_any_split_ends_it() {
    let world = marches();
    // When Wenna first leaves Greyford, and when she is first back.
    let mut probe = Engine::new_with_seed(&world, 7).unwrap();
    let (mut left, mut back) = (None, None);
    while back.is_none() && now(&probe) < 480 + DURATION_BOUND {
        probe.execute(PassTime(1)).unwrap();
        match (left, wenna(&probe) == "greyford") {
            (None, false) => left = Some(now(&probe)),
            (Some(_), true) => back = Some(now(&probe)),
            _ => {}
        }
    }
    let span = back.expect("Wenna comes back to Greyford") - 480;
    let mut batch = Engine::new_with_seed(&world, 7).unwrap();
    let mut split = Engine::new_with_seed(&world, 7).unwrap();
    batch.execute(Talk("wenna".into())).unwrap();
    split.execute(Talk("wenna".into())).unwrap();
    let once = happenings(batch.execute(PassTime(span)).unwrap());
    let minutes = pass_in_steps(&mut split, span, 1);
    // She is back, but the conversation ended when she went.
    assert_eq!(wenna(&batch), "greyford");
    assert_eq!(batch.state().dialogue, None);
    assert_eq!(once, minutes);
    assert_eq!(
        once.iter().filter(|e| **e == Event::DialogueEnded).count(),
        1
    );
    assert_eq!(played(&batch), played(&split));
}

#[test]
fn an_outcome_that_holds_for_one_minute_of_a_batch_is_reached_as_any_split_reaches_it() {
    // An ending that holds only at 14:00, once the thaw (day 2, 14:00) has come.
    let mut world = marches();
    world.world.outcomes.push(RouteOutcome {
        id: "thaw_seen".into(),
        name: "thaw_seen".into(),
        text: "thaw_seen".into(),
        when: Condition::All {
            of: vec![
                Condition::Flag {
                    flag: "thaw".into(),
                },
                Condition::TimeOfDay { from: 840, to: 841 },
            ],
        },
    });
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut once = Engine::new_with_seed(&world, 7).unwrap();
    let mut split = Engine::new_with_seed(&world, 7).unwrap();
    let long = happenings(once.execute(PassTime(1_801)).unwrap());
    let mut parts = happenings(split.execute(PassTime(1_800)).unwrap());
    parts.extend(happenings(split.execute(PassTime(1)).unwrap()));
    let reached = Event::OutcomeReached {
        outcome: "thaw_seen".into(),
    };
    assert!(long.contains(&reached));
    assert_eq!(long, parts);
    assert_eq!(once.state().outcome.as_deref(), Some("thaw_seen"));
    assert_eq!(played(&once), played(&split));
}

#[test]
fn readings_unlocked_a_minute_apart_are_each_reported_however_time_is_batched() {
    // Two flags at 06:01 and 06:02, each unlocking a further reading of the
    // wheel ruts, which the player already holds.
    let mut world = caravan_trail();
    for (minute, flag) in [(361, "first_look"), (362, "second_look")] {
        world.world.flags.push(flag.into());
        world.world.events.push(
            serde_json::from_value(serde_json::json!({
                "id": flag, "schedule": { "at": minute },
                "effects": [{ "kind": "set_flag", "flag": flag }]
            }))
            .unwrap(),
        );
        let ruts = world
            .world
            .evidence
            .iter_mut()
            .find(|e| e.id == "wheel_ruts")
            .unwrap();
        ruts.interpretations.push(
            serde_json::from_value(serde_json::json!({
                "when": { "kind": "flag", "flag": flag }, "text": flag
            }))
            .unwrap(),
        );
    }
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut snapshot = Engine::start(&world, 7, &["caravan_guard".into()])
        .unwrap()
        .snapshot();
    snapshot.state.evidence.insert("wheel_ruts".into());
    let mut once = Engine::restore(&world, snapshot.clone()).unwrap();
    let mut split = Engine::restore(&world, snapshot).unwrap();
    let long = happenings(once.execute(PassTime(2)).unwrap());
    let parts = pass_in_steps(&mut split, 2, 1);
    let readings = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::EvidenceReinterpreted { .. }))
            .count()
    };
    assert_eq!(readings(&parts), 2);
    assert_eq!(long, parts);
    assert_eq!(played(&once), played(&split));
}

#[test]
fn chained_breakthroughs_open_at_the_minute_they_hold_however_time_is_batched() {
    // Both arts wait at a sealed rank. Cloud Palm's opens with a flag at
    // minute 1; Azure Breath's needs Cloud Palm's and the minute 1 to 2, and
    // is promoted first, by ID, so only a second look at that minute opens it.
    let mut world = sect();
    world.world.time = Some(WorldTime {
        start: 0,
        clock: TextTemplate("Day {day}, {hour}:{minute}".into()),
        wait: Some(30),
        rest: Some(480),
        calendar: None,
    });
    world.world.flags.push("palm_open".into());
    world.world.events.push(
        serde_json::from_value(serde_json::json!({
            "id": "palm_open", "schedule": { "at": 1 },
            "effects": [{ "kind": "set_flag", "flag": "palm_open" }]
        }))
        .unwrap(),
    );
    let combat = world.world.combat.as_mut().unwrap();
    combat.player_techniques[0].xp = 40;
    combat.player_techniques.push(
        serde_json::from_value(serde_json::json!({ "technique": "cloud_palm", "xp": 40 })).unwrap(),
    );
    let gate = |id: &str| -> Condition {
        serde_json::from_value(match id {
            "azure_breath" => serde_json::json!({ "kind": "all", "of": [
                { "kind": "technique", "technique": "cloud_palm", "rank": 2 },
                { "kind": "time_of_day", "from": 1, "to": 2 }
            ]}),
            _ => serde_json::json!({ "kind": "flag", "flag": "palm_open" }),
        })
        .unwrap()
    };
    for technique in &mut combat.techniques {
        let sealed = technique.ranks.len() - 1;
        technique.ranks[sealed].requires = Some(gate(&technique.id));
    }
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let mut once = Engine::new(&world).unwrap();
    let mut split = Engine::new(&world).unwrap();
    let long = happenings(once.execute(PassTime(2)).unwrap());
    let parts = pass_in_steps(&mut split, 2, 1);
    let ranks = |engine: &Engine<'_>| {
        let learned = &combat_of(engine).techniques;
        (learned["azure_breath"].rank, learned["cloud_palm"].rank)
    };
    assert_eq!(ranks(&split), (3, 2), "both opened at minute 1");
    assert_eq!(ranks(&once), ranks(&split));
    assert_eq!(long, parts);
    assert_eq!(played(&once), played(&split));
}

fn combat_of<'e>(engine: &'e Engine<'_>) -> &'e CombatState {
    engine.state().combat.as_ref().unwrap()
}

fn wenna<'e>(engine: &'e Engine<'_>) -> &'e str {
    &engine.state().whereabouts["wenna"]
}

#[test]
fn time_is_never_offered_and_a_refused_pass_changes_nothing() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(!engine
        .actions()
        .iter()
        .any(|a| matches!(a.command, PassTime(_))));
    let before = engine.state().clone();
    for minutes in [0, DURATION_BOUND + 1, u64::MAX] {
        assert!(matches!(
            engine.execute(PassTime(minutes)),
            Err(EngineError::InvalidDuration)
        ));
    }
    assert_eq!(engine.state(), &before);
    engine.execute(PassTime(DURATION_BOUND)).unwrap();
    assert_eq!(now(&engine), 480 + DURATION_BOUND);
    // At the end of time, a pass past it is refused whole.
    let mut ending = marches();
    unscheduled(&mut ending);
    ending.world.time.as_mut().unwrap().start = WORLD_TIME_BOUND - 10;
    let mut engine = Engine::new_with_seed(&ending, 7).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(PassTime(11)),
        Err(EngineError::NumericLimit)
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(PassTime(10)).unwrap();
    assert_eq!(now(&engine), WORLD_TIME_BOUND);
}

#[test]
fn a_world_without_a_clock_never_passes_time() {
    let world = demo();
    let mut engine = Engine::new(&world).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(PassTime(1)),
        Err(EngineError::NoWorldTime)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn no_fight_lets_world_time_pass() {
    // The turn-based timeline.
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Move(Direction::North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    refused_in_a_fight(&mut engine);
    // A real-time duel on its own clock, in a world with world time.
    let world = duel_yard();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Duel("brigand".into())).unwrap();
    refused_in_a_fight(&mut engine);
    // A mass battle.
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let mut snapshot = engine.snapshot();
    snapshot
        .state
        .retinue
        .as_mut()
        .unwrap()
        .roster
        .entry("levy".into())
        .or_default()
        .insert(
            1,
            Squad {
                healthy: 6,
                wounded: 0,
                xp: 0,
            },
        );
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    engine.execute(Engage("outlaws".into())).unwrap();
    assert!(engine.battle().is_some());
    refused_in_a_fight(&mut engine);
}

fn refused_in_a_fight(engine: &mut Engine<'_>) {
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(PassTime(1)),
        Err(EngineError::InEncounter)
    ));
    assert_eq!(engine.state(), &before);
}

fn duel_yard() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/practice-yard"
    ))
    .unwrap()
}

/// The duel world with a clock, MP that recovers a quarter every hour and
/// Rage that decays a point every 30 minutes, for a player who uses both.
fn recovering() -> WorldSpec {
    let mut world = duel();
    world.world.time = Some(WorldTime {
        start: 0,
        clock: TextTemplate("Day {day}, {hour}:{minute}".into()),
        wait: Some(30),
        rest: Some(480),
        calendar: None,
    });
    let combat = world.world.combat.as_mut().unwrap();
    combat.resources.rage_cap = Some(10);
    combat.resources.rage_carries = true;
    combat.resources.mp_recovery = Some(Recovery {
        percent: 25,
        every: 60,
    });
    combat.resources.rage_decay = Some(Decay {
        points: 1,
        every: 30,
    });
    combat.player_resources = Some(ResourceUse {
        mp: true,
        rage: true,
    });
    world
}

#[test]
fn mp_recovers_and_rage_decays_on_the_minute_however_time_is_batched() {
    let world = recovering();
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
    let start = Engine::new(&world).unwrap();
    let mut snapshot = start.snapshot();
    let vitals = &mut snapshot.state.combat.as_mut().unwrap().vitals;
    vitals.mp = 0;
    vitals.rage = 9;
    let mut once = Engine::restore(&world, snapshot.clone()).unwrap();
    let mut minutes = Engine::restore(&world, snapshot.clone()).unwrap();
    let mut waited = Engine::restore(&world, snapshot).unwrap();
    once.execute(PassTime(150)).unwrap();
    pass_in_steps(&mut minutes, 150, 1);
    waited.execute(Wait(150)).unwrap();
    let after = vitals_of(&once);
    assert_eq!(vitals_of(&minutes), after);
    assert_eq!(vitals_of(&waited), after);
    // Two hours crossed recover two quarters; five half-hours, five points.
    let max = once.player_pools().unwrap().mp.unwrap();
    assert_eq!(after.mp, max / 2);
    assert_eq!(after.rage, 4);
}

fn vitals_of(engine: &Engine<'_>) -> Vitals {
    engine.state().combat.as_ref().unwrap().vitals
}

#[test]
fn midnights_and_month_ends_settle_alike_in_any_batch() {
    let mut world = marches();
    world.world.time.as_mut().unwrap().calendar = Some(Calendar {
        epoch: GregorianDate {
            year: 1200,
            month: 1,
            day: 30,
        },
    });
    let economy = world.world.economy.as_mut().unwrap();
    economy.currency.start = 1_000;
    economy.banking = Some(Banking {
        branches: vec!["greyford".into()],
        monthly_interest_basis_points: 100,
    });
    let mut once = Engine::new_with_seed(&world, 7).unwrap();
    let mut hours = Engine::new_with_seed(&world, 7).unwrap();
    once.execute(Deposit(500)).unwrap();
    hours.execute(Deposit(500)).unwrap();
    let long = happenings(once.execute(PassTime(3 * MINUTES_PER_DAY)).unwrap());
    assert!(long.iter().any(|e| matches!(e, Event::MonthEnded { .. })));
    assert_eq!(pass_in_steps(&mut hours, 3 * MINUTES_PER_DAY, 60), long);
    assert_eq!(played(&hours), played(&once));
}

#[test]
fn passed_time_saves_restores_and_replays_over_the_wire() {
    let world = marches();
    let commands = [
        PassTime(30),
        Talk("wenna".into()),
        PassTime(90),
        PassTime(1),
    ];
    let wire: Vec<Command> =
        serde_json::from_str(&serde_json::to_string(&commands).unwrap()).unwrap();
    assert_eq!(wire, commands);
    assert_eq!(
        serde_json::to_string(&PassTime(3)).unwrap(),
        r#"{"pass_time":3}"#
    );
    let mut direct = Engine::new_with_seed(&world, 7).unwrap();
    for command in &commands {
        direct.execute(command.clone()).unwrap();
    }
    let mut halfway = Engine::new_with_seed(&world, 7).unwrap();
    for command in &commands[..2] {
        halfway.execute(command.clone()).unwrap();
    }
    let bytes = halfway.snapshot().to_json();
    let mut restored = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    for command in &commands[2..] {
        restored.execute(command.clone()).unwrap();
    }
    assert_eq!(restored.state(), direct.state());
}
