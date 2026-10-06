//! Validation of world time, roads, scheduled events and movers.

mod common;

use common::*;
use realmkit_spec::*;

fn road<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Road {
    w.world.roads.iter_mut().find(|r| r.id == id).unwrap()
}

fn wenna(w: &mut WorldSpec) -> &mut Character {
    w.characters.iter_mut().find(|c| c.id == "wenna").unwrap()
}

#[test]
fn the_marches_validate_and_roundtrip() {
    let world = marches();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let json = serde_json::to_string(&world.world).unwrap();
    assert_eq!(serde_json::from_str::<World>(&json).unwrap(), world.world);
    let ashmere = world.road("ashmere", "greyford").unwrap();
    assert_eq!(ashmere.id, "greyford-ashmere");
    assert!(world.road("ashmere", "hollin_keep").is_none());
}

#[test]
fn a_schedule_finds_its_next_occurrence() {
    let daily = Schedule {
        at: 1_800,
        every: Some(1_440),
    };
    assert_eq!(daily.next(0, false), Some(1_800));
    assert_eq!(daily.next(1_800, true), Some(1_800));
    assert_eq!(daily.next(1_800, false), Some(3_240));
    assert_eq!(daily.next(3_239, false), Some(3_240));
    assert_eq!(daily.next(3_240, true), Some(3_240));
    let once = Schedule {
        at: 1_800,
        every: None,
    };
    assert_eq!(once.next(1_799, false), Some(1_800));
    assert_eq!(once.next(1_800, false), None);
    let edge = Schedule {
        at: 1,
        every: Some(u64::MAX),
    };
    assert_eq!(edge.next(5, false), None);
}

#[test]
fn time_content_is_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| w.world.time.as_mut().unwrap().clock.0 = "{hours}".into(),
            "invalid_template",
        ),
        (
            |w| w.world.time.as_mut().unwrap().wait = Some(0),
            "invalid_duration",
        ),
        (
            |w| road(w, "greyford-ashmere").minutes = DURATION_BOUND + 1,
            "invalid_duration",
        ),
        (
            |w| road(w, "greyford-ashmere").between[1] = "greyford".into(),
            "invalid_road",
        ),
        (
            |w| road(w, "greyford-ashmere").between[1] = "nowhere".into(),
            "missing_reference",
        ),
        (
            |w| road(w, "fen-causeway").blocked_text = None,
            "invalid_road",
        ),
        (
            |w| {
                let mut twin = w.world.roads[0].clone();
                twin.id = "twin".into();
                twin.between.reverse();
                w.world.roads.push(twin);
            },
            "duplicate_road",
        ),
        (|w| w.world.events[0].schedule.at = 480, "invalid_schedule"),
        (
            |w| w.world.events[0].schedule.every = Some(0),
            "invalid_schedule",
        ),
        (
            |w| {
                w.world.events[0].effects.push(Effect::AcceptQuest {
                    quest: "carry_letter".into(),
                })
            },
            "invalid_effect",
        ),
        (
            |w| {
                w.world.events[0].effects.push(Effect::SetFlag {
                    flag: "missing".into(),
                })
            },
            "missing_reference",
        ),
        (
            |w| wenna(w).moves.as_mut().unwrap().among.truncate(1),
            "invalid_mover",
        ),
        (
            |w| wenna(w).moves.as_mut().unwrap().among[0] = "hollin_keep".into(),
            "invalid_mover",
        ),
        (
            |w| w.locations[1].characters.push("wenna".into()),
            "invalid_mover",
        ),
        (
            |w| wenna(w).moves.as_mut().unwrap().among[1] = "nowhere".into(),
            "missing_reference",
        ),
        (
            |w| w.characters[2].requires = Some(Condition::TimeOfDay { from: 480, to: 480 }),
            "invalid_condition",
        ),
        (
            |w| {
                w.characters[2].requires = Some(Condition::TimeOfDay {
                    from: 0,
                    to: MINUTES_PER_DAY,
                })
            },
            "invalid_condition",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = marches();
        change(&mut world);
        assert_eq!(codes(&world), [code], "case {index}");
    }
    // Resting restores HP, so a world without combat has nothing to rest.
    let mut archive = archive();
    archive.world.time = Some(WorldTime {
        start: 0,
        clock: TextTemplate("Day {day}".into()),
        wait: None,
        rest: Some(60),
        calendar: None,
    });
    assert!(codes(&archive).contains(&"combat_disabled".to_string()));
    // A start past the bound also leaves every schedule before it.
    let mut world = marches();
    world.world.time.as_mut().unwrap().start = WORLD_TIME_BOUND + 1;
    assert!(codes(&world).contains(&"invalid_time".to_string()));
}

