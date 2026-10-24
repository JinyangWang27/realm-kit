//! MP and Rage: which resources a character uses, paying for skills, Rage
//! from fighting, and what elapsed time does to both. MP is a reserve that
//! time refills; Rage is momentum that fighting builds and time drains.
//! Each clock (the turn-based timeline, world time) calls these rules for its
//! own span, and the two never overlap: world time stands still during a
//! fight, and encounter time exists only inside one.

use super::*;
use realmkit_spec::{CombatProfile, ResourceUse};

/// The resources a character uses, with their ceilings: MP's effective
/// maximum and Rage's cap (`u32::MAX` when the world sets none). `None` is a
/// resource the character does not use, which is not the same as one at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pools {
    pub mp: Option<u32>,
    pub rage: Option<u32>,
}

impl Pools {
    /// The most MP the character can hold: 0 when it does not use MP.
    pub fn mp_max(&self) -> u32 {
        self.mp.unwrap_or(0)
    }
    /// The most Rage the character can hold: 0 when it does not use Rage.
    pub fn rage_cap(&self) -> u32 {
        self.rage.unwrap_or(0)
    }
}

/// The pools of a character with effective `stats`. Without an explicit
/// choice, a character uses MP when it has any maximum MP, and Rage when the
/// world builds Rage at all.
pub(crate) fn pools(world: &WorldSpec, uses: Option<ResourceUse>, stats: &Stats) -> Pools {
    let rules = world.combat().unwrap().resources;
    let (mp, rage) = match uses {
        Some(uses) => (uses.mp, uses.rage),
        None => (stats.mp > 0, rules.gains_rage()),
    };
    Pools {
        mp: mp.then_some(stats.mp),
        rage: rage.then_some(rules.rage_cap.unwrap_or(u32::MAX)),
    }
}

/// The player's pools at effective stats `stats`.
pub(crate) fn player_pools(world: &WorldSpec, stats: &Stats) -> Pools {
    pools(world, world.combat().unwrap().player_resources, stats)
}

pub(crate) fn profile_pools(world: &WorldSpec, profile: &CombatProfile) -> Pools {
    pools(world, profile.resources, &profile.stats)
}

/// Fresh vitals at full HP and MP with no Rage, as a character enters play.
pub(crate) fn full(stats: &Stats, pools: Pools) -> Vitals {
    Vitals {
        hp: stats.hp,
        mp: pools.mp_max(),
        rage: 0,
        rage_remainder: 0,
    }
}

/// Whether `vitals` cover every cost of `skill`. A resource the character
/// does not use pays for nothing; a cost of 0 needs nothing.
pub(crate) fn affordable(vitals: &Vitals, pools: Pools, skill: &Skill) -> bool {
    afford(vitals, pools, skill).is_ok()
}

/// [`affordable`], or the error naming the short resource, MP first.
pub(crate) fn afford(vitals: &Vitals, pools: Pools, skill: &Skill) -> Result<(), EngineError> {
    let mp = skill.cost_in(Resource::Mp);
    if mp > 0 && (pools.mp.is_none() || vitals.mp < mp) {
        return Err(EngineError::NotEnoughMp(skill.id.clone()));
    }
    let rage = skill.cost_in(Resource::Rage);
    if rage > 0 && (pools.rage.is_none() || vitals.rage < rage) {
        return Err(EngineError::NotEnoughRage(skill.id.clone()));
    }
    Ok(())
}

/// Spends an affordable skill's costs, MP first, reporting each.
pub(crate) fn spend(vitals: &mut Vitals, character: &Id, skill: &Skill, events: &mut Vec<Event>) {
    for resource in [Resource::Mp, Resource::Rage] {
        let amount = skill.cost_in(resource);
        if amount == 0 {
            continue;
        }
        match resource {
            Resource::Mp => vitals.mp -= amount,
            Resource::Rage => vitals.rage -= amount,
        }
        events.push(Event::ResourceSpent {
            character: character.clone(),
            resource,
            amount,
        });
    }
}

/// Rage for resolving an attack or skill.
pub(crate) fn on_action(world: &WorldSpec, vitals: &mut Vitals, pools: Pools) {
    let Some(cap) = pools.rage else {
        return;
    };
    let gain = world.combat().unwrap().resources.rage_per_action;
    // Rage beyond the cap buys nothing more, so it saturates rather than failing.
    vitals.rage = vitals.rage.saturating_add(gain).min(cap);
    at_cap(world, vitals, cap);
}

