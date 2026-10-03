//! The overland map a client draws.

use super::*;

pub(crate) fn map_view(world: &WorldSpec, state: &GameState) -> Option<MapView> {
    // Validation guarantees every place has a position or none does.
    let places = world
        .locations
        .iter()
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
        .map(|r| MapRoad {
            road: r.id.clone(),
            between: r.between.clone(),
            minutes: r.minutes,
        })
        .collect();
    let exits = world
        .locations
        .iter()
        .flat_map(|l| {
            l.exits.iter().map(|(direction, exit)| MapExit {
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
