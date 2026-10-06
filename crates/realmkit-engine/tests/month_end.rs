//! The bank and the Gregorian month end: deposits and withdrawals at a
//! branch, interest on the average daily closing balance, and month-end
//! wages for the roster standing then.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

const DAY: u64 = MINUTES_PER_DAY;
/// Play starts at 08:00 on the epoch's day.
const START: u64 = 480;

fn date(year: i32, month: u8, day: u8) -> GregorianDate {
    GregorianDate { year, month, day }
}

/// The marches dated from `epoch`, with `silver` to start and one bank, in
/// Greyford where play starts, paying `basis_points` a month.
fn banked(epoch: GregorianDate, basis_points: u32, silver: u64) -> WorldSpec {
    let mut world = marches();
    world.world.time.as_mut().unwrap().calendar = Some(Calendar { epoch });
    let economy = world.world.economy.as_mut().unwrap();
    economy.currency.start = silver;
    economy.banking = Some(Banking {
        branches: vec!["greyford".into()],
        monthly_interest_basis_points: basis_points,
    });
    world
}

/// The marches without their bank.
fn unbanked() -> WorldSpec {
    let mut world = marches();
    world.world.economy.as_mut().unwrap().banking = None;
    world
}

/// [`banked`], with wages paid at each month end instead of each upkeep,
/// and a fifth of each squad leaving when they go unpaid. The daily upkeep
/// stays, to mend the wounded.
fn monthly(epoch: GregorianDate, basis_points: u32, silver: u64) -> WorldSpec {
    let mut world = banked(epoch, basis_points, silver);
    let troops = world.world.troops.as_mut().unwrap();
    troops.payroll = Payroll::Monthly { desert_percent: 20 };
    troops.upkeep.as_mut().unwrap().desert_percent = 0;
    world
}

fn now(engine: &Engine<'_>) -> u64 {
    engine.state().time.unwrap()
}

fn bank(engine: &Engine<'_>) -> BankAccount {
    engine.state().economy.as_ref().unwrap().bank.unwrap()
}

/// Waits until `minute`, in waits as long as allowed.
fn wait_until(engine: &mut Engine<'_>, minute: u64) -> Vec<Event> {
    let mut events = Vec::new();
    while now(engine) < minute {
        let step = (minute - now(engine)).min(DURATION_BOUND);
        events.extend(engine.execute(Wait(step)).unwrap());
    }
    events
}

/// The first first-of-a-month midnight after `minute`.
fn month_end(world: &WorldSpec, minute: u64) -> u64 {
    let calendar = world.world.time.as_ref().unwrap().calendar.unwrap();
    calendar.next_month(minute).unwrap()
}

/// What the month end at `minute` credited: the average and the interest.
fn interest(events: &[Event]) -> Option<(u64, u64)> {
    events.iter().find_map(|e| match e {
        Event::InterestCredited {
            average, amount, ..
        } => Some((*average, *amount)),
        _ => None,
    })
}

/// One month from 08:00 on its first day: each command runs at 08:00 on
/// its day of the month, in order, and the month's interest is returned.
fn month(epoch: GregorianDate, basis_points: u32, moves: &[(u64, Command)]) -> Option<(u64, u64)> {
    let world = banked(epoch, basis_points, 10_000);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    for (day, command) in moves {
        wait_until(&mut engine, (day - 1) * DAY + START);
        engine.execute(command.clone()).unwrap();
    }
    let end = month_end(&world, START);
    let events = wait_until(&mut engine, end);
    let ended = epoch;
    assert!(events.contains(&Event::MonthEnded {
        year: ended.year,
        month: ended.month,
    }));
    // The month's sum starts again.
    assert_eq!(bank(&engine).month_to_date, 0);
    interest(&events)
}

