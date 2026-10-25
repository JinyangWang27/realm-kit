//! Save checks for a duel in progress. A duel may be saved between any two
//! commands: every committed command leaves it at a step boundary, and its
//! whole state (clock, phases, opponent's vitals) is in the snapshot.

use super::*;
use crate::character::profile;
use crate::realtime;
use realmkit_spec::Combat;

/// A duel the rules could have left: the player and a lone opponent who is
/// here and undefeated, vitals within maxima, regeneration progress below
/// one point, every committed action ending after now and no later than its
/// phase allows, and the duel still open.
pub(super) fn check(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    duel: &Duel,
) -> Result<(), String> {
    let invalid = || "invalid duel state".to_string();
    let realtime = rules.realtime.as_ref().ok_or_else(invalid)?;
    let [player, opponent] = &duel.fighters;
    let identities = player.character == world.world.player
        && player.vitals.is_none()
        && opponent.vitals.is_some()
        && !combat.defeated.contains(&opponent.character)
        && rules::character_here(world, state, &opponent.character)
            .is_some_and(|c| c.combat.is_some())
        && encounter::opponents_for(world, state, &opponent.character)
            == [opponent.character.clone()];
    // Identities first: stats exist only for characters that can fight.
    if !identities {
        return Err(invalid());
    }
    let sides = realtime::sides(world, combat, &opponent.character);
    let yield_share = realtime::group(world, duel).and_then(|g| g.yield_share);
    let no_flee = realtime::group(world, duel).is_some_and(|g| g.no_flee);
    let per_point = 100 * u128::from(realtime.steps_per_second);
    let known = techniques::player_skills(world, combat);
    let now = duel.step;
    let fighters_ok = duel.fighters.iter().enumerate().all(|(i, f)| {
        let side = &sides[i];
        let v = duel.vitals(i, &combat.vitals);
        let yield_ok = match yield_share {
            Some(share) => {
                let low = v.hp <= encounter::yield_threshold(side.stats.hp, share);
                v.hp > 0 && (f.yielded == low || (!f.yielded && v.hp == side.stats.hp))
            }
            None => !f.yielded,
        };
        let usable = |id: &Id| match i {
            0 => known.iter().find(|s| &s.id == id).copied(),
            _ => profile(world, &f.character)
                .skills
                .contains(id)
                .then(|| world.skill(id))
                .flatten(),
        };
        let action_ok = match &f.action {
            // The opponent's policy begins an attack at every step it is free.
            DuelAction::Ready => i == 0 || now == 0,
            DuelAction::Attack {
                skill,
                phase,
                until,
                connected,
            } => {
                let skill = skill.as_ref().map(usable);
                skill.is_none_or(|s| s.is_some_and(|s| s.level <= side.level))
                    && *until > now
                    && until - now
                        <= realtime::attack_steps(realtime, side, skill.flatten(), *phase)
                    && !(*phase == AttackPhase::Windup && *connected)
            }
            DuelAction::Guard { ready_at } => {
                i == 0 && *ready_at <= now.saturating_add(u64::from(realtime.guard_raise))
            }
            DuelAction::Retreat { until } => {
                i == 0 && !no_flee && *until > now && until - now <= u64::from(realtime.retreat)
            }
        };
        yield_ok
            && action_ok
            && combat::within(rules, v, &side.stats, side.pools)
            && u128::from(f.mp_remainder) < per_point
    });
    // A finished duel never stays open: both still fight, or the player is dead.
    let dead = combat.vitals.hp == 0;
    let open = dead || (duel.fighting(0, &combat.vitals) && duel.fighting(1, &combat.vitals));
    if fighters_ok && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}