/// Rage for `dealt` damage taken, before it is capped at the HP left:
/// `rage_per_max_hp` for a full bar of `max_hp`, cumulatively.
pub(crate) fn on_damage(
    world: &WorldSpec,
    vitals: &mut Vitals,
    pools: Pools,
    dealt: u32,
    max_hp: u32,
) {
    let Some(cap) = pools.rage else {
        return;
    };
    let per_max_hp = world.combat().unwrap().resources.rage_per_max_hp;
    let progress = u128::from(vitals.rage_remainder) + u128::from(per_max_hp) * u128::from(dealt);
    let max_hp = u128::from(max_hp);
    let rage = u128::from(vitals.rage) + progress / max_hp;
    vitals.rage = u32::try_from(rage).unwrap_or(u32::MAX).min(cap);
    vitals.rage_remainder = (progress % max_hp) as u64;
    at_cap(world, vitals, cap);
}

/// Damage taken at a world's cap banks nothing toward the next point.
pub(crate) fn at_cap(world: &WorldSpec, vitals: &mut Vitals, cap: u32) {
    let capped = world.combat().unwrap().resources.rage_cap.is_some();
    if capped && vitals.rage == cap {
        vitals.rage_remainder = 0;
    }
}

/// MP regained over `elapsed` units of some clock at `rate` percent of `max`
/// per `per_point / 100` units, with the clock's progress kept in
/// `remainder` so rounding loses nothing. Time spent full banks nothing.
pub(crate) fn regenerate(
    mp: &mut u32,
    remainder: &mut u64,
    max: u32,
    rate: u32,
    elapsed: u64,
    per_point: u128,
) {
    let progress =
        u128::from(*remainder) + u128::from(max) * u128::from(rate) * u128::from(elapsed);
    let gained = progress / per_point;
    if u128::from(*mp) + gained >= u128::from(max) {
        (*mp, *remainder) = (max, 0);
    } else {
        *mp += gained as u32;
        *remainder = (progress % per_point) as u64;
    }
}

/// How often world time reaches a multiple of `every` minutes in `(from, to]`.
/// Boundaries are fixed minutes of the clock, so splitting a span anywhere
/// crosses the same ones in total and nothing needs saving between them.
fn crossings(from: u64, to: u64, every: u64) -> u64 {
    to / every - from / every
}

/// What world time from minute `from` to `to` does to the player: MP
/// recovers and Rage decays at each boundary it crosses.
pub(crate) fn elapse(world: &WorldSpec, state: &mut GameState, from: u64, to: u64) {
    let (Some(combat), Some(rules)) = (state.combat.as_mut(), world.combat()) else {
        return;
    };
    let rules = rules.resources;
    let max = player_stats(world, combat);
    let pools = player_pools(world, &max);
    let vitals = &mut combat.vitals;
    if let (Some(recovery), Some(mp_max)) = (rules.mp_recovery, pools.mp) {
        let times = crossings(from, to, recovery.every);
        let each = (u64::from(mp_max) * u64::from(recovery.percent) / 100).max(1);
        let gained = u64::from(vitals.mp).saturating_add(times.saturating_mul(each));
        vitals.mp = gained.min(u64::from(mp_max)) as u32;
    }
    if let Some(decay) = rules.rage_decay.filter(|_| pools.rage.is_some()) {
        let times = crossings(from, to, decay.every);
        let lost = times.saturating_mul(u64::from(decay.points));
        vitals.rage = vitals
            .rage
            .saturating_sub(lost.try_into().unwrap_or(u32::MAX));
        // Rage drained by a boundary keeps no part of a point either; a
        // span that crosses none leaves the fraction to carry on.
        if lost > 0 && vitals.rage == 0 {
            vitals.rage_remainder = 0;
        }
    }
}

/// Rage that belongs to one fight leaves with it, unless the world carries it.
pub(crate) fn after_fight(world: &WorldSpec, vitals: &mut Vitals) {
    if !world.combat().unwrap().resources.rage_carries {
        vitals.rage = 0;
        vitals.rage_remainder = 0;
    }
}