#[test]
fn deposits_and_withdrawals_move_money_at_a_branch_in_no_time() {
    let world = banked(date(742, 1, 1), 25, 100);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(offered(&engine).contains(&(Deposit(100), true)));
    assert_eq!(
        engine.execute(Deposit(60)).unwrap(),
        [Event::Deposited { amount: 60 }]
    );
    assert_eq!((currency(&engine), bank(&engine).balance), (40, 60));
    assert_eq!(
        engine.execute(Withdraw(25)).unwrap(),
        [Event::Withdrawn { amount: 25 }]
    );
    assert_eq!((currency(&engine), bank(&engine).balance), (65, 35));
    assert_eq!(now(&engine), START);
    let offers = offered(&engine);
    assert!(offers.contains(&(Deposit(65), true)));
    assert!(offers.contains(&(Withdraw(35), true)));
    // Ashmere has no branch: the account stays put and nothing is offered.
    engine.execute(Travel("ashmere".into())).unwrap();
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| matches!(c, Deposit(_) | Withdraw(_))));
    for command in [Deposit(1), Withdraw(1)] {
        assert!(matches!(engine.execute(command), Err(EngineError::NoBank)));
    }
    // A world without banking has no account and no bank anywhere.
    let world = unbanked();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(engine.state().economy.as_ref().unwrap().bank, None);
    assert!(matches!(
        engine.execute(Deposit(1)),
        Err(EngineError::NoBank)
    ));
}

#[test]
fn refused_transfers_change_nothing() {
    let world = banked(date(742, 1, 1), 25, 100);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Deposit(30)).unwrap();
    let before = engine.state().clone();
    for (command, refused) in [
        (Deposit(0), "InvalidAmount"),
        (Withdraw(0), "InvalidAmount"),
        (Deposit(CURRENCY_BOUND + 1), "InvalidAmount"),
        (Deposit(71), "NotEnoughCurrency"),
        (Withdraw(31), "NotEnoughDeposit"),
    ] {
        let error = engine.execute(command).unwrap_err();
        assert!(format!("{error:?}").starts_with(refused), "{error:?}");
        assert_eq!(engine.state(), &before);
    }
    // Neither side may pass the currency bound.
    let mut snapshot = engine.snapshot();
    let wallet = snapshot.state.economy.as_mut().unwrap();
    wallet.currency = CURRENCY_BOUND;
    wallet.bank.as_mut().unwrap().balance = CURRENCY_BOUND;
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    let before = engine.state().clone();
    assert!(offered(&engine).contains(&(Deposit(CURRENCY_BOUND), false)));
    assert!(offered(&engine).contains(&(Withdraw(CURRENCY_BOUND), false)));
    for command in [Deposit(1), Withdraw(1)] {
        assert!(matches!(
            engine.execute(command),
            Err(EngineError::NumericLimit)
        ));
        assert_eq!(engine.state(), &before);
    }
}

#[test]
fn interest_follows_the_average_of_every_daily_close() {
    // At 100 percent the interest is the average itself: January has 31 days.
    let january = date(742, 1, 1);
    let full = 10_000;
    assert_eq!(month(january, full, &[]), None);
    // From the first day, every close holds it.
    assert_eq!(
        month(january, full, &[(1, Deposit(3_100))]),
        Some((3_100, 3_100))
    );
    // Halfway, from the 16th: 16 closes of 31.
    assert_eq!(
        month(january, full, &[(16, Deposit(3_100))]),
        Some((1_600, 1_600))
    );
    // On the last day, one close.
    assert_eq!(
        month(january, full, &[(31, Deposit(3_100))]),
        Some((100, 100))
    );
    // Out from the 11th to the 20th: closes 1 to 10 and 21 to 31.
    let moves = [
        (1, Deposit(3_100)),
        (11, Withdraw(3_100)),
        (21, Deposit(3_100)),
    ];
    assert_eq!(month(january, full, &moves), Some((2_100, 2_100)));
    // Only a day's close counts, however the day went.
    let moves = [(5, Deposit(3_100)), (5, Withdraw(3_100))];
    assert_eq!(month(january, full, &moves), None);
    // February in a common year has 28 days, in a leap year 29; April 30.
    for (epoch, days) in [
        (date(742, 2, 1), 28),
        (date(744, 2, 1), 29),
        (date(1900, 2, 1), 28),
        (date(2000, 2, 1), 29),
        (date(742, 4, 1), 30),
    ] {
        let average = 10 * 1_000 / days;
        assert_eq!(
            month(epoch, full, &[(days - 9, Deposit(1_000))]),
            Some((average, average)),
            "{epoch:?}"
        );
    }
    // The rate is in basis points, rounded down once: 25 bp on an average
    // of 16 × 9,999 ÷ 31 is 16 × 9,999 × 25 ÷ 310,000 = 12.9.
    assert_eq!(
        month(january, 25, &[(16, Deposit(9_999))]),
        Some((5_160, 12))
    );
}

