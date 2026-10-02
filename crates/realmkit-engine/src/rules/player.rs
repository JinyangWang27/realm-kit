//! The player's effective stats, stat points, and resting.

use super::*;

/// Effective stats: the level table plus allocated stat points plus learned
/// techniques' current rank bonuses. Derived
/// whenever needed, never saved, so no bonus can be counted twice.
pub(crate) fn player_stats(world: &WorldSpec, combat: &CombatState) -> Stats {
    let rules = world.combat().unwrap();
    let mut stats = rules.levels[combat.level - 1].stats;
    if let Some(points) = &rules.stat_points {
        for (stat, spent) in &combat.allocation {
            let value = points.values.get(stat).copied().unwrap_or(0);
            let total = stats.get_mut(*stat);
            *total = total.saturating_add(spent.saturating_mul(value));
        }
    }
    techniques::add_passives(world, combat, &mut stats);
    gear::apply(world, combat, &mut stats);
    stats
}

/// Maxima can drop (a smaller rank bonus, removed gear), so current HP and
/// MP never stay above the effective maxima.
pub(crate) fn clamp_vitals(world: &WorldSpec, state: &mut GameState) {
    let combat = state.combat.as_mut().unwrap();
    let max = rules::player_stats(world, combat);
    let (hp, mp) = match &mut combat.stance {
        Stance::Exploring(vitals) => (&mut vitals.hp, &mut vitals.mp),
        Stance::Fighting(encounter) => {
            let player = &mut encounter.participants[0];
            // Rage progress is counted in maximum HP: carry whole points over.
            let whole = player.rage_remainder / u64::from(max.hp);
            player.rage = player
                .rage
                .saturating_add(whole.try_into().unwrap_or(u32::MAX));
            player.rage_remainder %= u64::from(max.hp);
            (&mut player.hp, &mut player.mp)
        }
    };
    *hp = (*hp).min(max.hp);
    *mp = (*mp).min(max.mp);
}

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
    if let Stance::Exploring(vitals) = &mut combat.stance {
        match stat {
            Stat::Hp => vitals.hp = vitals.hp.saturating_add(gain),
            Stat::Mp => vitals.mp = vitals.mp.saturating_add(gain),
            _ => {}
        }
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
    let combat = state.combat.as_mut().unwrap();
    combat.allocation.clear();
    let max = player_stats(world, combat);
    if let Stance::Exploring(vitals) = &mut combat.stance {
        vitals.hp = vitals.hp.min(max.hp);
        vitals.mp = vitals.mp.min(max.mp);
    }
    events.push(Event::PointsRefunded);
    Ok(())
}

/// Restores HP and MP at a safe location.
pub(crate) fn rest(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let safe = world.location(&state.player.location).unwrap().safe;
    let Some(combat) = state.combat.as_mut().filter(|_| safe) else {
        return Err(EngineError::NotSafe);
    };
    let stats = player_stats(world, combat);
    combat.stance = Stance::Exploring(Vitals {
        hp: stats.hp,
        mp: stats.mp,
    });
    state.dialogue = None;
    events.push(Event::Rested);
    if let Some(retinue) = state.retinue.as_mut() {
        retinue::mend(retinue, None, events);
    }
    // Resting takes the world's authored time, if it keeps one.
    let minutes = world.world.time.as_ref().and_then(|t| t.rest).unwrap_or(0);
    time::advance(world, state, minutes, events)
}
