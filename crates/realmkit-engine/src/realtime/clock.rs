//! The fight's fixed-step clock. Each step runs in one order: the clock
//! moves on, every fighter still fighting regenerates MP, each fighter's
//! committed action moves through its phases, and each engine-controlled
//! fighter acts by its policy, all in join order. Nothing depends on how
//! steps are batched into advances, so a host may submit one step per frame
//! or many at once.

use super::*;

/// Runs `steps` steps, stopping at the step the fight ends.
pub(super) fn advance(
    world: &WorldSpec,
    state: &mut GameState,
    steps: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    for _ in 0..steps {
        if step(world, state, events)? {
            return super::end(world, state, Outcome::Fled, events);
        }
    }
    Ok(())
}

/// One step; whether the player's retreat completed at it.
fn step(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<bool, EngineError> {
    let realtime = *rules(world)?;
    let mut ring = ring(world, state)?;
    ring.fight.step = ring
        .fight
        .step
        .checked_add(1)
        .ok_or(EngineError::NumericLimit)?;
    let now = ring.fight.step;
    // Fight time regenerates only on this clock, per second of steps.
    let per_point = 100 * u128::from(realtime.steps_per_second);
    for i in 0..ring.fight.fighters.len() {
        if !ring.fighting(i) {
            continue;
        }
        let max = ring.sides[i].pools.mp_max();
        let mut remainder = ring.fight.fighters[i].mp_remainder;
        let rate = realtime.mp_regen_percent;
        let mp = &mut ring.vitals_mut(i).mp;
        resources::regenerate(mp, &mut remainder, max, rate, 1, per_point);
        ring.fight.fighters[i].mp_remainder = remainder;
    }
    for i in 0..ring.fight.fighters.len() {
        if phase(world, &realtime, &mut ring, i, now, events)? {
            return Ok(true);
        }
    }
    for i in 0..ring.fight.fighters.len() {
        if ring.fight.fighters[i].control == Control::Policy && ring.fighting(i) {
            policy(world, &realtime, &mut ring, i, now, events)?;
        }
    }
    Ok(false)
}

/// An engine-controlled fighter's authored policy: with a `guard_hold`, it
/// raises its guard whenever it is ready and attacks once the guard has
/// been up that many steps; without one, it attacks whenever it is ready.
/// Each attack aims by its `targeting` (a named companion's by its order
/// first) and uses the strongest skill it can pay for, else a basic attack,
/// chosen as in turn-based encounters. A companion with nobody it would
/// strike now waits as it is, ready or behind its guard.
fn policy(
    world: &WorldSpec,
    realtime: &Realtime,
    ring: &mut Ring,
    i: usize,
    now: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let character = &ring.fight.fighters[i].character;
    let hold = profile(world, character).guard_hold;
    let attack = match (&ring.fight.fighters[i].action, hold) {
        (RealtimeAction::Ready, Some(_)) => return raise(realtime, ring, i, events),
        (RealtimeAction::Ready, None) => true,
        (RealtimeAction::Guard { ready_at }, Some(hold)) => {
            now >= ready_at.saturating_add(u64::from(hold))
        }
        _ => false,
    };
    let Some(t) = pick(world, ring, i).filter(|_| attack) else {
        return Ok(());
    };
    let skill = encounter::choose(world, ring.vitals(i), character);
    let target = ring.fight.fighters[t].character.clone();
    begin(realtime, ring, i, skill, target, events)
}

/// Moves fighter `i`'s action on if its current state ends at `now`;
/// whether a retreat completed.
fn phase(
    world: &WorldSpec,
    realtime: &Realtime,
    ring: &mut Ring,
    i: usize,
    now: u64,
    events: &mut Vec<Event>,
) -> Result<bool, EngineError> {
    let side = ring.sides[i];
    let fighter = &mut ring.fight.fighters[i];
    let character = fighter.character.clone();
    match &mut fighter.action {
        RealtimeAction::Attack {
            skill,
            phase,
            until,
            connected,
            ..
        } if *until == now => {
            let id = skill.clone();
            let skill = id.as_deref().map(|id| world.skill(id).unwrap());
            let next = match phase {
                AttackPhase::Windup => AttackPhase::Active,
                AttackPhase::Active => {
                    if !*connected {
                        events.push(Event::Missed {
                            character: character.clone(),
                        });
                    }
                    AttackPhase::Recovery
                }
                AttackPhase::Recovery => {
                    fighter.action = RealtimeAction::Ready;
                    events.push(Event::Ready { character });
                    return Ok(false);
                }
            };
            *phase = next;
            *until = now
                .checked_add(attack_steps(realtime, &side, skill, next))
                .ok_or(EngineError::NumericLimit)?;
            events.push(Event::Swing {
                character,
                skill: id,
                phase: next,
            });
            Ok(false)
        }
        RealtimeAction::Guard { ready_at } if *ready_at == now => {
            events.push(Event::GuardUp { character });
            Ok(false)
        }
        RealtimeAction::Retreat { until } => Ok(*until == now),
        _ => Ok(false),
    }
}