#[test]
fn a_month_settles_on_its_last_close_and_a_new_midnight_belongs_to_the_next() {
    let world = banked(date(742, 1, 1), 10_000, 1_000);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let end = month_end(&world, START);
    wait_until(&mut engine, end - 1);
    engine.execute(Deposit(310)).unwrap();
    let events = engine.execute(Wait(1)).unwrap();
    assert_eq!(interest(&events), Some((10, 10)));
    // At the new month's midnight, after it settled: February's money.
    engine.execute(Deposit(280)).unwrap();
    assert_eq!(bank(&engine).month_to_date, 0);
    let events = wait_until(&mut engine, month_end(&world, end));
    assert_eq!(interest(&events), Some((600, 600)));
}

/// State and events with the bookkeeping of how they were reached left
/// out: the turn counter and when time passed.
fn outcome(engine: &Engine<'_>, events: Vec<Event>) -> (GameState, Vec<Event>) {
    let mut state = engine.state().clone();
    state.turn = 0;
    let events = events
        .into_iter()
        .filter(|e| !matches!(e, Event::TimePassed { .. }))
        .collect();
    (state, events)
}

#[test]
fn one_long_wait_settles_like_many_short_ones() {
    // From 20:00 on 31 January, 30 days cross the ends of January and
    // February, with monthly wages for a squad paid out of the bank.
    let world = monthly(date(742, 1, 1), 25, 10_000);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Deposit(9_000)).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine
        .execute(Recruit {
            line: "levy".into(),
            quantity: 5,
        })
        .unwrap();
    wait_until(&mut engine, 30 * DAY + 20 * 60);
    let start = engine.snapshot();
    let long = engine.execute(Wait(30 * DAY)).unwrap();
    let long = outcome(&engine, long);
    for step in [DAY, 7 * 60, 1] {
        let mut engine = Engine::restore(&world, start.clone()).unwrap();
        let end = now(&engine) + 30 * DAY;
        let mut events = Vec::new();
        while now(&engine) < end {
            events.extend(engine.execute(Wait(step.min(end - now(&engine)))).unwrap());
        }
        assert_eq!(outcome(&engine, events), long, "{step}");
    }
    let ended: Vec<_> = long
        .1
        .iter()
        .filter(|e| matches!(e, Event::MonthEnded { .. }))
        .collect();
    assert_eq!(
        ended,
        [
            &Event::MonthEnded {
                year: 742,
                month: 1
            },
            &Event::MonthEnded {
                year: 742,
                month: 2
            }
        ]
    );
}

#[test]
fn a_save_mid_month_earns_the_same_interest() {
    let world = banked(date(742, 3, 1), 25, 10_000);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Deposit(7_000)).unwrap();
    wait_until(&mut engine, 12 * DAY + 600);
    engine.execute(Withdraw(2_500)).unwrap();
    wait_until(&mut engine, 17 * DAY + 900);
    let bytes = engine.snapshot().to_json();
    let mut restored = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    let end = month_end(&world, START) + 3 * DAY;
    let played = wait_until(&mut engine, end);
    let resumed = wait_until(&mut restored, end);
    assert_eq!(played, resumed);
    assert_eq!(engine.state(), restored.state());
    // 12 closes of 7,000 and 19 of 4,500, at 25 bp: (84,000 + 85,500) × 25
    // ÷ 310,000 = 13.7.
    assert_eq!(interest(&played), Some((5_467, 13)));
}

