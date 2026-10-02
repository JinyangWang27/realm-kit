//! Proficiencies: ranks the player trains with points from levels or is
//! taught by effects, which the capability that defines each one uses.

use super::*;

/// The player's rank; 0 if never gained.
pub(crate) fn rank(state: &GameState, proficiency: Proficiency) -> u32 {
    state
        .proficiencies
        .get(&proficiency)
        .map_or(0, ProficiencyState::rank)
}

/// Proficiency points granted by every level up to `level`.
pub(crate) fn granted_points(world: &WorldSpec, level: usize) -> u64 {
    world.combat().map_or(0, |c| {
        c.levels[..level]
            .iter()
            .map(|l| u64::from(l.proficiency_points))
            .sum()
    })
}

/// Points granted so far, minus those trained; none without combat.
pub(crate) fn unspent_proficiency_points(world: &WorldSpec, state: &GameState) -> u32 {
    let level = state.combat.as_ref().map_or(0, |c| c.level);
    let spent: u64 = state
        .proficiencies
        .values()
        .map(|p| u64::from(p.trained))
        .sum();
    u32::try_from(granted_points(world, level).saturating_sub(spent)).unwrap_or(u32::MAX)
}

/// Spends unspent points on one proficiency, up to its top rank.
pub(crate) fn train(
    world: &WorldSpec,
    state: &mut GameState,
    proficiency: Proficiency,
    points: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let max = world
        .proficiency_max(proficiency)
        .ok_or(EngineError::NoSuchProficiency)?;
    if points == 0 || points > unspent_proficiency_points(world, state) {
        return Err(EngineError::NotEnoughPoints);
    }
    let entry = state.proficiencies.entry(proficiency).or_default();
    if u64::from(entry.rank()) + u64::from(points) > u64::from(max) {
        return Err(EngineError::ProficiencyCap);
    }
    entry.trained += points;
    events.push(Event::ProficiencyTrained {
        proficiency,
        rank: entry.rank(),
    });
    Ok(())
}

/// An effect teaches ranks without spending points, up to the top rank.
pub(crate) fn raise(
    world: &WorldSpec,
    state: &mut GameState,
    proficiency: Proficiency,
    ranks: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let max = world.proficiency_max(proficiency).unwrap();
    let entry = state.proficiencies.entry(proficiency).or_default();
    if u64::from(entry.rank()) + u64::from(ranks) > u64::from(max) {
        return Err(EngineError::ProficiencyCap);
    }
    entry.taught += ranks;
    events.push(Event::ProficiencyRaised {
        proficiency,
        rank: entry.rank(),
    });
    Ok(())
}
