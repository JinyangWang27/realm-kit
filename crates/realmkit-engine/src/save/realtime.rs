//! Save checks for a real-time fight in progress. A fight may be saved
//! between any two commands: every committed command leaves it at a step
//! boundary, and its whole state (clock, fighters in join order, phases,
//! vitals of everyone but the player) is in the snapshot.

use super::*;
use crate::character::profile;
use crate::realtime;
use realmkit_spec::Combat;

/// A fight the rules could have left: the player first, then fighters who
/// are here, undefeated and each in it once, the first of them on team 1
/// with its whole group;
/// teams numbered in the order they appeared, allies only on the player's
/// and nobody else on it; joins in order and not after now; vitals within
/// maxima, regeneration progress below one point; every committed action
/// aimed at another team, ending after now and no later than its phase
/// allows; anyone out of the fight `Ready`; and the fight still open.
pub(super) fn check(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    fight: &RealtimeEncounter,
) -> Result<(), String> {
    let invalid = || "invalid real-time fight state".to_string();
    let realtime = rules.realtime.as_ref().ok_or_else(invalid)?;
    let now = fight.step;
    // The player and at least the opponent it started with, before any index.
    if fight.fighters.len() < 2 {
        return Err(invalid());
    }
    let player = &fight.fighters[0];
    let mut seen = BTreeSet::new();
    let mut last_team = 0;
    let mut last_joined = 0;
    let identities = player.character == world.world.player
        && player.vitals.is_none()
        && player.team == 0
        && player.control == Control::Player
        && player.joined == 0
        && fight
            .fighters
            .get(1)
            .is_some_and(|f| f.team == 1 && f.joined == 0)
        && fight.fighters[1..].iter().all(|f| {
            let profile =
                rules::character_here(world, state, &f.character).and_then(|c| c.combat.as_ref());
            let team_ok = match profile {
                Some(p) if p.ally => f.team == 0,
                Some(_) => (1..=last_team + 1).contains(&f.team),
                None => false,
            };
            let ok = team_ok
                && f.vitals.is_some()
                && f.control == Control::Policy
                && (last_joined..=now).contains(&f.joined)
                && f.character != world.world.player
                && !combat.defeated.contains(&f.character)
                && seen.insert(f.character.clone());
            last_team = last_team.max(f.team);
            last_joined = f.joined;
            ok
        });
    // Everyone the fight started with is still in it, from the start.
    let started = identities
        && encounter::opponents_for(world, state, &fight.fighters[1].character)
            .iter()
            .all(|c| {
                fight
                    .index(c)
                    .is_some_and(|t| fight.fighters[t].joined == 0)
            });
    // Identities first: stats exist only for characters that can fight.
    if !started {
        return Err(invalid());
    }
    let sides = realtime::sides(world, combat, fight);
    let yield_share = realtime::group(world, fight).and_then(|g| g.yield_share);
    let no_flee = realtime::group(world, fight).is_some_and(|g| g.no_flee);
    let per_point = 100 * u128::from(realtime.steps_per_second);
    let known = techniques::player_skills(world, combat);
    let fighters_ok = fight.fighters.iter().enumerate().all(|(i, f)| {
        let side = &sides[i];
        let v = fight.vitals(i, &combat.vitals);
        let fighting = fight.fighting(i, &combat.vitals);
        let hold = (i > 0)
            .then(|| profile(world, &f.character).guard_hold)
            .flatten();
        // A fighter yields when a hit leaves it at or below its threshold.
        // Others enter whole; the player may enter already that low, from
        // an earlier fight, and has not yielded until hit again.
        let yield_ok = match yield_share {
            Some(share) => {
                let low = v.hp <= encounter::yield_threshold(side.stats.hp, share);
                let entered_low = i == 0 && !f.yielded;
                v.hp > 0
                    && (f.yielded == low || entered_low || (!f.yielded && v.hp == side.stats.hp))
            }
            None => !f.yielded,
        };
        // A committed attack keeps the skill it began with, which a rank the
        // attack itself earned may since have replaced.
        let usable = |id: &Id| match i {
            0 => known
                .iter()
                .find(|s| &s.id == id)
                .copied()
                .or_else(|| superseded(world, combat, id)),
            _ => profile(world, &f.character)
                .skills
                .contains(id)
                .then(|| world.skill(id))
                .flatten(),
        };
        let action_ok = match &f.action {
            // A policy begins something at every step it is free, from the
            // step after it joined; anyone else out of the fight is `Ready`.
            RealtimeAction::Ready => i == 0 || !fighting || f.joined == now,
            // Only the player stays committed once out: dead, the fight open.
            _ if i > 0 && !fighting => false,
            RealtimeAction::Attack {
                skill,
                target,
                phase,
                until,
                connected,
            } => {
                let skill = skill.as_ref().map(usable);
                let aimed = fight
                    .index(target)
                    .is_some_and(|t| fight.fighters[t].team != f.team);
                aimed
                    && skill.is_none_or(|s| s.is_some_and(|s| s.level <= side.level))
                    && *until > now
                    && until - now <= longest(realtime, side, i, skill.flatten(), *phase)
                    && !(*phase == AttackPhase::Windup && *connected)
            }
            RealtimeAction::Guard { ready_at } => {
                let raised = *ready_at <= now.saturating_add(u64::from(realtime.guard_raise));
                // A policy attacks once its guard has been up its hold.
                raised
                    && (i == 0 || hold.is_some_and(|h| now < ready_at.saturating_add(u64::from(h))))
            }
            RealtimeAction::Retreat { until } => {
                i == 0 && !no_flee && *until > now && until - now <= u64::from(realtime.retreat)
            }
        };
        yield_ok
            && action_ok
            && combat::within(rules, v, &side.stats, side.pools)
            && u128::from(f.mp_remainder) < per_point
    });
    // A finished fight never stays open: the player and someone against
    // them still fight, or the player is dead.
    let dead = combat.vitals.hp == 0;
    let open = dead || (fight.fighting(0, &combat.vitals) && fight.contested(&combat.vitals));
    if fighters_ok && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// A skill of an earlier rank of a technique the player has learned.
fn superseded<'w>(world: &'w WorldSpec, combat: &CombatState, id: &str) -> Option<&'w Skill> {
    let earlier = combat.techniques.iter().any(|(technique, learned)| {
        world.technique(technique).is_some_and(|t| {
            t.ranks[..learned.rank - 1]
                .iter()
                .any(|r| r.skill.as_deref() == Some(id))
        })
    });
    earlier.then(|| world.skill(id)).flatten()
}

/// The most steps a phase begun earlier in this fight can have left. Other
/// fighters' stats are fixed, so their phases are exact. A rank the player
/// earns mid-fight can change their speed after a phase began, so theirs is
/// bounded at the slowest speed instead.
fn longest(
    realtime: &realmkit_spec::Realtime,
    side: &realtime::Side,
    i: usize,
    skill: Option<&Skill>,
    phase: AttackPhase,
) -> u64 {
    let mut slowest = *side;
    if i == 0 {
        slowest.stats.speed = 1;
    }
    realtime::attack_steps(realtime, &slowest, skill, phase)
}