/// The wages and their split at the month end in `events`.
fn wages(events: &[Event]) -> Option<(u64, u64)> {
    events.iter().find_map(|e| match e {
        Event::WagesPaid { amount, from_bank } => Some((*amount, *from_bank)),
        _ => None,
    })
}

/// In Ashmere with `count` levies recruited at 08:00 on `day` of January,
/// having banked `deposit`; `silver` covers both and the road.
fn levies(world: &WorldSpec, deposit: u64, day: u64, count: u64) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    if deposit > 0 {
        engine.execute(Deposit(deposit)).unwrap();
    }
    engine.execute(Travel("ashmere".into())).unwrap();
    wait_until(&mut engine, (day - 1) * DAY + START);
    engine
        .execute(Recruit {
            line: "levy".into(),
            quantity: count,
        })
        .unwrap();
    engine
}

#[test]
fn month_end_wages_are_for_whoever_stands_in_the_roster_then() {
    let world = monthly(date(742, 1, 1), 0, 1_000);
    let end = month_end(&world, START);
    // No wages on the daily upkeep: the month end pays all five at once,
    // from the purse, while the bank holds nothing.
    let mut engine = levies(&world, 0, 1, 5);
    let daily = wait_until(&mut engine, end - 1);
    assert_eq!(wages(&daily), None);
    let settled = engine.execute(Wait(1)).unwrap();
    assert_eq!(wages(&settled), Some((5, 0)));
    // Recruited on the last evening, a whole month's wage all the same.
    let mut engine = levies(&world, 0, 31, 5);
    let currency_before = currency(&engine);
    assert_eq!(wages(&wait_until(&mut engine, end)), Some((5, 0)));
    assert_eq!(currency(&engine), currency_before - 5);
    // Recruited at the new month's midnight, after it settled: nothing
    // until the next month end.
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    assert_eq!(wages(&wait_until(&mut engine, end)), None);
    engine
        .execute(Recruit {
            line: "levy".into(),
            quantity: 5,
        })
        .unwrap();
    let february = wait_until(&mut engine, month_end(&world, end));
    assert_eq!(wages(&february), Some((5, 0)));
}

#[test]
fn soldiers_gone_before_the_month_end_cost_nothing() {
    let world = monthly(date(742, 1, 1), 0, 1_000);
    let mut engine = levies(&world, 0, 2, 6);
    engine.execute(Engage("outlaws".into())).unwrap();
    engine.execute(Autoresolve).unwrap();
    let troops = world.troops().unwrap();
    let roster = &engine.state().retinue.as_ref().unwrap().roster;
    let due: u64 = roster
        .iter()
        .flat_map(|(line, levels)| {
            let line = troops.line(line).unwrap();
            levels
                .iter()
                .map(move |(l, s)| line.wage_at(*l) * s.heads())
        })
        .sum();
    let heads = engine.state().retinue.as_ref().unwrap().heads();
    assert!(heads < 6, "the battle should cost some levies");
    let events = wait_until(&mut engine, month_end(&world, START));
    assert_eq!(wages(&events), Some((due, 0)));
}

#[test]
fn wages_come_from_the_bank_then_the_purse_or_not_at_all() {
    let world = monthly(date(742, 1, 1), 0, 120);
    let end = month_end(&world, START);
    // Six levies cost 60, leaving 60 carried or banked.
    let mut engine = levies(&world, 60, 1, 6);
    assert_eq!(wages(&wait_until(&mut engine, end)), Some((6, 6)));
    assert_eq!((bank(&engine).balance, currency(&engine)), (54, 0));
    let mut engine = levies(&world, 4, 1, 6);
    assert_eq!(wages(&wait_until(&mut engine, end)), Some((6, 4)));
    assert_eq!((bank(&engine).balance, currency(&engine)), (0, 54));
    // Short of the whole bill: nothing is paid, and a share deserts.
    let world = monthly(date(742, 1, 1), 0, 64);
    let mut engine = levies(&world, 2, 1, 6);
    assert_eq!((bank(&engine).balance, currency(&engine)), (2, 2));
    let events = wait_until(&mut engine, end);
    assert_eq!(wages(&events), None);
    assert!(events.contains(&Event::WagesUnpaid {
        amount: 6,
        available: 4
    }));
    assert!(events.contains(&Event::Deserted {
        line: "levy".into(),
        level: 1,
        count: 1
    }));
    assert_eq!((bank(&engine).balance, currency(&engine)), (2, 2));
    assert_eq!(engine.state().retinue.as_ref().unwrap().heads(), 5);
}

