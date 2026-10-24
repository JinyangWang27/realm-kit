//! Resolving one action on the timeline: the character core decides its
//! damage, costs and rage; the timeline adds when the actor acts next, and
//! the encounter what it reports and who yields.

use super::*;
use crate::character::{resources, strike};

fn basic_channel(world: &WorldSpec, p: &Participant) -> Channel {
    match p.control {
        Control::Player => world.combat().unwrap().player_basic_channel,
        Control::Policy => profile(world, &p.character).basic_channel,
    }
}

fn basic_crit(world: &WorldSpec, p: &Participant) -> Option<realmkit_spec::Crit> {
    match p.control {
        Control::Player => world.combat().unwrap().player_basic_crit,
        Control::Policy => profile(world, &p.character).basic_crit,
    }
}

/// The player's command, or `Err` without any change when it cannot be used.
pub(crate) fn check_skill(
    vitals: &Vitals,
    pools: Pools,
    level: usize,
    skill: &Skill,
) -> Result<(), EngineError> {
    if skill.level > level {
        return Err(EngineError::SkillLocked(skill.id.clone()));
    }
    resources::afford(vitals, pools, skill)
}

/// The strongest affordable skill, else the basic attack. Equal power prefers
/// the cheaper skill, then the later tier; the first listed wins a full tie.
pub(super) fn choose<'w>(world: &'w WorldSpec, vitals: &Vitals, id: &str) -> Option<&'w Skill> {
    let profile = profile(world, id);
    let pools = resources::profile_pools(world, profile);
    // Wide enough that two authored costs never overflow.
    let cost =
        |s: &Skill| u64::from(s.cost_in(Resource::Mp)) + u64::from(s.cost_in(Resource::Rage));
    profile
        .skills
        .iter()
        .filter_map(|id| world.skill(id))
        .filter(|s| s.level <= profile.level && resources::affordable(vitals, pools, s))
        .rev()
        .max_by_key(|s| (s.power, std::cmp::Reverse(cost(s)), s.level))
}

/// What an action does before anything changes: its damage (the crit, if
/// any, drawn from the combat stream), and the delay to the actor's next turn.
struct Hit {
    dealt: u32,
    critical: bool,
    next: u64,
}

fn hit(
    world: &WorldSpec,
    board: &Board,
    me: Me,
    rng: &mut Option<RngState>,
    actor: usize,
    target: usize,
    skill: Option<&Skill>,
) -> Result<Hit, EngineError> {
    let participants = &board.encounter.participants;
    let (a, t) = (&participants[actor], &participants[target]);
    let (attacker, defender) = (stats(world, me.stats, a), stats(world, me.stats, t));
    let weapon = me.basic.0.filter(|_| a.control == Control::Player);
    let channel = weapon.unwrap_or_else(|| basic_channel(world, a));
    let attack = strike::Attack::of(world, skill, channel, basic_crit(world, a));
    // Only the player wears gear, so only hits on the player are modified.
    let guard = if t.control == Control::Player {
        me.guard(attack.channel)
    } else {
        (1, 1)
    };
    let (dealt, critical) = strike::damage_of(&attacker, &defender, attack, guard, rng)?;
    let basic_time = me
        .basic
        .1
        .filter(|_| a.control == Control::Player)
        .unwrap_or(100);
    let time = skill.map_or(basic_time, |s| s.time);
    let next = delay(timeline(world)?, attacker.speed, time)?;
    Ok(Hit {
        dealt,
        critical,
        next,
    })
}

/// Resolves one action. Rage from acting is credited after the action, so an
/// actor one point short cannot spend what its own action earns; rage from
/// being hit follows the damage dealt, before capping at the target's HP.
#[allow(clippy::too_many_arguments)]
pub(super) fn act(
    world: &WorldSpec,
    board: &mut Board,
    me: Me,
    turn: u64,
    rng: &mut Option<RngState>,
    actor: usize,
    target: usize,
    skill: Option<&Skill>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let Hit {
        dealt,
        critical,
        next,
    } = hit(world, board, me, rng, actor, target, skill)?;
    let max_hp = stats(world, me.stats, &board.encounter.participants[target]).hp;
    // In a yielding group nobody dies: a hit stops at 1 HP.
    let yield_share = group(world, board.encounter).and_then(|g| g.yield_share);
    let source = board.encounter.participants[actor].character.clone();
    let pools = board.pools(world, me, actor);
    let a = board.vitals_mut(actor);
    if let Some(skill) = skill {
        resources::spend(a, &source, skill, events);
    }
    resources::on_action(world, a, pools);
    let a = &mut board.encounter.participants[actor];
    a.next_time = a
        .next_time
        .checked_add(next)
        .ok_or(EngineError::NumericLimit)?;
    let pools = board.pools(world, me, target);
    let floor = u32::from(yield_share.is_some());
    let t = board.vitals_mut(target);
    let taken = strike::take(t, dealt, floor);
    resources::on_damage(world, t, pools, dealt, max_hp);
    let hp = t.hp;
    let t = &mut board.encounter.participants[target];
    let skill = skill.map(|s| s.id.clone());
    if actor == 0 {
        events.push(Event::DamageDealt {
            target: t.character.clone(),
            amount: taken,
            variant: (turn % rules.narrative.attack.len() as u64) as usize,
            skill,
            critical,
        });
        if hp == 0 {
            events.push(Event::EnemyDefeated {
                monster: t.character.clone(),
            });
        }
    } else {
        events.push(Event::DamageReceived {
            source,
            amount: taken,
            variant: (turn % rules.narrative.hurt.len() as u64) as usize,
            skill,
            critical,
        });
        if hp == 0 && t.control == Control::Player {
            events.push(Event::PlayerDied);
        }
    }
    if yield_share.is_some_and(|share| hp <= yield_threshold(max_hp, share)) {
        t.yielded = true;
        events.push(Event::Yielded {
            character: t.character.clone(),
        });
    }
    Ok(())
}