#[test]
fn everything_that_takes_time_needs_a_clock() {
    let mut world = marches();
    world.world.time = None;
    let codes = codes(&world);
    // Four timed roads, the thaw, two people's hours, Wenna, the price tick,
    // prosperity, restocking, workshops, the bank, the troops' upkeep and
    // Ashmere's recruits refilling.
    assert_eq!(
        codes.iter().filter(|c| *c == "time_disabled").count(),
        15,
        "{codes:?}"
    );
    // A road without minutes takes no time, so it needs no clock.
    let mut world = marches();
    world.world.time = None;
    world.world.events.clear();
    let economy = world.world.economy.as_mut().unwrap();
    economy.tick = None;
    economy.prosperity = None;
    economy.stock = None;
    economy.workshops = None;
    economy.banking = None;
    economy.markets.iter_mut().for_each(|m| m.prosperity = None);
    world.dialogues.retain(|d| d.id != "maddoc");
    let troops = world.world.troops.as_mut().unwrap();
    troops.upkeep = None;
    for line in &mut troops.lines {
        line.levels.iter_mut().for_each(|l| l.wage = None);
    }
    world.characters.retain(|c| c.moves.is_none());
    world.locations[0].characters.retain(|c| c != "wenna");
    for location in &mut world.locations {
        location.recruits.iter_mut().for_each(|r| r.refill = None);
    }
    for character in &mut world.characters {
        character.requires = None;
        if character.id == "maddoc" {
            character.dialogue = None;
        }
    }
    for road in &mut world.world.roads {
        road.minutes = 0;
    }
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
}

#[test]
fn map_positions_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| w.locations[1].map = None, "map_partial"),
        (
            |w| w.locations[1].map.as_mut().unwrap().x = MAP_BOUND + 1,
            "map_bounds",
        ),
        (|w| w.locations[1].map = w.locations[0].map, "map_duplicate"),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = marches();
        change(&mut world);
        assert_eq!(codes(&world), [code], "case {index}");
    }
    // A world may leave every place off the map.
    let mut unmapped = marches();
    for l in &mut unmapped.locations {
        l.map = None;
    }
    assert!(codes(&unmapped).is_empty());
}

fn date(year: i32, month: u8, day: u8) -> GregorianDate {
    GregorianDate { year, month, day }
}

fn from(epoch: GregorianDate) -> Calendar {
    Calendar { epoch }
}

#[test]
fn a_calendar_dates_each_minute_from_its_epoch() {
    let days = |n: u64| n * MINUTES_PER_DAY;
    let cases = [
        // Minute 0 is the epoch's midnight; the day turns at 1,440.
        (date(742, 1, 1), 0, date(742, 1, 1)),
        (date(742, 1, 1), 480, date(742, 1, 1)),
        (date(742, 1, 1), days(1) - 1, date(742, 1, 1)),
        (date(742, 1, 1), days(1), date(742, 1, 2)),
        (date(742, 1, 1), days(30), date(742, 1, 31)),
        (date(742, 1, 1), days(31), date(742, 2, 1)),
        // 742 is a common year, 744 a leap year.
        (date(742, 1, 1), days(58), date(742, 2, 28)),
        (date(742, 1, 1), days(59), date(742, 3, 1)),
        (date(744, 2, 28), days(1), date(744, 2, 29)),
        (date(744, 2, 28), days(2), date(744, 3, 1)),
        // A century is common unless it divides by 400.
        (date(1900, 2, 28), days(1), date(1900, 3, 1)),
        (date(2000, 2, 28), days(1), date(2000, 2, 29)),
        (date(742, 4, 30), days(1), date(742, 5, 1)),
        (date(742, 12, 31), days(1) + 480, date(743, 1, 1)),
        (date(-1, 12, 31), days(1), date(0, 1, 1)),
    ];
    for (index, (epoch, minute, expected)) in cases.into_iter().enumerate() {
        assert_eq!(from(epoch).date(minute), Some(expected), "case {index}");
    }
}

