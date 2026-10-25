//! The duel's fixed-step clock. Each step runs in one order: the clock
//! moves on, both fighters regenerate MP, the player's committed action
//! moves through its phases, then the opponent's, and a ready opponent
//! begins its next attack. Nothing depends on how steps are batched into
//! advances, so a host may submit one step per frame or many at once.

use super::*;

/// Runs `steps` steps, stopping at the step the duel ends.
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
    let mut ring = dueling(world, &mut state.combat)?;
    ring.duel.step = ring
        .duel
        .step
        .checked_add(1)
        .ok_or(EngineError::NumericLimit)?;
    let now = ring.duel.step;
    // Duel time regenerates only on this clock, per second of steps.
    let per_point = 100 * u128::from(realtime.steps_per_second);
    for i in 0..2 {
        let max = ring.sides[i].pools.mp_max();
        let fighter = &mut ring.duel.fighters[i];
        let mp = match fighter.vitals.as_mut() {
            Some(vitals) => &mut vitals.mp,
            None => &mut ring.player.mp,
        };
        let rate = realtime.mp_regen_percent;
        resources::regenerate(mp, &mut fighter.mp_remainder, max, rate, 1, per_point);
    }
    for i in 0..2 {
        if phase(world, &realtime, &mut ring, i, now, events)? {
            return Ok(true);
        }
    }
    // The opponent's authored policy: attack whenever free, with the
    // strongest skill it can pay for.
    if ring.duel.fighters[1].action == DuelAction::Ready {
        let opponent = &ring.duel.fighters[1].character;
        let skill = encounter::choose(world, ring.duel.vitals(1, ring.player), opponent);
        begin(&realtime, &mut ring, 1, skill, events)?;
    }
    Ok(false)
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
    let fighter = &mut ring.duel.fighters[i];
    let character = fighter.character.clone();
    match &mut fighter.action {
        DuelAction::Attack {
            skill,
            phase,
            until,
            connected,
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
                    fighter.action = DuelAction::Ready;
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
        DuelAction::Guard { ready_at } if *ready_at == now => {
            events.push(Event::GuardUp { character });
            Ok(false)
        }
        DuelAction::Retreat { until } => Ok(*until == now),
        _ => Ok(false),
    }
}
