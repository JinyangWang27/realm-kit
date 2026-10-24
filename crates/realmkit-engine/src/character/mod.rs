//! The universal character core: what a fighting character is, whatever runs
//! the fight. Effective stats derive from the level table, stat points,
//! technique passives (`techniques`) and worn gear (`gear`); vitals have one
//! owner per character; resources, damage and Rage follow the rules in
//! [`resources`] and [`strike`], with the arithmetic in `formulas`.
//!
//! Nothing here knows of initiative, turns, ticks or a host's frames. The
//! turn-based `encounter` layer schedules who acts when and calls these
//! rules for what an action does; world time calls them for what time does.
//! A real-time layer would call the same rules on its own clock.

use super::*;

pub(crate) mod resources;
pub(crate) mod strike;

pub use resources::Pools;

/// Effective stats: the level table plus allocated stat points plus learned
/// techniques' current rank bonuses plus worn gear. Derived whenever needed,
/// never saved, so no bonus can be counted twice.
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

/// Maxima can drop (a smaller rank bonus, removed gear), so the player's
/// current HP and MP never stay above the effective maxima, and damage-rage
/// progress, counted in maximum HP, carries its whole points over.
pub(crate) fn clamp_vitals(world: &WorldSpec, state: &mut GameState) {
    let combat = state.combat.as_mut().unwrap();
    let max = player_stats(world, combat);
    let pools = resources::player_pools(world, &max);
    let vitals = &mut combat.vitals;
    let whole = vitals.rage_remainder / u64::from(max.hp);
    vitals.rage = vitals
        .rage
        .saturating_add(whole.try_into().unwrap_or(u32::MAX));
    vitals.rage_remainder %= u64::from(max.hp);
    vitals.hp = vitals.hp.min(max.hp);
    vitals.mp = vitals.mp.min(pools.mp_max());
    vitals.rage = vitals.rage.min(pools.rage_cap());
    if pools.rage.is_none() {
        vitals.rage_remainder = 0;
    } else {
        resources::at_cap(world, vitals, pools.rage_cap());
    }
}

/// Full HP and MP at the effective maxima, as resting and levelling up
/// give; Rage is momentum, which rest does not build or clear.
pub(crate) fn restore(world: &WorldSpec, combat: &mut CombatState) {
    let max = player_stats(world, combat);
    let pools = resources::player_pools(world, &max);
    combat.vitals.hp = max.hp;
    combat.vitals.mp = pools.mp_max();
}

/// The combat profile of a non-player fighter.
pub(crate) fn profile<'w>(world: &'w WorldSpec, id: &str) -> &'w realmkit_spec::CombatProfile {
    world.character(id).unwrap().combat.as_ref().unwrap()
}