#[test]
fn a_calendar_finds_the_next_day_and_month_boundaries() {
    let calendar = from(date(742, 1, 1));
    let days = |n: u64| n * MINUTES_PER_DAY;
    assert_eq!(calendar.next_day(480), Some(days(1)));
    assert_eq!(calendar.next_day(days(1)), Some(days(2)));
    assert_eq!(calendar.next_month(480), Some(days(31)));
    assert_eq!(calendar.next_month(days(31) - 1), Some(days(31)));
    // Strictly after: a boundary is not its own next one.
    assert_eq!(calendar.next_month(days(31)), Some(days(31 + 28)));
    // December rolls into January of the next year.
    let december = from(date(742, 12, 15));
    assert_eq!(december.next_month(0), Some(days(17)));
    assert_eq!(december.date(days(17)), Some(date(743, 1, 1)));
    // A step from 08:00 on the first to 100 days later crosses three month
    // boundaries, found one after another.
    let (start, end) = (480, 480 + days(100));
    let crossed: Vec<_> =
        std::iter::successors(calendar.next_month(start), |&m| calendar.next_month(m))
            .take_while(|&m| m <= end)
            .collect();
    assert_eq!(crossed, [days(31), days(59), days(90)]);
    assert_eq!(calendar.date(crossed[2]), Some(date(742, 4, 1)));
}

#[test]
fn a_clock_shows_the_date_only_with_a_calendar_and_day_keeps_counting() {
    let mut time: WorldTime =
        serde_json::from_str(r#"{ "start": 480, "clock": "Day {day}, {hour}:{minute}" }"#).unwrap();
    let minute = 2 * MINUTES_PER_DAY + 485;
    let plain = [("day", "3"), ("hour", "08"), ("minute", "05")].map(|(k, v)| (k, v.to_string()));
    assert_eq!(clock_values(&time, minute), plain);
    time.calendar = Some(from(date(742, 1, 31)));
    let dated: Vec<_> = plain
        .into_iter()
        .chain(
            [("year", "742"), ("month", "02"), ("day_of_month", "02")]
                .map(|(k, v)| (k, v.to_string())),
        )
        .collect();
    assert_eq!(clock_values(&time, minute), dated);
}

#[test]
fn a_calendar_is_checked_with_stable_codes_and_belongs_to_the_rules() {
    let calendar = r#"{ "start": 480, "clock": "{year}-{month}-{day_of_month} {hour}:{minute}",
        "calendar": { "epoch": { "year": 742, "month": 1, "day": 1 } } }"#;
    let time: WorldTime = serde_json::from_str(calendar).unwrap();
    assert_eq!(time.calendar, Some(from(date(742, 1, 1))));
    assert!(serde_json::from_str::<WorldTime>(
        &calendar.replace("\"day\": 1", "\"day\": 1, \"era\": 1")
    )
    .is_err());
    let dated = |epoch: GregorianDate| {
        let mut world = marches();
        let mut time = time.clone();
        time.calendar = Some(from(epoch));
        world.world.time = Some(time);
        world
    };
    for epoch in [
        date(744, 2, 29),
        date(2000, 2, 29),
        date(-9999, 1, 1),
        date(8000, 1, 1),
    ] {
        assert!(codes(&dated(epoch)).is_empty(), "{epoch:?}");
    }
    for epoch in [
        date(742, 0, 1),
        date(742, 13, 1),
        date(742, 1, 0),
        date(742, 4, 31),
        date(742, 2, 29),
        date(1900, 2, 29),
        // Minute 1,000,000,000 would fall after 9999.
        date(8100, 1, 1),
    ] {
        assert_eq!(codes(&dated(epoch)), ["invalid_calendar"], "{epoch:?}");
    }
    // The clock alone; the bank's own need is checked with banking.
    let mut world = dated(date(742, 1, 1));
    world.world.economy.as_mut().unwrap().banking = None;
    world.world.time.as_mut().unwrap().calendar = None;
    assert_eq!(codes(&world), ["calendar_disabled"]);
    world.world.time.as_mut().unwrap().clock.0 = "{day_of_week}".into();
    assert_eq!(codes(&world), ["invalid_template"]);
    // The epoch is a rule, so moving it changes the revision.
    let revision = dated(date(742, 1, 1)).revision();
    assert_ne!(dated(date(742, 1, 2)).revision(), revision);
    assert_ne!(marches().revision(), revision);
}
