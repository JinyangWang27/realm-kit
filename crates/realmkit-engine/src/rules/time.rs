//! World time: travel and waiting move it on, and every scheduled
//! occurrence it crosses happens in order.

use super::*;
use realmkit_spec::{Schedule, DURATION_BOUND, WORLD_TIME_BOUND};

/// Something that happens on a schedule. Occurrences at the same minute go
/// in this order: authored events, characters who move, the price tick, the
/// retinue's upkeep, then recruiting pools refilling in location order.
enum Due<'w> {
    Event(&'w realmkit_spec::WorldEvent),
    Mover(&'w Character),
    PriceTick,
    Upkeep,
    Refill(&'w realmkit_spec::Location),
}

fn schedules(world: &WorldSpec) -> Vec<(Schedule, Due<'_>)> {
    let events = world
        .world
        .events
        .iter()
        .map(|e| (e.schedule, Due::Event(e)));
    let movers = world
        .characters
        .iter()
        .filter_map(|c| Some((c.moves.as_ref()?.schedule, Due::Mover(c))));
    let prices = world
        .economy()
        .and_then(|e| e.tick)
        .map(|t| (t.schedule, Due::PriceTick));
    let upkeep = world
        .troops()
        .and_then(|t| t.upkeep)
        .map(|u| (u.schedule, Due::Upkeep));
    let refills = world.locations.iter().filter_map(|l| {
        let refill = l.recruits.as_ref()?.refill?;
        Some((refill.schedule, Due::Refill(l)))
    });
    events
        .chain(movers)
        .chain(prices)
        .chain(upkeep)
        .chain(refills)
        .collect()
}

/// Moves world time on by `minutes` while the player stays where they are,
/// resolving every occurrence it crosses in chronological order, ties in
/// schedule order. A world without a clock ignores it.
pub(crate) fn advance(
    world: &WorldSpec,
    state: &mut GameState,
    minutes: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    pass(world, state, minutes, events, true)
}

/// [`advance`], with the player at their location (`present`) or on the
/// road, where nobody's comings and goings are seen.
fn pass(
    world: &WorldSpec,
    state: &mut GameState,
    minutes: u64,
    events: &mut Vec<Event>,
    present: bool,
) -> Result<(), EngineError> {
    let Some(start) = state.time else {
        return Ok(());
    };
    let end = start
        .checked_add(minutes)
        .filter(|end| *end <= WORLD_TIME_BOUND)
        .ok_or(EngineError::NumericLimit)?;
    let due = schedules(world);
    let before = events.len();
    // Everything up to (minute, index) has happened; at `start`, all of it.
    let mut cursor = (start, usize::MAX);
    loop {
        let next = due
            .iter()
            .enumerate()
            .filter_map(|(index, (schedule, _))| {
                let minute = schedule.next(cursor.0, index > cursor.1)?;
                Some((minute, index))
            })
            .filter(|(minute, _)| *minute <= end)
            .min();
        let Some((minute, index)) = next else {
            break;
        };
        cursor = (minute, index);
        state.time = Some(minute);
        match due[index].1 {
            Due::Event(event) => {
                if allowed(state, event.requires.as_ref()) {
                    occur(world, state, &event.effects, events);
                }
            }
            Due::Mover(character) => relocate(state, character, events, present),
            Due::PriceTick => economy::tick(world, state),
            Due::Upkeep => retinue::upkeep(world, state, events),
            Due::Refill(location) => retinue::refill(world, state, &location.id),
        }
    }
    state.time = Some(end);
    if minutes > 0 {
        events.push(Event::TimePassed {
            minutes,
            now: end,
            // Setting a flag is bookkeeping; the player sees nothing.
            eventful: events[before..]
                .iter()
                .any(|e| !matches!(e, Event::StoryFlagSet { .. })),
        });
    }
    Ok(())
}

/// Applies one occurrence's effects as a whole, or not at all: an occurrence
/// that cannot happen (a grant past a bound) is skipped, so it never holds
/// time back for every later command.
fn occur(
    world: &WorldSpec,
    state: &mut GameState,
    effects: &[realmkit_spec::Effect],
    events: &mut Vec<Event>,
) {
    // Setting flags cannot fail, so only fallible grants need a staged copy.
    if effects
        .iter()
        .all(|e| matches!(e, realmkit_spec::Effect::SetFlag { .. }))
    {
        story::apply(world, state, effects, events).expect("setting flags cannot fail");
        return;
    }
    // ponytail: one state copy per granting occurrence, and `pass` rescans
    // every schedule per occurrence; a per-minute grant over a 30-day wait
    // costs about 33 ms in release. Use a min-heap of next occurrences and a
    // pre-check or undo for grants if worlds need many frequent events.
    let mut staged = state.clone();
    let mut happened = Vec::new();
    if story::apply(world, &mut staged, effects, &mut happened).is_ok() {
        *state = staged;
        events.extend(happened);
    }
}

/// Draws where a mover goes next from the world stream, and reports it if
/// it comes to or leaves the player standing there.
fn relocate(state: &mut GameState, character: &Character, events: &mut Vec<Event>, present: bool) {
    let among = &character.moves.as_ref().unwrap().among;
    let stream = state.rng.as_mut().unwrap().world.as_mut().unwrap();
    let to = &among[rng::below(stream, among.len() as u64) as usize];
    let from = state
        .whereabouts
        .insert(character.id.clone(), to.clone())
        .unwrap();
    let here = &state.player.location;
    // A character absent under its conditions comes and goes unseen.
    if !present || from == *to || !allowed(state, character.requires.as_ref()) {
        return;
    }
    if from == *here {
        events.push(Event::CharacterLeft {
            character: character.id.clone(),
        });
    } else if to == here {
        events.push(Event::CharacterArrived {
            character: character.id.clone(),
        });
    }
}

/// Takes the road to `to`: arrives, then its travel time passes.
pub(crate) fn travel(
    world: &WorldSpec,
    state: &mut GameState,
    to: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let road = world
        .road(&state.player.location, &to)
        .ok_or_else(|| EngineError::NoRoad(to.clone()))?;
    if !allowed(state, road.requires.as_ref()) {
        return Err(EngineError::RoadBlocked {
            road: road.id.clone(),
        });
    }
    let from = std::mem::replace(&mut state.player.location, to.clone());
    state.dialogue = None;
    events.push(Event::Moved {
        from,
        to: to.clone(),
    });
    // On the road: the destination shows who is there on arrival.
    pass(world, state, road.minutes, events, false)?;
    events.push(Event::LocationViewed { location: to });
    Ok(())
}

pub(crate) fn wait(
    world: &WorldSpec,
    state: &mut GameState,
    minutes: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if world.world.time.as_ref().is_none_or(|t| t.wait.is_none()) {
        return Err(EngineError::NoWaiting);
    }
    if minutes == 0 || minutes > DURATION_BOUND {
        return Err(EngineError::InvalidWait);
    }
    state.dialogue = None;
    advance(world, state, minutes, events)
}
