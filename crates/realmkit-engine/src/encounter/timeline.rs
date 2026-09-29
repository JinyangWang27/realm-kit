//! Time on the paused initiative timeline: how long actions delay the next
//! turn, regeneration while time passes, and who acts next.

use super::*;

/// Ticks until an actor's next turn after an action taking `time` percent of
/// a basic action: `max(1, ceil(action_cost × time / (100 × speed)))`, with
/// speed clamped to `[1, speed_cap]`.
pub(crate) fn delay(timeline: &Timeline, speed: u32, time: u32) -> Result<u64, EngineError> {
    let speed = u128::from(speed.clamp(1, timeline.speed_cap.max(1)));
    let ticks = u128::from(timeline.action_cost) * u128::from(time);
    u64::try_from(ticks.div_ceil(100 * speed).max(1)).map_err(|_| EngineError::NumericLimit)
}

/// One basic action at baseline speed 100; MP regeneration is measured in it.
pub(crate) fn baseline_turn(timeline: &Timeline) -> Result<u64, EngineError> {
    delay(timeline, 100, 100)
}

/// Regenerates everyone for the time until `actor`'s turn, then moves there.
pub(super) fn tick(
    world: &WorldSpec,
    encounter: &mut Encounter,
    me: Me,
    actor: usize,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let at = encounter.participants[actor].next_time;
    let elapsed = at - encounter.now;
    let per_point = 100 * u128::from(baseline_turn(&rules.timeline)?);
    for p in &mut encounter.participants {
        let max = stats(world, me.stats, p).mp;
        // Time spent at full MP banks nothing, so the remainder drops at the cap.
        let progress = u128::from(p.mp_remainder)
            + u128::from(max) * u128::from(rules.resources.mp_regen_percent) * u128::from(elapsed);
        let gained = progress / per_point;
        if u128::from(p.mp) + gained >= u128::from(max) {
            (p.mp, p.mp_remainder) = (max, 0);
        } else {
            p.mp += gained as u32;
            p.mp_remainder = (progress % per_point) as u64;
        }
    }
    encounter.now = at;
    Ok(())
}

/// The living participant who acts next: `(next_time, side, position)`.
pub(super) fn next_actor(encounter: &Encounter) -> Option<usize> {
    (0..encounter.participants.len())
        .filter(|&i| encounter.participants[i].fighting())
        .min_by_key(|&i| {
            let p = &encounter.participants[i];
            (p.next_time, p.side, i)
        })
}

/// The next `n` actors if every action took a basic action's time.
pub(crate) fn turn_order(world: &WorldSpec, state: &GameState, n: usize) -> Vec<Id> {
    let Some(combat) = &state.combat else {
        return Vec::new();
    };
    let Stance::Fighting(encounter) = &combat.stance else {
        return Vec::new();
    };
    let timeline = world.combat().unwrap().timeline;
    let mut projected = encounter.clone();
    let mut order = Vec::new();
    while order.len() < n {
        let Some(i) = next_actor(&projected) else {
            break;
        };
        let p = &projected.participants[i];
        let player = rules::player_stats(world, combat);
        // The player's basic action may be a heavy weapon's slower swing.
        let time = match p.control {
            Control::Player => gear::basic(world, combat).1.unwrap_or(100),
            Control::Policy => 100,
        };
        let Ok(step) = delay(&timeline, stats(world, player, p).speed, time) else {
            break;
        };
        order.push(p.character.clone());
        projected.participants[i].next_time = p.next_time.saturating_add(step);
    }
    order
}
