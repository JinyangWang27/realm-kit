//! The player's stat points and resting; effective stats and vitals are the
//! character core's.

use super::*;
use crate::character::{self, resources};

/// Points granted by every level reached, minus those spent.
pub(crate) fn granted_points(world: &WorldSpec, level: usize) -> u64 {
    let levels = &world.combat().unwrap().levels;
    levels[..level].iter().map(|l| u64::from(l.points)).sum()
}

pub(crate) fn unspent_points(world: &WorldSpec, combat: &CombatState) -> u32 {
    let spent: u64 = combat.allocation.values().map(|p| u64::from(*p)).sum();
    u32::try_from(granted_points(world, combat.level).saturating_sub(spent)).unwrap_or(u32::MAX)
}

/// Spends points on one stat; the gained maximum HP or MP is gained now too.
pub(crate) fn allocate(
    world: &WorldSpec,
    state: &mut GameState,
    stat: Stat,
    points: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = world.combat().ok_or(EngineError::NoSuchStat)?;
    let spec = rules.stat_points.as_ref().ok_or(EngineError::NoSuchStat)?;
    let value = *spec.values.get(&stat).ok_or(EngineError::NoSuchStat)?;
    let combat = state.combat.as_mut().unwrap();
    if points == 0 || points > unspent_points(world, combat) {
        return Err(EngineError::NotEnoughPoints);
    }
    let spent = combat.allocation.get(&stat).copied().unwrap_or(0);
    let total = spent
        .checked_add(points)
        .ok_or(EngineError::NotEnoughPoints)?;
    if spec.caps.get(&stat).is_some_and(|cap| total > *cap) {
        return Err(EngineError::PointCap);
    }
    combat.allocation.insert(stat, total);
    let gain = points.saturating_mul(value);
    // Points are spent only while exploring, so the gain is never mid-fight.
    let uses_mp = resources::player_pools(world, &player_stats(world, combat))
        .mp
        .is_some();
    let vitals = &mut combat.vitals;
    match stat {
        Stat::Hp => vitals.hp = vitals.hp.saturating_add(gain),
        Stat::Mp if uses_mp => vitals.mp = vitals.mp.saturating_add(gain),
        _ => {}
    }
    events.push(Event::PointsAllocated { stat, points });
    Ok(())
}

/// Refunds every point where the world allows it; HP and MP keep what fits.
pub(crate) fn respec(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let allowed = world
        .combat()
        .and_then(|c| c.stat_points.as_ref())
        .is_some_and(|p| p.respec == Respec::Safe);
    let safe = world.location(&state.player.location).unwrap().safe;
    if !allowed || !safe {
        return Err(EngineError::NoRespec);
    }
    state.combat.as_mut().unwrap().allocation.clear();
    clamp_vitals(world, state);
    events.push(Event::PointsRefunded);
    Ok(())
}

/// Restores HP and MP at a safe location, the party's included, then the
/// world's rest time passes.
pub(crate) fn rest(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let safe = world.location(&state.player.location).unwrap().safe;
    let Some(combat) = state.combat.as_mut().filter(|_| safe) else {
        return Err(EngineError::NotSafe);
    };
    character::restore(world, combat);
    party::rest(world, state);
    state.dialogue = None;
    events.push(Event::Rested);
    if let Some(retinue) = state.retinue.as_mut() {
        retinue::mend(retinue, None, events);
    }
    // Resting takes the world's authored time, if it keeps one.
    let minutes = world.world.time.as_ref().and_then(|t| t.rest).unwrap_or(0);
    time::advance(world, state, minutes, events)
}
