//! The overland map a client draws.

use super::*;

/// Whether the player knows of a place: it is where they are, or its
/// authored condition holds.
pub(crate) fn known(world: &WorldSpec, state: &GameState, location: &str) -> bool {
    state.player.location == location
        || world
            .location(location)
            .is_some_and(|l| allowed(state, l.known_when.as_ref()))
}

pub(crate) fn map_view(world: &WorldSpec, state: &GameState) -> Option<MapView> {
    // Validation guarantees every place has a position or none does.
    let places = world
        .locations
        .iter()
        .filter(|l| known(world, state, &l.id))
        .map(|l| {
            Some(MapPlace {
                location: l.id.clone(),
                point: l.map?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let roads = world
        .world
        .roads
        .iter()
        .filter(|r| r.between.iter().all(|end| known(world, state, end)))
        .map(|r| MapRoad {
            road: r.id.clone(),
            between: r.between.clone(),
            minutes: r.minutes,
        })
        .collect();
    let exits = world
        .locations
        .iter()
        .filter(|l| known(world, state, &l.id))
        .flat_map(|l| {
            let exits = l.exits.iter();
            let exits = exits.filter(|(_, exit)| known(world, state, &exit.destination));
            exits.map(|(direction, exit)| MapExit {
                from: l.id.clone(),
                to: exit.destination.clone(),
                direction: *direction,
            })
        })
        .collect();
    Some(MapView {
        here: state.player.location.clone(),
        places,
        roads,
        exits,
    })
}
