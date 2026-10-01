//! Resolving one action: its damage, costs, rage and what it reports.

use super::*;

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

fn affordable(p: &Participant, skill: &Skill) -> bool {
    let pool = match skill.resource {
        Resource::Mp => p.mp,
        Resource::Rage => p.rage,
    };
    pool >= skill.cost
}

/// The player's command, or `Err` without any change when it cannot be used.
pub(crate) fn check_skill(p: &Participant, level: usize, skill: &Skill) -> Result<(), EngineError> {
    if skill.level > level {
        return Err(EngineError::SkillLocked(skill.id.clone()));
    }
    if !affordable(p, skill) {
        return Err(match skill.resource {
            Resource::Mp => EngineError::NotEnoughMp(skill.id.clone()),
            Resource::Rage => EngineError::NotEnoughRage(skill.id.clone()),
        });
    }
    Ok(())
}

/// The strongest affordable skill, else the basic attack. Equal power prefers
/// the cheaper skill, then the later tier; the first listed wins a full tie.
pub(super) fn choose<'w>(world: &'w WorldSpec, p: &Participant) -> Option<&'w Skill> {
    let profile = profile(world, &p.character);
    profile
        .skills
        .iter()
        .filter_map(|id| world.skill(id))
        .filter(|s| s.level <= profile.level && affordable(p, s))
        .rev()
        .max_by_key(|s| (s.power, std::cmp::Reverse(s.cost), s.level))
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
    encounter: &Encounter,
    me: Me,
    rng: &mut Option<RngState>,
    actor: usize,
    target: usize,
    skill: Option<&Skill>,
) -> Result<Hit, EngineError> {
    let rules = world.combat().unwrap();
    let (a, t) = (
        &encounter.participants[actor],
        &encounter.participants[target],
    );
    let (attacker, defender) = (stats(world, me.stats, a), stats(world, me.stats, t));
    let (channel, power, share) = match skill {
        Some(s) => (
            s.channel,
            s.power,
            s.cross_share.unwrap_or(rules.cross_share),
        ),
        None => {
            let weapon = me.basic.0.filter(|_| a.control == Control::Player);
            let channel = weapon.unwrap_or_else(|| basic_channel(world, a));
            (channel, BASIC_POWER, rules.cross_share)
        }
    };
    // A crit draws from the combat stream only when an action that has one resolves.
    let crit = match skill {
        Some(s) => s.crit,
        None => basic_crit(world, a),
    };
    let critical = match (crit, rng.as_mut().and_then(|r| r.combat.as_mut())) {
        (Some(c), Some(stream)) => rng::chance(stream, c.chance_percent),
        _ => false,
    };
    let multiplier = crit
        .filter(|_| critical)
        .map_or(100, |c| c.multiplier_percent);
    // Only the player wears gear, so only hits on the player are modified.
    let guard = if t.control == Control::Player {
        me.guard(channel)
    } else {
        (1, 1)
    };
    let dealt = damage_modified(
        &attacker, &defender, channel, power, share, multiplier, guard,
    )?;
    let basic_time = me
        .basic
        .1
        .filter(|_| a.control == Control::Player)
        .unwrap_or(100);
    let time = skill.map_or(basic_time, |s| s.time);
    let next = delay(&rules.timeline, attacker.speed, time)?;
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
    encounter: &mut Encounter,
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
    } = hit(world, encounter, me, rng, actor, target, skill)?;
    let max_hp = stats(world, me.stats, &encounter.participants[target]).hp;
    // In a yielding group nobody dies: a hit stops at 1 HP.
    let yield_share = group(world, encounter).and_then(|g| g.yield_share);
    let a = &mut encounter.participants[actor];
    if let Some(skill) = skill.filter(|s| s.cost > 0) {
        match skill.resource {
            Resource::Mp => a.mp -= skill.cost,
            Resource::Rage => a.rage -= skill.cost,
        }
        events.push(Event::ResourceSpent {
            character: a.character.clone(),
            resource: skill.resource,
            amount: skill.cost,
        });
    }
    // Rage beyond u32 buys nothing more, so it saturates rather than failing.
    a.rage = a.rage.saturating_add(rules.resources.rage_per_action);
    a.next_time = a
        .next_time
        .checked_add(next)
        .ok_or(EngineError::NumericLimit)?;
    let source = a.character.clone();
    let t = &mut encounter.participants[target];
    let floor = u32::from(yield_share.is_some());
    let taken = dealt.min(t.hp - floor);
    t.hp -= taken;
    let progress = u128::from(t.rage_remainder)
        + u128::from(rules.resources.rage_per_max_hp) * u128::from(dealt);
    let max_hp_wide = u128::from(max_hp);
    t.rage = u32::try_from(u128::from(t.rage) + progress / max_hp_wide).unwrap_or(u32::MAX);
    t.rage_remainder = (progress % max_hp_wide) as u64;
    let skill = skill.map(|s| s.id.clone());
    if actor == 0 {
        events.push(Event::DamageDealt {
            target: t.character.clone(),
            amount: taken,
            variant: (turn % rules.narrative.attack.len() as u64) as usize,
            skill,
            critical,
        });
        if t.hp == 0 {
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
        if t.hp == 0 && t.control == Control::Player {
            events.push(Event::PlayerDied);
        }
    }
    if yield_share.is_some_and(|share| t.hp <= yield_threshold(max_hp, share)) {
        t.yielded = true;
        events.push(Event::Yielded {
            character: t.character.clone(),
        });
    }
    Ok(())
}
