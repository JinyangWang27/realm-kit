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
    // prosperity, restocking, workshops, the troops' upkeep and Ashmere's
    // recruits refilling.
    assert_eq!(
        codes.iter().filter(|c| *c == "time_disabled").count(),
        14,
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