#[test]
fn interest_is_credited_before_the_wages_it_helps_pay() {
    // Three silver banked all January earn three at 100 percent, which
    // with the three makes the six levies' wages.
    let world = monthly(date(742, 1, 1), 10_000, 63);
    let mut engine = levies(&world, 3, 1, 6);
    assert_eq!(currency(&engine), 0);
    let events = wait_until(&mut engine, month_end(&world, START));
    let credited = events
        .iter()
        .position(|e| matches!(e, Event::InterestCredited { amount: 3, .. }));
    let paid = events.iter().position(|e| {
        *e == Event::WagesPaid {
            amount: 6,
            from_bank: 6,
        }
    });
    assert!(credited.is_some() && credited < paid, "{events:?}");
    assert_eq!(bank(&engine).balance, 0);
}

#[test]
fn upkeep_still_mends_the_wounded_on_its_own_schedule() {
    let world = monthly(date(742, 1, 1), 0, 1_000);
    let mut snapshot = Engine::new_with_seed(&world, 7).unwrap().snapshot();
    let roster = &mut snapshot.state.retinue.as_mut().unwrap().roster;
    roster.entry("levy".into()).or_default().insert(
        1,
        Squad {
            healthy: 2,
            wounded: 3,
            xp: 0,
        },
    );
    let mut engine = Engine::restore(&world, snapshot).unwrap();
    let events = engine.execute(Wait(DAY - START)).unwrap();
    assert!(events.contains(&Event::Recovered {
        line: "levy".into(),
        level: 1,
        count: 2
    }));
    assert_eq!(wages(&events), None);
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::WagesUnpaid { .. })));
}

#[test]
fn what_happens_at_the_new_month_midnight_comes_after_the_settlement() {
    // A purse found at the first of February's midnight is too late for
    // January's wages: the month settles first.
    let mut world = monthly(date(742, 1, 1), 0, 60);
    let end = month_end(&world, START);
    world.world.events.push(WorldEvent {
        id: "purse".into(),
        schedule: Schedule {
            at: end,
            every: None,
        },
        requires: None,
        effects: vec![Effect::GrantCurrency { amount: 100 }],
    });
    let mut engine = levies(&world, 0, 1, 6);
    let events = wait_until(&mut engine, end);
    let unpaid = events
        .iter()
        .position(|e| matches!(e, Event::WagesUnpaid { .. }));
    let found = events
        .iter()
        .position(|e| *e == Event::CurrencyReceived { amount: 100 });
    assert!(unpaid.is_some() && unpaid < found, "{events:?}");
}

