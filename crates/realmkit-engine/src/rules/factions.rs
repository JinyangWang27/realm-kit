//! Standing on authored tracks, and war and peace between factions.

use super::*;
use realmkit_spec::faction_pair;

/// The player's value on `track`; validation keeps references sound.
pub(super) fn standing(state: &GameState, track: &str, faction: Option<&str>) -> i32 {
    state.standing[track].get(faction).unwrap()
}

/// Moves standing by `by`, clamped to the track's bounds. A change the bound
/// absorbs entirely reports nothing.
pub(super) fn change(
    world: &WorldSpec,
    state: &mut GameState,
    track: &str,
    faction: Option<&Id>,
    by: i32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let bounds = world.standing_track(track).unwrap();
    let value = state
        .standing
        .get_mut(track)
        .unwrap()
        .get_mut(faction.map(Id::as_str))
        .unwrap();
    let to = value
        .checked_add(by)
        .ok_or(EngineError::NumericLimit)?
        .clamp(bounds.min, bounds.max);
    if to != *value {
        events.push(Event::StandingChanged {
            track: track.into(),
            faction: faction.cloned(),
            by: to - *value,
            value: to,
        });
        *value = to;
    }
    Ok(())
}

/// Puts two factions at war or at peace; a pair already so changes nothing.
pub(super) fn diplomacy(state: &mut GameState, pair: &[Id; 2], war: bool, events: &mut Vec<Event>) {
    let factions = faction_pair(pair);
    let changed = if war {
        state.at_war.insert(factions.clone())
    } else {
        state.at_war.remove(&factions)
    };
    if changed {
        events.push(match war {
            true => Event::WarDeclared { factions },
            false => Event::PeaceMade { factions },
        });
    }
}

/// A track's starting values: one, or one for every faction.
pub(crate) fn initial(world: &WorldSpec, track: &realmkit_spec::StandingTrack) -> Standing {
    match track.scope {
        realmkit_spec::StandingScope::Global => Standing::Global(track.start(None)),
        realmkit_spec::StandingScope::Faction => Standing::Factions(
            world
                .world
                .factions
                .iter()
                .map(|f| (f.id.clone(), track.start(Some(&f.id))))
                .collect(),
        ),
    }
}
