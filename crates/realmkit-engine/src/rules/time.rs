//! World time: travel and waiting move it on, and every scheduled
//! occurrence it crosses happens in order.

use super::*;
use realmkit_spec::{Schedule, DURATION_BOUND, WORLD_TIME_BOUND};

/// Something that happens on a schedule. Occurrences at the same minute go
/// in this order: authored events, then characters who move.
enum Due<'w> {
    Event(&'w realmkit_spec::WorldEvent),
    Mover(&'w Character),
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
    events.chain(movers).collect()
}

/// Moves world time on by `minutes`, resolving every occurrence it crosses
/// in chronological order, ties in schedule order. A world without a clock
/// ignores it.
pub(crate) fn advance(
    world: &WorldSpec,
    state: &mut GameState,
    minutes: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let Some(start) = state.time else {
        return Ok(());
    };
    let end = start
        .checked_add(minutes)
        .filter(|end| *end <= WORLD_TIME_BOUND)
        .ok_or(EngineError::NumericLimit)?;
    let due = schedules(world);
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
                    story::apply(world, state, &event.effects, events)?;
                }
            }
            Due::Mover(character) => relocate(state, character, events),
        }
    }
    state.time = Some(end);
    if minutes > 0 {
        events.push(Event::TimePassed { minutes, now: end });
    }
    Ok(())
}

/// Draws where a mover goes next from the world stream, and reports it if
/// it comes to or leaves the player.
fn relocate(state: &mut GameState, character: &Character, events: &mut Vec<Event>) {
    let among = &character.moves.as_ref().unwrap().among;
    let stream = state.rng.as_mut().unwrap().world.as_mut().unwrap();
    let to = &among[rng::below(stream, among.len() as u64) as usize];
    let from = state
        .whereabouts
        .insert(character.id.clone(), to.clone())
        .unwrap();
    let here = &state.player.location;
    if from == *to {
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
    advance(world, state, road.minutes, events)?;
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