#[test]
fn saves_reject_bank_accounts_the_rules_could_not_produce() {
    let world = banked(date(742, 1, 1), 25, 100);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Deposit(50)).unwrap();
    wait_until(&mut engine, 3 * DAY + START);
    let good = engine.snapshot();
    // Three closes of 50 by the 4th, so at most three days' worth.
    assert_eq!(bank(&engine).month_to_date, 150);
    Engine::restore(&world, good.clone()).unwrap();
    let broken: Vec<fn(&mut EconomyState)> = vec![
        |e| e.bank = None,
        |e| e.bank.as_mut().unwrap().balance = CURRENCY_BOUND + 1,
        |e| e.bank.as_mut().unwrap().month_to_date = 3 * CURRENCY_BOUND + 1,
    ];
    for (i, breaks) in broken.into_iter().enumerate() {
        let mut snapshot = good.clone();
        breaks(snapshot.state.economy.as_mut().unwrap());
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "{i}"
        );
    }
    // On the first day of play, or of a month, no day has closed yet.
    let mut snapshot = Engine::new_with_seed(&world, 7).unwrap().snapshot();
    let bank = snapshot
        .state
        .economy
        .as_mut()
        .unwrap()
        .bank
        .as_mut()
        .unwrap();
    bank.month_to_date = 1;
    assert!(Engine::restore(&world, snapshot).is_err());
    let end = month_end(&world, START);
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    wait_until(&mut engine, end + 600);
    let mut snapshot = engine.snapshot();
    let bank = snapshot
        .state
        .economy
        .as_mut()
        .unwrap()
        .bank
        .as_mut()
        .unwrap();
    bank.month_to_date = 1;
    assert!(Engine::restore(&world, snapshot).is_err());
    // An account in a world without banking.
    let plain = unbanked();
    let mut snapshot = Engine::new_with_seed(&plain, 7).unwrap().snapshot();
    snapshot.state.economy.as_mut().unwrap().bank = Some(BankAccount::default());
    assert!(Engine::restore(&plain, snapshot).is_err());
}

#[test]
fn monthly_wages_need_no_upkeep_and_desert_by_their_own_share() {
    // Half of each squad leaves over unpaid wages, with no upkeep at all.
    let mut world = monthly(date(742, 1, 1), 0, 64);
    let troops = world.world.troops.as_mut().unwrap();
    troops.payroll = Payroll::Monthly { desert_percent: 50 };
    troops.upkeep = None;
    let end = month_end(&world, START);
    let mut engine = levies(&world, 0, 1, 6);
    let events = wait_until(&mut engine, end);
    assert!(events.contains(&Event::WagesUnpaid {
        amount: 6,
        available: 4
    }));
    assert!(events.contains(&Event::Deserted {
        line: "levy".into(),
        level: 1,
        count: 3
    }));
    assert_eq!(engine.state().retinue.as_ref().unwrap().heads(), 3);
    // Paid, with nothing on the days between and nobody mended.
    let world_paid = {
        let mut world = world.clone();
        world.world.economy.as_mut().unwrap().currency.start = 1_000;
        world
    };
    let mut snapshot = Engine::new_with_seed(&world_paid, 7).unwrap().snapshot();
    let roster = &mut snapshot.state.retinue.as_mut().unwrap().roster;
    roster.entry("levy".into()).or_default().insert(
        1,
        Squad {
            healthy: 2,
            wounded: 3,
            xp: 0,
        },
    );
    let mut engine = Engine::restore(&world_paid, snapshot).unwrap();
    let daily = wait_until(&mut engine, end - 1);
    assert!(!daily.iter().any(|e| matches!(
        e,
        Event::Recovered { .. } | Event::WagesPaid { .. } | Event::WagesUnpaid { .. }
    )));
    assert_eq!(wages(&engine.execute(Wait(1)).unwrap()), Some((5, 0)));
    assert_eq!(squad(&engine, "levy", 1).unwrap().wounded, 3);
}

#[test]
fn upkeep_beside_monthly_wages_never_takes_its_own_deserters() {
    // Upkeep's share is all of each squad, the month's a fifth: unpaid
    // wages cost one levy of six, by the month's share.
    let mut world = monthly(date(742, 1, 1), 0, 64);
    world
        .world
        .troops
        .as_mut()
        .unwrap()
        .upkeep
        .as_mut()
        .unwrap()
        .desert_percent = 100;
    let mut engine = levies(&world, 2, 1, 6);
    let events = wait_until(&mut engine, month_end(&world, START));
    let deserted: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::Deserted { .. }))
        .collect();
    assert_eq!(
        deserted,
        [&Event::Deserted {
            line: "levy".into(),
            level: 1,
            count: 1
        }]
    );
}
